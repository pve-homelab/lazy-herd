use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::centered_rect;

#[derive(Debug, Clone)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub secret: bool,
}

impl FormField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            secret: false,
        }
    }

    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }
}

#[derive(Debug)]
pub struct FormState {
    pub title: String,
    pub fields: Vec<FormField>,
    pub focus: usize,
    pub reveal_secrets: bool,
}

impl FormState {
    pub fn new(title: impl Into<String>, fields: Vec<FormField>) -> Self {
        Self {
            title: title.into(),
            fields,
            focus: 0,
            reveal_secrets: false,
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> FormResult {
        match key.code {
            KeyCode::Esc => FormResult::Cancel,
            KeyCode::Enter => FormResult::Submit,
            KeyCode::Tab | KeyCode::Down => {
                if !self.fields.is_empty() {
                    self.focus = (self.focus + 1) % self.fields.len();
                }
                FormResult::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                if !self.fields.is_empty() {
                    self.focus = if self.focus == 0 {
                        self.fields.len() - 1
                    } else {
                        self.focus - 1
                    };
                }
                FormResult::Continue
            }
            KeyCode::Char('r') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.reveal_secrets = !self.reveal_secrets;
                FormResult::Continue
            }
            KeyCode::Backspace => {
                if let Some(field) = self.fields.get_mut(self.focus) {
                    field.value.pop();
                }
                FormResult::Continue
            }
            KeyCode::Char(c) => {
                if let Some(field) = self.fields.get_mut(self.focus) {
                    field.value.push(c);
                }
                FormResult::Continue
            }
            _ => FormResult::Continue,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(72, 55, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let mut lines = Vec::new();
        for (i, field) in self.fields.iter().enumerate() {
            let marker = if i == self.focus { ">" } else { " " };
            let display = if field.secret && !self.reveal_secrets {
                "•".repeat(field.value.chars().count().min(32))
            } else {
                field.value.clone()
            };
            let style = if i == self.focus {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{marker} {}: {display}", field.label),
                style,
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Tab fields · Enter save · Esc cancel · Ctrl+r reveal secrets",
        ));
        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub fn values(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.value.clone()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormResult {
    Continue,
    Submit,
    Cancel,
}
