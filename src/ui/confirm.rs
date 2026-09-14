use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::centered_rect;

#[derive(Debug)]
pub struct ConfirmDelete {
    pub item_name: String,
    pub typed: String,
}

impl ConfirmDelete {
    pub fn new(item_name: impl Into<String>) -> Self {
        Self {
            item_name: item_name.into(),
            typed: String::new(),
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> ConfirmResult {
        match key.code {
            KeyCode::Esc => ConfirmResult::Cancel,
            KeyCode::Enter => {
                if self.typed == self.item_name {
                    ConfirmResult::Confirmed
                } else {
                    ConfirmResult::Continue
                }
            }
            KeyCode::Backspace => {
                self.typed.pop();
                ConfirmResult::Continue
            }
            KeyCode::Char(c) => {
                self.typed.push(c);
                ConfirmResult::Continue
            }
            _ => ConfirmResult::Continue,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(60, 30, area);
        frame.render_widget(Clear, popup);
        let ok = self.typed == self.item_name;
        let border = if ok { Color::Red } else { Color::Yellow };
        let block = Block::default()
            .title("Confirm delete")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let body = format!(
            "Type the name to delete:\n  {}\n\n> {}\n\nEnter confirms when names match · Esc cancels",
            self.item_name, self.typed
        );
        frame.render_widget(Paragraph::new(body), inner);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    Continue,
    Confirmed,
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn requires_exact_name() {
        let mut c = ConfirmDelete::new("alpha");
        for ch in "alph".chars() {
            assert_eq!(
                c.handle(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
                ConfirmResult::Continue
            );
        }
        assert_eq!(
            c.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ConfirmResult::Continue
        );
        assert_eq!(
            c.handle(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            ConfirmResult::Continue
        );
        assert_eq!(
            c.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ConfirmResult::Confirmed
        );
    }
}
