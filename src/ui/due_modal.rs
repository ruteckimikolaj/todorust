use ratatui::{prelude::*, widgets::*};

use super::centered_rect;
use crate::app::UiState;
use crate::settings::Theme;

pub fn draw_due_modal(frame: &mut Frame, ui: &UiState, theme: &Theme) {
    let area = centered_rect(50, 25, frame.area());
    frame.render_widget(Clear, area);

    let border_color = if ui.due_error {
        theme.high_color
    } else {
        theme.accent_color
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(" Set Due Date ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(border_color).bg(theme.base_bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(inner);

    frame.render_widget(
        Paragraph::new("Format: YYYY-MM-DD HH:MM  (empty = clear)")
            .style(Style::default().fg(theme.help_text_fg))
            .alignment(Alignment::Center),
        layout[0],
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().fg(theme.base_fg));
    frame.render_widget(
        Paragraph::new(ui.due_input.as_str()).block(input_block),
        layout[2],
    );
    frame.set_cursor_position((layout[2].x + ui.due_input.len() as u16 + 1, layout[2].y + 1));

    if ui.due_error {
        frame.render_widget(
            Paragraph::new("Invalid date — try e.g. 2026-07-20 14:30")
                .style(Style::default().fg(theme.high_color))
                .alignment(Alignment::Center),
            layout[3],
        );
    }

    frame.render_widget(
        Paragraph::new(" [Enter] Save | [Esc] Cancel ")
            .style(Style::default().fg(theme.help_text_fg))
            .alignment(Alignment::Center),
        layout[4],
    );
}
