use std::{
    io::{self, stdout, Stdout},
    panic,
    time::{Duration, Instant},
};

use chrono::Utc;
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseEvent, MouseEventKind,
    },
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
use app::ui_state::SheetField;
use app::{App, InputMode, Priority, UiState, View};
use settings::{Settings, Theme};
use ui::{
    draw_edit_sheet, draw_notes_modal, draw_settings, draw_statistics, draw_task_details,
    draw_task_list,
};

/// A minimalist, powerful to-do manager for your terminal.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Export all tasks (with subtasks) to a JSON file, or `-` for stdout.
    Export {
        /// Path to write JSON to. Use `-` for stdout.
        path: String,
    },
    /// Import tasks from a JSON file previously produced by `export`. By default the
    /// imported tasks are appended; pass `--replace` to overwrite the existing store.
    Import {
        /// Path to read JSON from. Use `-` for stdin.
        path: String,
        /// Replace the existing task store instead of appending.
        #[arg(long)]
        replace: bool,
    },
}

fn main() -> io::Result<()> {
    // This panic hook ensures the terminal is restored even if a Rust-level panic occurs.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let mut stdout = stdout();
        let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
        disable_raw_mode().unwrap();
        original_hook(panic_info);
    }));

    let cli = Cli::parse();

    // Handle CLI subcommands without launching the TUI.
    if let Some(cmd) = cli.command {
        return match cmd {
            Command::Export { path } => run_export(&path),
            Command::Import { path, replace } => run_import(&path, replace),
        };
    }

    let mut terminal = setup_terminal()?;

    let settings = Settings::load();
    let mut app = App::load_with_settings(settings);

    run_app(&mut terminal, &mut app)?;
    restore_terminal(&mut terminal)?;
    Ok(())
}

fn run_export(path: &str) -> io::Result<()> {
    let settings = Settings::load();
    let app = App::load_with_settings(settings);
    let json = serde_json::to_string_pretty(&app.tasks).map_err(io::Error::other)?;
    if path == "-" {
        println!("{}", json);
    } else {
        std::fs::write(path, json)?;
        eprintln!("Exported {} task(s) to {}", app.tasks.len(), path);
    }
    Ok(())
}

fn run_import(path: &str, replace: bool) -> io::Result<()> {
    let raw = if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(path)?
    };
    let incoming: Vec<app::Task> =
        serde_json::from_str(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let settings = Settings::load();
    let mut app = App::load_with_settings(settings);
    let added = incoming.len();
    if replace {
        app.tasks = incoming;
    } else {
        app.tasks.extend(incoming);
    }
    app.save();
    eprintln!(
        "Imported {} task(s){} — store now has {}.",
        added,
        if replace { " (replaced existing)" } else { "" },
        app.tasks.len()
    );
    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
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
            match event::read()? {
                Event::Key(key) => handle_key_event(key, app, &mut ui_state),
                Event::Mouse(mev) => handle_mouse_event(mev, app, &mut ui_state),
                _ => {}
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

fn handle_mouse_event(mev: MouseEvent, app: &mut App, ui: &mut UiState) {
    // Only take mouse actions in Normal mode to avoid stealing focus while the
    // user is typing in the quick-add / filter / reschedule prompts.
    if !matches!(ui.input_mode, InputMode::Normal) {
        return;
    }
    match mev.kind {
        MouseEventKind::ScrollUp => match app.current_view {
            View::TaskList => ui.previous_active_task(app),
            View::Statistics => ui.previous_completed_task(app),
            _ => {}
        },
        MouseEventKind::ScrollDown => match app.current_view {
            View::TaskList => ui.next_active_task(app),
            View::Statistics => ui.next_completed_task(app),
            _ => {}
        },
        _ => {}
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
        InputMode::EditingSubtask => handle_editing_subtask_input(key, app, ui),
        InputMode::EditingSheet => handle_editing_sheet_input(key, app, ui),
        InputMode::Rescheduling => handle_rescheduling_input(key, app, ui),
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
                View::TaskList => handle_tasklist_input(key, app, ui),
                View::Statistics => handle_stats_input(key, app, ui),
                View::Settings => handle_settings_input(key, app, ui),
                View::TaskDetails => handle_task_details_input(key, app, ui),
            }
        }
    }
}

