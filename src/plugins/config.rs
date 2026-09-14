//! Lazy Herd settings + pointers into Herdr configuration.

use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_true")]
    confirm_destructive: bool,
    #[serde(default)]
    default_workspace_dir: String,
    #[serde(default)]
    notes: String,
}

fn default_theme() -> String {
    "cyan".into()
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            confirm_destructive: true,
            default_workspace_dir: "~/".into(),
            notes: String::new(),
        }
    }
}

pub struct ConfigPlugin {
    list: ScrollList,
    settings: Settings,
    form: Option<FormState>,
    info: String,
}

impl ConfigPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::new(vec![
                "Edit Lazy Herd settings".into(),
                "Show config/state paths".into(),
                "Herdr config hints".into(),
            ]),
            settings: Settings::default(),
            form: None,
            info: String::new(),
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("settings.toml")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        match read_toml::<Settings>(&Self::path(ctx)) {
            Ok(Some(s)) => self.settings = s,
            Ok(None) => self.settings = Settings::default(),
            Err(e) => ctx.set_error(format!("settings: {e}")),
        }
        self.info = format!(
            "theme: {}\nconfirm_destructive: {}\ndefault_workspace_dir: {}\nnotes: {}",
            self.settings.theme,
            self.settings.confirm_destructive,
            self.settings.default_workspace_dir,
            self.settings.notes
        );
    }
}

impl SubPlugin for ConfigPlugin {
    fn id(&self) -> &'static str {
        "config"
    }
    fn title(&self) -> &'static str {
        "Lazy Config"
    }
    fn description(&self) -> &'static str {
        "In-depth Lazy Herd configuration, storage paths, and hints for Herdr's own config.toml keybindings."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        ctx.set_status("Enter select · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        if let Some(form) = self.form.as_mut() {
            match form.handle(key) {
                FormResult::Cancel => self.form = None,
                FormResult::Submit => {
                    let vals = form.values();
                    self.settings.theme = vals.first().cloned().unwrap_or_else(default_theme);
                    self.settings.confirm_destructive = vals
                        .get(1)
                        .map(|s| {
                            matches!(
                                s.trim().to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "y"
                            )
                        })
                        .unwrap_or(true);
                    self.settings.default_workspace_dir =
                        vals.get(2).cloned().unwrap_or_default();
                    self.settings.notes = vals.get(3).cloned().unwrap_or_default();
                    if let Err(e) = write_toml(&Self::path(ctx), &self.settings) {
                        ctx.set_error(format!("save: {e}"));
                    } else {
                        ctx.set_status("settings saved");
                    }
                    self.form = None;
                    self.reload(ctx);
                }
                FormResult::Continue => {}
            }
            return NavAction::None;
        }

        if self.list.handle_nav(key) {
            return NavAction::None;
        }
        match key.code {
            KeyCode::Esc => NavAction::Back,
            KeyCode::Enter => {
                match self.list.selected() {
                    Some(0) => {
                        self.form = Some(FormState::new(
                            "Lazy Herd settings",
                            vec![
                                FormField::new("theme").with_value(&self.settings.theme),
                                FormField::new("confirm_destructive")
                                    .with_value(self.settings.confirm_destructive.to_string()),
                                FormField::new("default_workspace_dir")
                                    .with_value(&self.settings.default_workspace_dir),
                                FormField::new("notes").with_value(&self.settings.notes),
                            ],
                        ));
                    }
                    Some(1) => {
                        self.info = format!(
                            "HERDR_PLUGIN_CONFIG_DIR:\n  {}\n\nHERDR_PLUGIN_STATE_DIR:\n  {}\n\nTip: herdr plugin config-dir lazy-herd",
                            ctx.paths.config_dir.display(),
                            ctx.paths.state_dir.display()
                        );
                    }
                    Some(2) => {
                        self.info = r#"Herdr keybinding example (~/.config/herdr/config.toml):

[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "lazy-herd.open"
description = "open Lazy Herd"

Herdr Settings UI: prefix then the Settings binding (often prefix+s).
Plugin management: herdr plugin list | enable | disable | link
"#
                        .into();
                    }
                    _ => {}
                }
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        draw_select_list(
            frame,
            chunks[0],
            "Lazy Config",
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.info.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Info").borders(Borders::ALL)),
            chunks[1],
        );
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
    }
}
