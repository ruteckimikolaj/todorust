use std::{
    io::{self, stdout, Stdout},
    panic,
    time::{Duration, Instant},
};

use chrono::Utc;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify_rust::Notification;
use ratatui::prelude::*;
use ratatui_textarea::Input;

mod app;
mod db;
mod settings;
mod ui;
use app::{App, InputMode, UiState, View};
use settings::{Settings, Theme};
use ui::{
    draw_dashboard, draw_due_modal, draw_notes_modal, draw_settings, draw_statistics,
    draw_task_details, draw_task_list,
};

/// A minimalist, powerful to-do manager for your terminal.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {}

fn main() -> io::Result<()> {
    // This panic hook ensures the terminal is restored even if a Rust-level panic occurs.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let mut stdout = stdout();
        execute!(stdout, LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
        original_hook(panic_info);
    }));

    let _cli = Cli::parse();

    let mut terminal = setup_terminal()?;

    let settings = Settings::load();
    let mut app = App::load_with_settings(settings);

    run_app(&mut terminal, &mut app)?;
    restore_terminal(&mut terminal)?;
    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);
    let mut ui_state = UiState::default();
    let mut ticks_since_save: u32 = 0;
    const AUTOSAVE_TICKS: u32 = 120; // ~30 seconds

    loop {
        terminal.draw(|f| ui(f, app, &ui_state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(key, app, &mut ui_state);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            check_due_notifications(app);
            last_tick = Instant::now();
            ticks_since_save += 1;
            if ticks_since_save >= AUTOSAVE_TICKS {
                app.save();
                ticks_since_save = 0;
            }
        }

        if app.should_quit {
            app.save();
            return Ok(());
        }
    }
}

/// Fire one desktop notification per task the moment its due date passes.
fn check_due_notifications(app: &mut App) {
    let now = Utc::now();
    let notify = app.settings.desktop_notifications;
    for task in app.tasks.iter_mut() {
        if task.completed || task.due_notified {
            continue;
        }
        if task.due_date.is_some_and(|d| d <= now) {
            task.due_notified = true;
            if notify {
                let _ = Notification::new()
                    .summary("Task due")
                    .body(&task.name)
                    .icon("dialog-information")
                    .show();
            }
        }
    }
}

fn handle_key_event(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    if key.kind != crossterm::event::KeyEventKind::Press {
        return;
    }

    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        app.should_quit = true;
        return;
    }

    match ui.input_mode {
        InputMode::Editing => handle_editing_input(key, app, ui),
        InputMode::Filtering => handle_filtering_input(key, ui),
        InputMode::EditingNotes => handle_editing_notes_input(key, app, ui),
        InputMode::EditingDue => handle_editing_due_input(key, app, ui),
        InputMode::EditingSubtask => handle_editing_subtask_input(key, app, ui),
        InputMode::Normal => {
            if key.code == KeyCode::Char('o')
                && key.modifiers == KeyModifiers::NONE
                && app.current_view != View::Settings
            {
                ui.previous_view = app.current_view;
                app.current_view = View::Settings;
                return;
            }

            match app.current_view {
                View::Dashboard => handle_dashboard_input(key, app, ui),
                View::TaskList => handle_tasklist_input(key, app, ui),
                View::Statistics => handle_stats_input(key, app, ui),
                View::Settings => handle_settings_input(key, app, ui),
                View::TaskDetails => handle_task_details_input(key, app, ui),
            }
        }
    }
}

fn handle_dashboard_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => {
            ui.previous_view = app.current_view;
            app.current_view = View::TaskList;
        }
        _ => {}
    }
}

fn handle_tasklist_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key {
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::SHIFT,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('K'),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => app.move_active_task_up(),
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::SHIFT,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('J'),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => app.move_active_task_down(),

        KeyEvent { code, .. } => match code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Tab => {
                ui.previous_view = app.current_view;
                app.current_view = View::Statistics;
            }
            KeyCode::Char('n') => ui.input_mode = InputMode::Editing,
            KeyCode::Char('a') => ui.start_add_subtask(app),
            KeyCode::Char(' ') | KeyCode::Char('x') => ui.toggle_selected_subtask(app),
            KeyCode::Char('A') if key.modifiers == KeyModifiers::SHIFT => {
                ui.show_archived = !ui.show_archived;
            }
            KeyCode::Char('e') => ui.start_rename(app),
            KeyCode::Char('E') if key.modifiers == KeyModifiers::SHIFT => {
                ui.start_edit_notes_active(app)
            }
            KeyCode::Char('p') => app.cycle_active_priority(),
            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => ui.start_edit_due(app),
            KeyCode::Char('s') => app.cycle_sort_mode(),
            KeyCode::Char('/') => ui.input_mode = InputMode::Filtering,
            KeyCode::Down | KeyCode::Char('j') => ui.next_active_task(app),
            KeyCode::Up | KeyCode::Char('k') => ui.previous_active_task(app),
            KeyCode::Enter => {
                ui.selected_subtask = None;
                app.complete_active_task();
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                ui.selected_subtask = None;
                app.delete_active_task();
            }
            _ => {}
        },
    }
}

