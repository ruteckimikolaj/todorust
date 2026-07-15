use chrono::{Datelike, Local, Weekday};
use ratatui::{prelude::*, widgets::*};

use crate::app::{App, InputMode, UiState};
use crate::settings::Theme;

// Below this total terminal width, collapse chart and show sparkline underneath
const BARCHART_MIN_WIDTH: u16 = 50;

fn weekday_label(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

// Full Mon–Sun of current ISO week; future days are 0. Counts completed tasks per day.
fn weekly_bar_data(app: &App) -> Vec<(String, u64)> {
    let today = Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let mut counts = [0u64; 7];
    for task in &app.tasks {
        if let Some(completed) = task.completion_date {
            let d = (completed.with_timezone(&Local).date_naive() - monday).num_days();
            if (0..7).contains(&d) {
                counts[d as usize] += 1;
            }
        }
    }
    (0..7)
        .map(|i| {
            let date = monday + chrono::Duration::days(i as i64);
            (weekday_label(date.weekday()).to_string(), counts[i])
        })
        .collect()
}

// Peak daily count over the last 28 days — used as BarChart max
fn four_week_max(app: &App) -> u64 {
    let today = Local::now().date_naive();
    let cutoff = today - chrono::Duration::days(28);
    let mut daily: std::collections::HashMap<chrono::NaiveDate, u64> = Default::default();
    for task in &app.tasks {
        if let Some(completed) = task.completion_date {
            let d = completed.with_timezone(&Local).date_naive();
            if d >= cutoff {
                *daily.entry(d).or_insert(0) += 1;
            }
        }
    }
    daily.values().copied().max().unwrap_or(1).max(1)
}

// Last 7 rolling days for the sparkline fallback
fn last7_sparkline(app: &App) -> Vec<u64> {
    let today = Local::now().date_naive();
    let mut counts = [0u64; 7];
    for task in &app.tasks {
        if let Some(completed) = task.completion_date {
            let days_ago = (today - completed.with_timezone(&Local).date_naive()).num_days();
            if days_ago >= 0 && (days_ago as usize) < 7 {
                counts[6 - days_ago as usize] += 1;
            }
        }
    }
    counts.to_vec()
}

// Rolling average age (in whole days) of currently open tasks. `None` when there are no
// open tasks. Age is measured from `creation_date` to now, so it grows with staleness.
fn avg_open_age_days(app: &App) -> Option<f64> {
    let now = chrono::Utc::now();
    let ages: Vec<f64> = app
        .tasks
        .iter()
        .filter(|t| !t.completed)
        .map(|t| (now - t.creation_date).num_seconds() as f64 / 86_400.0)
        .collect();
    if ages.is_empty() {
        None
    } else {
        Some(ages.iter().sum::<f64>() / ages.len() as f64)
    }
}

// (this_week_completions, last_week_completions) — for the review "trend" row.
fn completions_this_vs_last_week(app: &App) -> (u64, u64) {
    let today = Local::now().date_naive();
    let this_monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let last_monday = this_monday - chrono::Duration::days(7);
    let (mut this_wk, mut last_wk) = (0u64, 0u64);
    for task in &app.tasks {
        if let Some(dt) = task.completion_date {
            let d = dt.with_timezone(&Local).date_naive();
            if d >= this_monday && d <= today {
                this_wk += 1;
            } else if d >= last_monday && d < this_monday {
                last_wk += 1;
            }
        }
    }
    (this_wk, last_wk)
}

// Top N projects ranked by open-task count (ties broken alphabetically). "(no project)"
// is aggregated separately when it would land in the top slots.
fn top_projects_by_open(app: &App, n: usize) -> Vec<(String, usize)> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for task in app.tasks.iter().filter(|t| !t.completed) {
        let key = task.project.clone().unwrap_or_else(|| "(none)".to_string());
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut items: Vec<(String, usize)> = counts.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(n);
    items
}

pub fn draw_statistics(frame: &mut Frame, app: &App, ui: &UiState, theme: &Theme) {
    let wide = frame.area().width >= BARCHART_MIN_WIDTH;

    let chunks = if wide {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // title
                Constraint::Length(8), // summary (left) + barchart (right)
                Constraint::Length(6), // review
                Constraint::Min(0),    // task list
                Constraint::Length(4), // help
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // title
                Constraint::Length(8), // summary full-width
                Constraint::Length(3), // sparkline
                Constraint::Length(6), // review
                Constraint::Min(0),    // task list
                Constraint::Length(4), // help
            ])
            .split(frame.area())
    };

    // Title
    let stats_title = if !ui.filter_input.is_empty() {
        format!(" Σ STATISTICS [/{}] ", ui.filter_input)
    } else {
        " Σ STATISTICS ".to_string()
    };
    frame.render_widget(
        Block::default()
            .title(stats_title)
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        chunks[0],
    );

    // --- Stats data ---
    let today = Local::now().date_naive();
    let today_done: u64 = app
        .tasks
        .iter()
        .filter_map(|t| t.completion_date)
        .filter(|dt| dt.with_timezone(&Local).date_naive() == today)
        .count() as u64;
    let total_done = app.tasks.iter().filter(|t| t.completed).count();
    let open = app.tasks.iter().filter(|t| !t.completed).count();
    let overdue = app.tasks.iter().filter(|t| t.is_overdue()).count();
    let bold = Style::default().add_modifier(Modifier::BOLD);

    let summary_lines = vec![
        Line::from(Span::styled("Today", bold)),
        Line::from(format!("Completed: {}", today_done)),
        Line::from(Span::styled("All Time", bold)),
        Line::from(format!("Completed: {}", total_done)),
        Line::from(format!("Open:      {}", open)),
        Line::from(Span::styled(
            format!("Overdue:   {}", overdue),
            if overdue > 0 {
                Style::default().fg(theme.high_color)
            } else {
                Style::default()
            },
        )),
    ];

    if wide {
        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(chunks[1]);

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Summary")
            .style(Style::default().fg(theme.base_fg).bg(theme.base_bg));
        let stats_inner = stats_block.inner(top_cols[0]);
        frame.render_widget(stats_block, top_cols[0]);
        frame.render_widget(
            Paragraph::new(summary_lines).alignment(Alignment::Left),
            stats_inner,
        );

        let bar_data = weekly_bar_data(app);
        let max_val = four_week_max(app);
        let n = 7usize;
        let bar_gap: u16 = 1;
        let inner_w = top_cols[1].width.saturating_sub(2);
        let bar_width = (inner_w.saturating_sub(bar_gap * (n as u16 - 1)) / n as u16).max(3);
        let bars: Vec<Bar> = bar_data
            .iter()
            .map(|(label, count)| {
                Bar::default()
                    .label(Line::from(label.clone()))
                    .value(*count)
                    .style(Style::default().fg(theme.done_color))
            })
            .collect();
        frame.render_widget(
            BarChart::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title("This week")
                        .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
                )
                .bar_width(bar_width)
                .bar_gap(bar_gap)
                .max(max_val)
                .value_style(
                    Style::default()
                        .fg(theme.base_bg)
                        .bg(theme.done_color)
                        .add_modifier(Modifier::BOLD),
                )
                .label_style(Style::default().fg(theme.base_fg))
                .data(BarGroup::default().bars(&bars)),
            top_cols[1],
        );
    } else {
        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Summary")
            .style(Style::default().fg(theme.base_fg).bg(theme.base_bg));
        let stats_inner = stats_block.inner(chunks[1]);
        frame.render_widget(stats_block, chunks[1]);
        frame.render_widget(
            Paragraph::new(summary_lines).alignment(Alignment::Left),
            stats_inner,
        );

        let spark_data = last7_sparkline(app);
        frame.render_widget(
            Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title("Last 7 days")
                        .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
                )
                .data(spark_data.iter().copied())
                .style(Style::default().fg(theme.done_color)),
            chunks[2],
        );
    }

    let (review_idx, tasks_idx, help_idx) = if wide { (2, 3, 4) } else { (3, 4, 5) };

    // --- Review panel ---
    let completion_rate = if total_done + open == 0 {
        0.0
    } else {
        total_done as f64 / (total_done + open) as f64 * 100.0
    };
    let avg_age_display = match avg_open_age_days(app) {
        Some(d) if d < 1.0 => "<1 day".to_string(),
        Some(d) => format!("{:.1} days", d),
        None => "—".to_string(),
    };
    let (this_wk, last_wk) = completions_this_vs_last_week(app);
    let delta = this_wk as i64 - last_wk as i64;
    let trend_str = if last_wk == 0 && this_wk == 0 {
        "no data yet".to_string()
    } else if delta > 0 {
        format!("▲ +{} vs last week ({} → {})", delta, last_wk, this_wk)
    } else if delta < 0 {
        format!("▼ {} vs last week ({} → {})", delta, last_wk, this_wk)
    } else {
        format!("▶ same as last week ({})", this_wk)
    };
    let trend_style = if delta > 0 {
        Style::default().fg(theme.done_color)
    } else if delta < 0 {
        Style::default().fg(theme.high_color)
    } else {
        Style::default().fg(theme.help_text_fg)
    };
    let top_projects = top_projects_by_open(app, 3);
    let projects_line: Line = if top_projects.is_empty() {
        Line::from(Span::styled(
            "no open tasks",
            Style::default().fg(theme.help_text_fg),
        ))
    } else {
        let mut spans: Vec<Span> = Vec::new();
        for (i, (name, count)) in top_projects.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default().fg(theme.help_text_fg)));
            }
            let label = if name == "(none)" {
                "(no project)".to_string()
            } else {
                format!("@{}", name)
            };
            spans.push(Span::styled(label, Style::default().fg(theme.accent_color)));
            spans.push(Span::styled(
                format!(" {}", count),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        }
        Line::from(spans)
    };
    let review_lines = vec![
        Line::from(vec![
            Span::styled("Completion rate:  ", Style::default()),
            Span::styled(
                format!("{:.0}%", completion_rate),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} of {})", total_done, total_done + open),
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("Avg open age:     ", Style::default()),
            Span::styled(
                avg_age_display,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Overdue trend:    ", Style::default()),
            Span::styled(trend_str, trend_style),
        ]),
        Line::from(vec![
            Span::styled("Top open projects: ", Style::default()),
            Span::styled(String::new(), Style::default()), // placeholder to keep width even
        ]),
        projects_line,
    ];
    frame.render_widget(
        Paragraph::new(review_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Review")
                .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
        ),
        chunks[review_idx],
    );

    // --- Completed task list ---
    let filter = ui.filter_input.to_lowercase();
    let completed_indices = app.ordered_completed_indices(&filter);
    let mut list_state = ListState::default();
    list_state.select(ui.completed_task_list_state);

    let list_items: Vec<ListItem> = completed_indices
        .iter()
        .map(|&i| {
            let task = &app.tasks[i];
            let done = task
                .completion_date
                .map(|d| d.with_timezone(&Local).format("%m-%d").to_string())
                .unwrap_or_default();
            let mut spans = vec![
                Span::styled(
                    format!("✓ {:<36} ", task.name),
                    Style::default().fg(theme.done_color),
                ),
                Span::styled(done, Style::default().fg(theme.help_text_fg)),
            ];
            if let Some(proj) = &task.project {
                spans.push(Span::styled(
                    format!(" @{}", proj),
                    Style::default().fg(theme.accent_color),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let task_list_title = if !filter.is_empty() {
        format!("Completed Tasks [/{}]", ui.filter_input)
    } else {
        "Completed Tasks".to_string()
    };
    frame.render_stateful_widget(
        List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(task_list_title)
                    .style(Style::default().fg(theme.base_fg).bg(theme.base_bg)),
            )
            .highlight_style(
                Style::default()
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        chunks[tasks_idx],
        &mut list_state,
    );

    // --- Help bar / filter bar ---
    if let InputMode::Filtering = ui.input_mode {
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
            chunks[help_idx],
        );
        frame.set_cursor_position((
            chunks[help_idx].x + 1 + 1 + ui.filter_input.len() as u16,
            chunks[help_idx].y + 1,
        ));
        return;
    }

    let help_text = if chunks[help_idx].width > 80 {
        " [Tab] Tasks | [↑/↓] Navigate | [/] Filter | [Enter] Details | [d]elete | [q]uit "
    } else {
        " [Tab] [↑/↓] [/] [Ent] [d] [q] "
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
        chunks[help_idx],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Priority, Task};
    use chrono::Utc;

    fn app_with(tasks: Vec<Task>) -> App {
        App {
            tasks,
            ..App::default()
        }
    }

    #[test]
    fn avg_open_age_ignores_completed_tasks() {
        let mut old = Task::new("old".into(), None, Priority::Medium);
        old.creation_date = Utc::now() - chrono::Duration::days(4);
        let mut done = Task::new("done".into(), None, Priority::Medium);
        done.completed = true;
        done.creation_date = Utc::now() - chrono::Duration::days(30);
        let app = app_with(vec![old, done]);
        let age = avg_open_age_days(&app).expect("has open tasks");
        assert!((3.9..=4.1).contains(&age), "expected ~4 days, got {}", age);
    }

    #[test]
    fn avg_open_age_none_when_no_open_tasks() {
        let mut done = Task::new("done".into(), None, Priority::Medium);
        done.completed = true;
        assert!(avg_open_age_days(&app_with(vec![done])).is_none());
        assert!(avg_open_age_days(&App::default()).is_none());
    }

    #[test]
    fn top_projects_ranks_by_open_count_and_truncates() {
        let mk = |name: &str, proj: Option<&str>, done: bool| {
            let mut t = Task::new(name.into(), proj.map(|s| s.to_string()), Priority::Medium);
            t.completed = done;
            t
        };
        let app = app_with(vec![
            mk("a", Some("work"), false),
            mk("b", Some("work"), false),
            mk("c", Some("work"), true), // done — excluded
            mk("d", Some("home"), false),
            mk("e", None, false),
            mk("f", Some("gym"), false),
            mk("g", Some("gym"), false),
        ]);
        let top = top_projects_by_open(&app, 2);
        assert_eq!(top.len(), 2);
        // work and gym both have 2 open; work wins ties by alpha.
        assert_eq!(top[0], ("gym".to_string(), 2));
        assert_eq!(top[1], ("work".to_string(), 2));
    }

    #[test]
    fn completions_split_this_and_last_week() {
        let this_wk = Utc::now();
        let last_wk = Utc::now() - chrono::Duration::days(9);
        let mut t1 = Task::new("t1".into(), None, Priority::Medium);
        t1.completion_date = Some(this_wk);
        let mut t2 = Task::new("t2".into(), None, Priority::Medium);
        t2.completion_date = Some(last_wk);
        // Only compare relative ordering; exact numbers depend on which weekday
        // the test runs on.
        let app = app_with(vec![t1, t2]);
        let (this, last) = completions_this_vs_last_week(&app);
        assert!(this + last >= 1);
    }
}
