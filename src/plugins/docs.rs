//! Built-in documentation guide for Lazy Herd commands and navigation.

use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const GUIDE: &str = r#"# Lazy Herd guide

## Open
- Action: `lazy-herd.open` (Windows: `lazy-herd.open-windows`)
- Pane: popup menu entrypoint `menu` / `menu-windows`
- Bind a key in ~/.config/herdr/config.toml:
  type = "plugin_action"
  command = "lazy-herd.open"

## Menu keys
- ↑/↓ or j/k — move between sub-plugins
- Enter — open highlighted sub-plugin
- ? — jump to this Docs guide
- Esc / q — quit and close the pane

## Inside a sub-plugin
- Esc — return to the main menu (does not quit)
- n — create (where supported)
- e — edit selected
- d — delete (type the name to confirm)
- Enter — activate / run selected action

## Sub-plugins
1. Lazy Git — remote repo list after account setup; Enter clones
2. Lazy Workspace — save/apply Herdr workspace templates
3. Lazy Secrets — masked PATs (use as Lazy Git secret_ref)
4. Lazy Connect — Enter opens manage pane (create/edit/delete/SSH)
5. Lazy Agents — agent presets (kind + master prompt)
6. Lazy Bootstrap — export / import / bootstrap
7. Lazy Search — terminal-browser launcher
8. Lazy Doctor — health checks
9. Lazy Config — settings

## Storage
Config: $HERDR_PLUGIN_CONFIG_DIR (or herdr plugin config-dir lazy-herd)
State:  $HERDR_PLUGIN_STATE_DIR

## Add a sub-plugin
1. Create src/plugins/mything.rs implementing SubPlugin
2. `mod mything;` in plugins/mod.rs
3. `reg.register(Box::new(mything::MyPlugin::new()));` in build_registry()
"#;

pub struct DocsPlugin {
    scroll: u16,
}

impl DocsPlugin {
    pub fn new() -> Self {
        Self { scroll: 0 }
    }
}

impl SubPlugin for DocsPlugin {
    fn id(&self) -> &'static str {
        "docs"
    }
    fn title(&self) -> &'static str {
        "Docs"
    }
    fn description(&self) -> &'static str {
        "Read the Lazy Herd command guide: menu keys, sub-plugin CRUD, storage paths, and how to add another sub-plugin."
    }

    fn handle(&mut self, _ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        match key.code {
            KeyCode::Esc => NavAction::Back,
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                NavAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                NavAction::None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                NavAction::None
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                NavAction::None
            }
            KeyCode::Home => {
                self.scroll = 0;
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let p = Paragraph::new(GUIDE)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0))
            .block(
                Block::default()
                    .title("Docs · j/k scroll · Esc back")
                    .borders(Borders::ALL),
            );
        frame.render_widget(p, area);
    }
}