fn handle_tasklist_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    // The help overlay swallows every key; any press dismisses it.
    if ui.show_help {
        ui.show_help = false;
        return;
    }

    // The delete confirmation intercepts input: only `y`/Enter destroys.
    if ui.confirm_delete {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                ui.selected_subtask = None;
                app.delete_active_task();
            }
            _ => {}
        }
        ui.confirm_delete = false;
        return;
    }

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
            // `a`dd a top-level task; `+` adds a subtask under the active parent.
            KeyCode::Char('a') => ui.input_mode = InputMode::Editing,
            KeyCode::Char('+') => ui.start_add_subtask(app),
            // Space/x is the checkbox: toggle the highlighted subtask if one is
            // selected, otherwise toggle the parent task's done state.
            KeyCode::Char(' ') | KeyCode::Char('x') => {
                if ui.selected_subtask.is_some() {
                    ui.toggle_selected_subtask(app);
                } else {
                    app.complete_active_task();
                }
            }
            KeyCode::Char('A') if key.modifiers == KeyModifiers::SHIFT => {
                ui.show_archived = !ui.show_archived;
            }
            // Enter/e open the edit sheet — the one place all attributes are edited.
            KeyCode::Enter | KeyCode::Char('e') => ui.open_edit_sheet(app),
            // Fast priority set on the selected task.
            KeyCode::Char('1') => app.set_active_priority(Priority::Low),
            KeyCode::Char('2') => app.set_active_priority(Priority::Medium),
            KeyCode::Char('3') => app.set_active_priority(Priority::High),
            // Reschedule presets: today / tomorrow / next week / prompt.
            KeyCode::Char('t') => ui.reschedule_today(app),
            KeyCode::Char('T') if key.modifiers == KeyModifiers::SHIFT => {
                ui.reschedule_tomorrow(app);
            }
            KeyCode::Char('w') => ui.reschedule_next_week(app),
            KeyCode::Char('r') => ui.start_reschedule(app),
            KeyCode::Char('s') | KeyCode::Char('g') => app.cycle_grouping_mode(),
            KeyCode::Char('/') => ui.input_mode = InputMode::Filtering,
            KeyCode::Down | KeyCode::Char('j') => ui.next_active_task(app),
            KeyCode::Up | KeyCode::Char('k') => ui.previous_active_task(app),
            // Never silent-destroy: arm a confirmation prompt instead.
            KeyCode::Char('d') | KeyCode::Delete => {
                if app.active_task_index.is_some() {
                    ui.confirm_delete = true;
                }
            }
            KeyCode::Char('?') => ui.show_help = true,
            _ => {}
        },
    }
}

fn handle_stats_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => {
            ui.previous_view = app.current_view;
            app.current_view = View::TaskList;
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

fn handle_editing_sheet_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    // Save / cancel take priority over field-local handling.
    match (key.code, key.modifiers) {
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
            ui.submit_sheet(app);
            return;
        }
        (KeyCode::Esc, _) => {
            ui.cancel_sheet();
            return;
        }
        _ => {}
    }

    let Some(sheet) = ui.edit_sheet.as_mut() else {
        return;
    };

    // Tab / Shift+Tab move between fields regardless of which field is focused.
    match key.code {
        KeyCode::Tab => {
            sheet.field = sheet.field.next();
            return;
        }
        KeyCode::BackTab => {
            sheet.field = sheet.field.prev();
            return;
        }
        _ => {}
    }

    match sheet.field {
        SheetField::Priority => match key.code {
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                sheet.priority = sheet.priority.cycle();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // Cycle is Low→Med→High→Low; two hops backward = one step back.
                sheet.priority = sheet.priority.cycle().cycle();
            }
            _ => {}
        },
        SheetField::Notes => {
            sheet.notes.input(Input::from(key));
        }
        // Name / Project / Due are plain text fields.
        _ => match key.code {
            KeyCode::Char(c) => {
                if let Some(buf) = ui.sheet_text_field_mut() {
                    buf.push(c);
                }
            }
            KeyCode::Backspace => {
                if let Some(buf) = ui.sheet_text_field_mut() {
                    buf.pop();
                }
            }
            _ => {}
        },
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

fn handle_rescheduling_input(key: KeyEvent, app: &mut App, ui: &mut UiState) {
    match key.code {
        KeyCode::Enter => ui.submit_reschedule(app),
        KeyCode::Esc => ui.cancel_reschedule(),
        KeyCode::Char(c) => {
            ui.reschedule_error = false;
            ui.reschedule_input.push(c);
        }
        KeyCode::Backspace => {
            ui.reschedule_error = false;
            ui.reschedule_input.pop();
        }
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
        View::TaskList => draw_task_list(frame, app, ui_state, &theme),
        View::Statistics => draw_statistics(frame, app, ui_state, &theme),
        View::Settings => draw_settings(frame, app, ui_state, &theme),
        View::TaskDetails => draw_task_details(frame, app, ui_state, &theme),
    }
    match ui_state.input_mode {
        InputMode::EditingNotes => draw_notes_modal(frame, ui_state, &theme),
        InputMode::EditingSheet => draw_edit_sheet(frame, ui_state, &theme),
        _ => {}
    }
}
