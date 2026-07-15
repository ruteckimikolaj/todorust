use ratatui::{prelude::*, widgets::*};

use super::centered_rect;
use crate::app::ui_state::SheetField;
use crate::app::{Priority, UiState};
use crate::settings::Theme;

/// One label/value row; the focused field gets an accent marker and highlight.
fn field_line<'a>(label: &'a str, value: Vec<Span<'a>>, focused: bool, theme: &Theme) -> Line<'a> {
    let marker = if focused { "▶ " } else { "  " };
    let label_style = if focused {
        Style::default()
            .fg(theme.accent_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.help_text_fg)
    };
    let mut spans = vec![
        Span::styled(marker, Style::default().fg(theme.accent_color)),
        Span::styled(format!("{:<9}", label), label_style),
    ];
    spans.extend(value);
    Line::from(spans)
}

pub fn draw_edit_sheet(frame: &mut Frame, ui: &UiState, theme: &Theme) {
    let Some(sheet) = &ui.edit_sheet else {
        return;
    };

    let area = centered_rect(64, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Edit Task ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(theme.base_fg).bg(theme.base_bg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // name
            Constraint::Length(1), // project
            Constraint::Length(1), // priority
            Constraint::Length(1), // due
            Constraint::Length(1), // due error / spacer
            Constraint::Length(1), // notes label
            Constraint::Min(3),    // notes body
            Constraint::Length(1), // footer
        ])
        .split(inner);

    let base = Style::default().fg(theme.base_fg);

    // Name
    frame.render_widget(
        Paragraph::new(field_line(
            "Name",
            vec![Span::styled(sheet.name.clone(), base)],
            sheet.field == SheetField::Name,
            theme,
        )),
        rows[0],
    );

    // Project
    let project_display = if sheet.project.is_empty() {
        Span::styled("(none)", Style::default().fg(theme.help_text_fg))
    } else {
        Span::styled(format!("@{}", sheet.project.trim_start_matches('@')), Style::default().fg(theme.accent_color))
    };
    frame.render_widget(
        Paragraph::new(field_line(
            "Project",
            vec![project_display],
            sheet.field == SheetField::Project,
            theme,
        )),
        rows[1],
    );

    // Priority radio
    let radio = |label: &str, p: Priority| {
        let dot = if sheet.priority == p { "(•) " } else { "( ) " };
        let color = match p {
            Priority::Low => theme.low_color,
            Priority::Medium => theme.medium_color,
            Priority::High => theme.high_color,
        };
        let modifier = if sheet.priority == p {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        Span::styled(
            format!("{}{}  ", dot, label),
            Style::default().fg(color).add_modifier(modifier),
        )
    };
    frame.render_widget(
        Paragraph::new(field_line(
            "Priority",
            vec![
                radio("Low", Priority::Low),
                radio("Med", Priority::Medium),
                radio("High", Priority::High),
            ],
            sheet.field == SheetField::Priority,
            theme,
        )),
        rows[2],
    );

    // Due
    let due_display = if sheet.due.trim().is_empty() {
        Span::styled(
            "(empty = no due date)",
            Style::default().fg(theme.help_text_fg),
        )
    } else {
        Span::styled(sheet.due.clone(), base)
    };
    frame.render_widget(
        Paragraph::new(field_line(
            "Due",
            vec![due_display],
            sheet.field == SheetField::Due,
            theme,
        )),
        rows[3],
    );
    if sheet.due_error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "           invalid — use YYYY-MM-DD HH:MM",
                Style::default().fg(theme.high_color),
            ))),
            rows[4],
        );
    }

    // Notes label + body (the textarea is rendered live so it stays editable).
    frame.render_widget(
        Paragraph::new(field_line(
            "Notes",
            vec![],
            sheet.field == SheetField::Notes,
            theme,
        )),
        rows[5],
    );
    let notes_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(if sheet.field == SheetField::Notes {
            Style::default().fg(theme.accent_color)
        } else {
            Style::default().fg(theme.help_text_fg)
        });
    let notes_inner = notes_block.inner(rows[6]);
    frame.render_widget(notes_block, rows[6]);
    frame.render_widget(&sheet.notes, notes_inner);

    // Footer
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " [Tab] next  [Shift+Tab] prev  [Ctrl+S] save  [Esc] cancel",
            Style::default().fg(theme.help_text_fg),
        ))),
        rows[7],
    );
}
