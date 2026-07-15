use chrono::{Local, Utc};
use ratatui::{prelude::*, widgets::*};

use crate::app::ui_state::parse_quick_add;
use crate::app::{App, InputMode, Priority, SectionTone, Task, UiState};
use crate::settings::Theme;

fn priority_color(p: Priority, theme: &Theme) -> Color {
    match p {
        Priority::Low => theme.low_color,
        Priority::Medium => theme.medium_color,
        Priority::High => theme.high_color,
    }
}

fn section_color(tone: SectionTone, theme: &Theme) -> Color {
    match tone {
        SectionTone::Overdue | SectionTone::High => theme.high_color,
        SectionTone::Today | SectionTone::Medium => theme.medium_color,
        SectionTone::Upcoming => theme.accent_color,
        SectionTone::Low => theme.low_color,
        SectionTone::NoDate | SectionTone::Neutral => theme.help_text_fg,
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

/// Compact one-line agenda strip shown above the task list (Phase 3 home view).
fn draw_today_strip(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let today = Local::now().date_naive();
    let open = app.tasks.iter().filter(|t| !t.completed).count();
    let overdue = app.tasks.iter().filter(|t| t.is_overdue()).count();
    let due_today = app
        .tasks
        .iter()
        .filter(|t| {
            !t.completed
                && t.due_date
                    .is_some_and(|d| d.with_timezone(&Local).date_naive() == today)
        })
        .count();
    let done_today = app
        .tasks
        .iter()
        .filter(|t| {
            t.completion_date
                .is_some_and(|d| d.with_timezone(&Local).date_naive() == today)
        })
        .count();

    let date_str = Local::now().format("%A, %-d %B %Y").to_string();
    let overdue_style = if overdue > 0 {
        Style::default()
            .fg(theme.high_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.help_text_fg)
    };

    let spans = vec![
        Span::styled(
            format!(" {} ", date_str),
            Style::default()
                .fg(theme.accent_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme.help_text_fg)),
        Span::styled(format!("⚠ Overdue {}", overdue), overdue_style),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("● Today {}", due_today),
            Style::default().fg(theme.medium_color),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("✓ Done today {}", done_today),
            Style::default().fg(theme.done_color),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("◦ Open {}", open),
            Style::default().fg(theme.help_text_fg),
        ),
    ];

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" ✓ TODAY ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
            ),
        area,
    );
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

    draw_today_strip(frame, chunks[0], app, theme);

    let filter = ui.filter_input.to_lowercase();
    let sections = app.grouped_active_sections(&filter);

    let mut list_title = format!("Active Tasks — group: {}", app.grouping_mode.title());
    if !ui.filter_input.is_empty() {
        list_title.push_str(&format!(" [/{}]", ui.filter_input));
    }

    // Flattened render model: for each section, an optional header row plus
    // its task rows. Expanded parents inline their visible subtask rows.
    let now = Utc::now();
    let mut list_items: Vec<ListItem> = Vec::new();
    let mut selected_pos: Option<usize> = None;

    for section in &sections {
        // Only render a header when the section is labeled (Manual has none).
        if !section.label.is_empty() {
            let header_color = section_color(section.tone, theme);
            list_items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {}  ", section.label),
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", section.indices.len()),
                    Style::default().fg(theme.help_text_fg),
                ),
            ])));
        }

        for &i in &section.indices {
            let task = &app.tasks[i];
            let is_active = Some(i) == app.active_task_index;
            let is_marked = ui.marked_uuids.contains(&task.uuid);
            let marker = if is_active { "▶ " } else { "  " };

            if is_active && ui.selected_subtask.is_none() {
                selected_pos = Some(list_items.len());
            }

            let checkbox = if is_marked { " [•] " } else { " [ ] " };
            let checkbox_style = if is_marked {
                Style::default()
                    .fg(theme.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.base_fg)
            };
            let mut spans = vec![
                Span::styled(marker, Style::default().fg(theme.base_fg)),
                Span::styled(
                    "▍",
                    Style::default()
                        .fg(priority_color(task.priority, theme))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(checkbox, checkbox_style),
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
            if task.recurrence.is_some() {
                spans.push(Span::styled("  ↻", Style::default().fg(theme.accent_color)));
            }
            list_items.push(ListItem::new(Line::from(spans)));

            if is_active {
                let vis = task.visible_subtask_indices(ui.show_archived, now);
                let mut archived_header = false;
                for (row_idx, &si) in vis.iter().enumerate() {
                    let sub = &task.subtasks[si];
                    let archived = sub.is_archived(now);
                    if archived && !archived_header {
                        list_items.push(ListItem::new(Line::from(Span::styled(
                            "      ─ archived ─",
                            Style::default()
                                .fg(theme.help_text_fg)
                                .add_modifier(Modifier::ITALIC),
                        ))));
                        archived_header = true;
                    }
                    if ui.selected_subtask == Some(row_idx) {
                        selected_pos = Some(list_items.len());
                    }
                    let checkbox = if sub.done { "[x] " } else { "[ ] " };
                    let mut style = Style::default().fg(theme.base_fg);
                    if sub.done {
                        style = Style::default()
                            .fg(theme.help_text_fg)
                            .add_modifier(Modifier::CROSSED_OUT);
                    }
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::raw("     "),
                        Span::styled(checkbox, Style::default().fg(theme.accent_color)),
                        Span::styled(sub.name.clone(), style),
                    ])));
                }
            }
        }
    }

    if list_items.is_empty() {
        let msg = if ui.filter_input.is_empty() {
            "All clear ✓  Press [a] to add your first task."
        } else {
            "No matching tasks."
        };
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("  {}", msg),
            Style::default()
                .fg(theme.help_text_fg)
                .add_modifier(Modifier::ITALIC),
        ))));
    }

    let mut list_state = ListState::default();
    list_state.select(selected_pos);

    let active_list = List::new(list_items)
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
    let input_active = matches!(
        ui.input_mode,
        InputMode::Editing | InputMode::EditingSubtask
    );
    let input_value = if editing_sub {
        ui.subtask_input.as_str()
    } else {
        ui.current_input.as_str()
    };
    // Adding a new task? Show the quick-add parse preview inline in the title
    // so users see how their `@!^` tokens are being interpreted.
    let base_title = if editing_sub {
        "Add Subtask".to_string()
    } else if ui.editing_task_index.is_some() {
        "Rename Task".to_string()
    } else {
        "New Task".to_string()
    };
    let input_title = if matches!(ui.input_mode, InputMode::Editing)
        && ui.editing_task_index.is_none()
        && !input_value.trim().is_empty()
    {
        let parsed = parse_quick_add(input_value);
        let mut parts: Vec<String> = Vec::new();
        if let Some(p) = &parsed.project {
            parts.push(format!("@{}", p));
        }
        if let Some(pr) = parsed.priority {
            parts.push(format!("{} {}", pr.glyph(), pr.title()));
        }
        if let Some(due) = parsed.due {
            parts.push(format!(
                "◷ {}",
                due.with_timezone(&Local).format("%m-%d %H:%M")
            ));
        }
        if let Some(r) = parsed.recurrence {
            parts.push(format!("↻ {}", r.title()));
        }
        if parts.is_empty() {
            base_title
        } else {
            format!("{}  →  {}", base_title, parts.join("  "))
        }
    } else {
        base_title
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
        InputMode::Rescheduling => {
            let (color, title) = if ui.reschedule_error {
                (theme.high_color, "Reschedule — unknown shortcut")
            } else {
                (
                    theme.accent_color,
                    "Reschedule (today · tomorrow · mon..sun · nw · YYYY-MM-DD · empty=clear)",
                )
            };
            let display = format!("^{}", ui.reschedule_input);
            frame.render_widget(
                Paragraph::new(display.as_str())
                    .style(Style::default().fg(theme.medium_color))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title(title)
                            .style(Style::default().fg(color)),
                    ),
                chunks[3],
            );
            frame.set_cursor_position((
                chunks[3].x + 1 + 1 + ui.reschedule_input.len() as u16,
                chunks[3].y + 1,
            ));
        }
        _ => {
            let n_marked = ui.marked_uuids.len();
            let title = if n_marked > 0 {
                format!("Controls — {} selected", n_marked)
            } else {
                "Controls".to_string()
            };
            let help_text: std::borrow::Cow<'static, str> = match ui.input_mode {
                InputMode::Editing | InputMode::EditingSubtask => {
                    std::borrow::Cow::Borrowed(" [Enter] Submit | [Esc] Cancel ")
                }
                _ if n_marked > 0 => std::borrow::Cow::Owned(format!(
                    " {} marked | [Spc] done all | [d]el all | [1/2/3] prio | [v] toggle | [V] clear ",
                    n_marked
                )),
                _ => std::borrow::Cow::Borrowed(if chunks[3].width > 82 {
                    " [↑/↓] Nav | [a]dd | [Spc] done | [e]dit | [t/T/w/r] resched | [g]roup | [/] find | [v] mark | [d]el | [?] "
                } else {
                    " [↑↓][a][Spc][e][t/T/w/r][g][/][v][d][?][q] "
                }),
            };
            let title_style = if n_marked > 0 {
                Style::default()
                    .fg(theme.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.help_text_fg)
            };
            frame.render_widget(
                Paragraph::new(help_text.as_ref())
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .style(title_style),
                    )
                    .alignment(Alignment::Center),
                chunks[3],
            );
        }
    }

    // Overlays draw last so they sit on top of the list.
    if ui.confirm_delete {
        draw_confirm_delete(frame, app, theme);
    }
    if ui.show_help {
        draw_help_overlay(frame, theme);
    }
}

