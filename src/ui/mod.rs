//! Shared TUI widgets for list/form/confirm flows.

mod confirm;
mod form;
mod list;

pub use confirm::{ConfirmDelete, ConfirmResult};
pub use form::{FormField, FormResult, FormState};
pub use list::ScrollList;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn draw_help_footer(frame: &mut Frame, area: Rect, text: &str) {
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(p, area);
}

pub fn draw_status(frame: &mut Frame, area: Rect, status: &str, error: Option<&str>) {
    let (text, style) = if let Some(err) = error {
        (err, Style::default().fg(Color::Red))
    } else {
        (status, Style::default().fg(Color::Green))
    };
    frame.render_widget(Paragraph::new(text).style(style), area);
}

pub fn mask_secret(value: &str, revealed: bool) -> String {
    if revealed || value.is_empty() {
        value.to_string()
    } else {
        "•".repeat(value.chars().count().min(24).max(4))
    }
}

/// Draw a selectable list without needing `&mut ListState` (SubPlugin::draw is `&self`).
pub fn draw_select_list(frame: &mut Frame, area: Rect, title: &str, items: &[String], selected: Option<usize>) {
    let lines: Vec<Line> = if items.is_empty() {
        vec![Line::from("  (empty — press n to create)")]
    } else {
        items
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let marker = if selected == Some(i) { "▶ " } else { "  " };
                let style = if selected == Some(i) {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!("{marker}{s}"), style))
            })
            .collect()
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
