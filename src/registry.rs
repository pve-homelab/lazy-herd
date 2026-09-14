//! Sub-plugin registration surface.

use crate::storage::Paths;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    None,
    /// Leave the current sub-plugin and return to the main menu.
    Back,
    /// Quit the entire Lazy Herd TUI (closes the Herdr pane).
    #[allow(dead_code)] // reserved for future force-quit from nested screens
    Quit,
}

/// Shared runtime context passed into every sub-plugin.
pub struct PluginCtx {
    pub paths: Paths,
    pub status: String,
    pub error: Option<String>,
}

impl PluginCtx {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            status: String::new(),
            error: None,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.error = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }
}

pub trait SubPlugin {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// Called when the user enters this sub-plugin from the menu.
    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        let _ = ctx;
    }
    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction;
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &PluginCtx);
}

pub struct PluginMeta {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub struct PluginRegistry {
    plugins: Vec<Box<dyn SubPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a sub-plugin. Order here = menu order.
    pub fn register(&mut self, plugin: Box<dyn SubPlugin>) {
        self.plugins.push(plugin);
    }

    pub fn metas(&self) -> Vec<PluginMeta> {
        self.plugins
            .iter()
            .map(|p| PluginMeta {
                id: p.id(),
                title: p.title(),
                description: p.description(),
            })
            .collect()
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Box<dyn SubPlugin>> {
        self.plugins.iter_mut().find(|p| p.id() == id)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ids(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.id()).collect()
    }

}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    struct Dummy;
    impl SubPlugin for Dummy {
        fn id(&self) -> &'static str {
            "dummy"
        }
        fn title(&self) -> &'static str {
            "Dummy"
        }
        fn description(&self) -> &'static str {
            "test"
        }
        fn handle(&mut self, _ctx: &mut PluginCtx, _key: KeyEvent) -> NavAction {
            NavAction::Back
        }
        fn draw(&self, _frame: &mut Frame, _area: Rect, _ctx: &PluginCtx) {}
    }

    #[test]
    fn registry_preserves_order() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(Dummy));
        assert_eq!(reg.ids(), vec!["dummy"]);
        let _ = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    }
}