/// A rectangle centered in `area`, sized to `width`×`height` (clamped to fit).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Confirmation prompt shown before a destructive delete.
fn draw_confirm_delete(frame: &mut Frame, app: &App, theme: &Theme) {
    let name = app
        .active_task_index
        .and_then(|i| app.tasks.get(i))
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let area = centered_rect(50, 5, frame.area());
    frame.render_widget(Clear, area);
    let text = Text::from(vec![
        Line::from(Span::styled(
            format!("Delete \"{}\"?", name),
            Style::default()
                .fg(theme.base_fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[y] delete    [n/Esc] cancel",
            Style::default().fg(theme.help_text_fg),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Confirm ")
                .style(Style::default().fg(theme.high_color).bg(theme.base_bg)),
        ),
        area,
    );
}

/// Full keybinding reference — the home for every key kept off the short bar.
fn draw_help_overlay(frame: &mut Frame, theme: &Theme) {
    let rows = [
        ("↑/↓  j/k", "Move selection"),
        ("a", "Add task (tokens: @proj !prio ^date %repeat)"),
        ("+", "Add subtask"),
        ("Space / x", "Toggle done (task or selected subtask)"),
        ("Enter / e", "Edit sheet — name/project/priority/due/notes"),
        ("1 / 2 / 3", "Set priority Low/Med/High"),
        ("t / T / w", "Reschedule: today / tomorrow / next week"),
        ("r", "Reschedule prompt (today, mon..sun, YYYY-MM-DD, …)"),
        ("g", "Cycle grouping (Smart/Project/Priority/Manual)"),
        ("K / J", "Reorder task (Manual sort)"),
        ("v", "Mark / unmark task for bulk actions"),
        ("Shift+V", "Clear all marks"),
        ("Shift+A", "Toggle archived subtasks"),
        ("/", "Filter / search"),
        ("d / Del", "Delete (with confirm)"),
        ("o", "Settings"),
        ("Tab", "Switch view"),
        ("q", "Quit"),
    ];
    let mut lines: Vec<Line> = Vec::with_capacity(rows.len());
    for (keys, desc) in rows {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<11}", keys),
                Style::default()
                    .fg(theme.accent_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc.to_string(), Style::default().fg(theme.base_fg)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Press any key to close",
        Style::default()
            .fg(theme.help_text_fg)
            .add_modifier(Modifier::ITALIC),
    )));

    let area = centered_rect(52, lines.len() as u16 + 2, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Keybindings ")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        ),
        area,
    );
}
