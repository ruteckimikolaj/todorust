use chrono::{Local, Utc};
use ratatui::{prelude::*, widgets::*};

use crate::app::{App, InputMode, Priority, Task, UiState};
use crate::settings::Theme;

fn priority_color(p: Priority, theme: &Theme) -> Color {
    match p {
        Priority::Low => theme.low_color,
        Priority::Medium => theme.medium_color,
        Priority::High => theme.high_color,
    }
}

/// Short human badge for a task's due date, plus the colour it should use.
fn due_badge(task: &Task, theme: &Theme) -> Option<(String, Color)> {
    let due = task.due_date?;
    let local = due.with_timezone(&Local);
    if task.is_overdue() {
        return Some((
            format!("⚠ {}", local.format("%m-%d %H:%M")),
            theme.high_color,
        ));
    }
    let today = Local::now().date_naive();
    if local.date_naive() == today {
        Some((format!("◷ {}", local.format("%H:%M")), theme.medium_color))
    } else {
        Some((
            format!("◷ {}", local.format("%m-%d %H:%M")),
            theme.help_text_fg,
        ))
    }
}

pub fn draw_task_list(frame: &mut Frame, app: &App, ui: &UiState, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(4),
        ])
        .split(frame.area());

    frame.render_widget(
        Block::default()
            .title(" ✓ TASKS ")
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        chunks[0],
    );

    let filter = ui.filter_input.to_lowercase();
    let active_indices = app.ordered_active_indices(&filter);

    let mut list_title = format!("Active Tasks — sort: {}", app.sort_mode.title());
    if !ui.filter_input.is_empty() {
        list_title.push_str(&format!(" [/{}]", ui.filter_input));
    }

    // Flattened render model: each parent, and — for the active parent — its
    // visible subtask rows (active checklist, then an archived section on demand).
    let now = Utc::now();
    let mut active_list_items: Vec<ListItem> = Vec::new();
    let mut selected_pos: Option<usize> = None;
    for &i in &active_indices {
        let task = &app.tasks[i];
        let is_active = Some(i) == app.active_task_index;
        let marker = if is_active { "▶ " } else { "  " };

        if is_active && ui.selected_subtask.is_none() {
            selected_pos = Some(active_list_items.len());
        }

        let mut spans = vec![
            Span::styled(
                format!("{}[ ] ", marker),
                Style::default().fg(theme.base_fg),
            ),
            Span::styled(
                format!("{} ", task.priority.glyph()),
                Style::default()
                    .fg(priority_color(task.priority, theme))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(task.name.clone(), Style::default().fg(theme.base_fg)),
        ];
        if let Some(proj) = &task.project {
            spans.push(Span::styled(
                format!(" @{}", proj),
                Style::default().fg(theme.accent_color),
            ));
        }
        if let Some((done, total)) = task.subtask_progress() {
            let color = if done == total {
                theme.low_color
            } else {
                theme.accent_color
            };
            spans.push(Span::styled(
                format!("  [{}/{}]", done, total),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }
        if let Some((badge, color)) = due_badge(task, theme) {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(badge, Style::default().fg(color)));
        }
        active_list_items.push(ListItem::new(Line::from(spans)));

        if is_active {
            let vis = task.visible_subtask_indices(ui.show_archived, now);
            let mut archived_header = false;
            for (row_idx, &si) in vis.iter().enumerate() {
                let sub = &task.subtasks[si];
                let archived = sub.is_archived(now);
                if archived && !archived_header {
                    active_list_items.push(ListItem::new(Line::from(Span::styled(
                        "      ─ archived ─",
                        Style::default()
                            .fg(theme.help_text_fg)
                            .add_modifier(Modifier::ITALIC),
                    ))));
                    archived_header = true;
                }
                if ui.selected_subtask == Some(row_idx) {
                    selected_pos = Some(active_list_items.len());
                }
                let checkbox = if sub.done { "[x] " } else { "[ ] " };
                let mut style = Style::default().fg(theme.base_fg);
                if sub.done {
                    style = Style::default()
                        .fg(theme.help_text_fg)
                        .add_modifier(Modifier::CROSSED_OUT);
                }
                active_list_items.push(ListItem::new(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(checkbox, Style::default().fg(theme.accent_color)),
                    Span::styled(sub.name.clone(), style),
                ])));
            }
        }
    }

    let mut list_state = ListState::default();
    list_state.select(selected_pos);

    let active_list = List::new(active_list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(list_title)
                .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(active_list, chunks[1], &mut list_state);

    let editing_sub = matches!(ui.input_mode, InputMode::EditingSubtask);
    let input_active = matches!(ui.input_mode, InputMode::Editing | InputMode::EditingSubtask);
    let input_value = if editing_sub {
        ui.subtask_input.as_str()
    } else {
        ui.current_input.as_str()
    };
    let input_title = if editing_sub {
        "Add Subtask"
    } else if ui.editing_task_index.is_some() {
        "Rename Task"
    } else {
        "New Task"
    };
    let input = Paragraph::new(input_value)
        .style(if input_active {
            Style::default().fg(theme.medium_color)
        } else {
            Style::default().fg(theme.base_fg)
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(input_title)
                .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        );
    frame.render_widget(input, chunks[2]);
    if input_active {
        frame.set_cursor_position((chunks[2].x + input_value.len() as u16 + 1, chunks[2].y + 1));
    }

    match ui.input_mode {
        InputMode::Filtering => {
            let filter_display = format!("/{}", ui.filter_input);
            frame.render_widget(
                Paragraph::new(filter_display.as_str())
                    .style(Style::default().fg(theme.medium_color))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title("Filter")
                            .style(Style::default().fg(theme.accent_color)),
                    ),
                chunks[3],
            );
            frame.set_cursor_position((
                chunks[3].x + 1 + 1 + ui.filter_input.len() as u16,
                chunks[3].y + 1,
            ));
        }
        _ => {
            let help_text = match ui.input_mode {
                InputMode::Editing | InputMode::EditingSubtask => {
                    " [Enter] Submit | [Esc] Cancel "
                }
                _ => {
                    if chunks[3].width > 80 {
                        " [↑/↓] Nav | [n]ew | [a]dd sub | [Spc] toggle sub | [Shift+A] archived | [e]dit | [p]riority | [D]ue | [Shift+E] notes | [s]ort | [/] Filter | [Enter] Done | [d]el | [q]uit "
                    } else {
                        " [↑/↓][n][a][Spc][A][e][p][D][E][s][/][Ent][d][q] "
                    }
                }
            };
            frame.render_widget(
                Paragraph::new(help_text)
                    .block(
                        Block::default()
                            .title("Controls")
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .style(Style::default().fg(theme.help_text_fg)),
                    )
                    .alignment(Alignment::Center),
                chunks[3],
            );
        }
    }
}
