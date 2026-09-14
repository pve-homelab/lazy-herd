use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;

#[derive(Debug, Default)]
pub struct ScrollList {
    pub state: ListState,
    pub items: Vec<String>,
}

impl ScrollList {
    pub fn new(items: Vec<String>) -> Self {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        Self { state, items }
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        let prev = self.state.selected();
        self.items = items;
        if self.items.is_empty() {
            self.state.select(None);
        } else {
            let idx = prev.unwrap_or(0).min(self.items.len() - 1);
            self.state.select(Some(idx));
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn handle_nav(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev();
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
                true
            }
            KeyCode::Home => {
                if !self.items.is_empty() {
                    self.state.select(Some(0));
                }
                true
            }
            KeyCode::End => {
                if !self.items.is_empty() {
                    self.state.select(Some(self.items.len() - 1));
                }
                true
            }
            _ => false,
        }
    }

    fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(0) => self.items.len() - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.state.select(Some(i));
    }
}