fn handle_stats_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => {
            ui.previous_view = app.current_view;
            app.current_view = View::Dashboard;
        }
        KeyCode::Char('/') => ui.input_mode = InputMode::Filtering,
        KeyCode::Down | KeyCode::Char('j') => ui.next_completed_task(app),
        KeyCode::Up | KeyCode::Char('k') => ui.previous_completed_task(app),
        KeyCode::Enter => {
            if ui.completed_task_list_state.is_some() {
                ui.previous_view = app.current_view;
                app.current_view = View::TaskDetails;
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => ui.delete_selected_completed_task(app),
        _ => {}
    }
}

fn handle_settings_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.current_view = ui.previous_view,
        KeyCode::Up | KeyCode::Char('k') => ui.previous_setting(),
        KeyCode::Down | KeyCode::Char('j') => ui.next_setting(),
        KeyCode::Left | KeyCode::Char('h') => ui.modify_setting(app, false),
        KeyCode::Right | KeyCode::Char('l') => ui.modify_setting(app, true),
        _ => {}
    }
}

fn handle_task_details_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('E') if key.modifiers == KeyModifiers::SHIFT => ui.start_edit_notes(app),
        KeyCode::Esc | KeyCode::Enter => app.current_view = ui.previous_view,
        _ => {}
    }
}

fn handle_editing_notes_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key {
        KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            ui.submit_notes(app);
        }
        KeyEvent {
            code: KeyCode::Esc, ..
        } => {
            ui.cancel_notes();
        }
        _ => {
            if let Some(textarea) = &mut ui.notes_textarea {
                textarea.input(Input::from(key));
            }
        }
    }
}

fn handle_editing_due_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Enter => ui.submit_due(app),
        KeyCode::Esc => ui.cancel_due(),
        KeyCode::Char(c) => {
            ui.due_error = false;
            ui.due_input.push(c);
        }
        KeyCode::Backspace => {
            ui.due_error = false;
            ui.due_input.pop();
        }
        _ => {}
    }
}

fn handle_editing_subtask_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Enter => ui.submit_subtask(app),
        KeyCode::Char(c) => ui.subtask_input.push(c),
        KeyCode::Backspace => {
            ui.subtask_input.pop();
        }
        KeyCode::Esc => ui.cancel_subtask(),
        _ => {}
    }
}

fn handle_filtering_input(key: KeyEvent, ui: &mut UiState) {
    match key.code {
        KeyCode::Char(c) => ui.filter_input.push(c),
        KeyCode::Backspace => {
            ui.filter_input.pop();
        }
        KeyCode::Esc => {
            ui.input_mode = InputMode::Normal;
            ui.filter_input.clear();
        }
        KeyCode::Enter => ui.input_mode = InputMode::Normal,
        _ => {}
    }
}

fn handle_editing_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Enter => ui.submit_task(app),
        KeyCode::Char(c) => ui.current_input.push(c),
        KeyCode::Backspace => {
            ui.current_input.pop();
        }
        KeyCode::Esc => {
            ui.input_mode = InputMode::Normal;
            ui.current_input.clear();
            ui.editing_task_index = None;
        }
        _ => {}
    }
}

fn ui(frame: &mut Frame, app: &App, ui_state: &UiState) {
    let theme = Theme::from_settings(app.settings.theme, app.settings.custom_theme.as_ref());
    match app.current_view {
        View::Dashboard => draw_dashboard(frame, app, &theme),
        View::TaskList => draw_task_list(frame, app, ui_state, &theme),
        View::Statistics => draw_statistics(frame, app, ui_state, &theme),
        View::Settings => draw_settings(frame, app, ui_state, &theme),
        View::TaskDetails => draw_task_details(frame, app, ui_state, &theme),
    }
    match ui_state.input_mode {
        InputMode::EditingNotes => draw_notes_modal(frame, ui_state, &theme),
        InputMode::EditingDue => draw_due_modal(frame, ui_state, &theme),
        _ => {}
    }
}
