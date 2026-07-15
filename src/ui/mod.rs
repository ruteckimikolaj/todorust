pub mod dashboard;
pub mod details;
pub mod edit_sheet;
pub mod notes_modal;
pub mod settings;
pub mod statistics;
pub mod task_list;

pub use dashboard::draw_dashboard;
pub use details::draw_task_details;
pub use edit_sheet::draw_edit_sheet;
pub use notes_modal::draw_notes_modal;
pub use settings::draw_settings;
pub use statistics::draw_statistics;
pub use task_list::draw_task_list;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// 5-row block-art for digits and a few symbols. Shared by the dashboard.
pub fn get_char_art(c: char) -> Vec<&'static str> {
    match c {
        '0' => vec!["███", "█ █", "█ █", "█ █", "███"],
        '1' => vec![" █ ", "██ ", " █ ", " █ ", "███"],
        '2' => vec!["███", "  █", "███", "█  ", "███"],
        '3' => vec!["███", "  █", "███", "  █", "███"],
        '4' => vec!["█ █", "█ █", "███", "  █", "  █"],
        '5' => vec!["███", "█  ", "███", "  █", "███"],
        '6' => vec!["███", "█  ", "███", "█ █", "███"],
        '7' => vec!["███", "  █", "  █", "  █", "  █"],
        '8' => vec!["███", "█ █", "███", "█ █", "███"],
        '9' => vec!["███", "█ █", "███", "  █", "███"],
        ':' => vec!["   ", " █ ", "   ", " █ ", "   "],
        _ => vec!["   ", "   ", "   ", "   ", "   "],
    }
}

pub fn create_big_text_paragraph<'a>(text: &str, style: Style) -> Paragraph<'a> {
    let mut lines: Vec<Line> = vec![Line::from(""); 5];
    for character in text.chars() {
        let art = get_char_art(character);
        for (i, art_line) in art.iter().enumerate() {
            lines[i].spans.push(Span::styled(*art_line, style));
            lines[i].spans.push(Span::raw(" "));
        }
    }
    Paragraph::new(lines).alignment(Alignment::Center)
}
