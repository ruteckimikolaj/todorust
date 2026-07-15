use chrono::Local;
use ratatui::{prelude::*, widgets::*};

use super::create_big_text_paragraph;
use crate::app::App;
use crate::settings::Theme;

pub fn draw_dashboard(frame: &mut Frame, app: &App, theme: &Theme) {
    let base_style = Style::default().bg(theme.base_bg).fg(theme.base_fg);

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

    // A splash of colour: red when something is overdue, green when the list is clear.
    let (accent_color, panel_bg) = if overdue > 0 {
        (theme.high_color, theme.high_bg)
    } else if open == 0 {
        (theme.done_color, theme.low_bg)
    } else {
        (theme.accent_color, theme.medium_bg)
    };
    let accent_style = Style::default().fg(accent_color);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(frame.area());

    frame.render_widget(
        Block::default()
            .title(" T O D O R U S T ")
            .title_alignment(Alignment::Center)
            .style(base_style),
        main_layout[0],
    );

    let panel = Block::default()
        .title(" Open Tasks ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(accent_style)
        .style(Style::default().bg(panel_bg));
    let panel_area = panel.inner(main_layout[1]);
    frame.render_widget(panel, main_layout[1]);

    let center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(panel_area);

    // Big block-art counter of open tasks
    frame.render_widget(
        create_big_text_paragraph(&open.to_string(), accent_style),
        center[1],
    );

    let info = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .horizontal_margin(4)
        .split(center[2]);

    frame.render_widget(
        Paragraph::new(Local::now().format("%A, %-d %B %Y").to_string())
            .style(accent_style.add_modifier(Modifier::ITALIC | Modifier::BOLD))
            .alignment(Alignment::Center),
        info[1],
    );

    let overdue_style = if overdue > 0 {
        Style::default()
            .fg(theme.high_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.help_text_fg)
    };
    frame.render_widget(
        Paragraph::new(format!("⚠ Overdue: {}", overdue))
            .style(overdue_style)
            .alignment(Alignment::Center),
        info[2],
    );
    frame.render_widget(
        Paragraph::new(format!("◷ Due today: {}", due_today))
            .style(Style::default().fg(theme.medium_color))
            .alignment(Alignment::Center),
        info[3],
    );
    frame.render_widget(
        Paragraph::new(format!("✓ Completed today: {}", done_today))
            .style(Style::default().fg(theme.done_color))
            .alignment(Alignment::Center),
        info[4],
    );

    let help_text = if main_layout[2].width > 80 {
        " [Tab] Tasks | [o]ptions | [q]uit "
    } else {
        " [Tab] [o] [q] "
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
        main_layout[2],
    );
}
