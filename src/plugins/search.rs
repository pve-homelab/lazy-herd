//! Terminal browser launcher + configuration.

use crate::herdr::{run_capture, run_herdr_ok, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchConfig {
    #[serde(default = "default_bin")]
    binary: String,
    #[serde(default)]
    default_url: String,
    #[serde(default = "default_plugin_action")]
    plugin_action: String,
}

fn default_bin() -> String {
    "terminal-browser".into()
}

fn default_plugin_action() -> String {
    "zenbu-labs.terminal-browser.open-split".into()
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            binary: default_bin(),
            default_url: "https://herdr.dev/docs/".into(),
            plugin_action: default_plugin_action(),
        }
    }
}

pub struct SearchPlugin {
    list: ScrollList,
    config: SearchConfig,
    form: Option<FormState>,
}

impl SearchPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::new(vec![
                "Open default URL".into(),
                "Open custom URL…".into(),
                "Invoke terminal-browser plugin action".into(),
                "Edit search config".into(),
            ]),
            config: SearchConfig::default(),
            form: None,
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("search.toml")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        match read_toml::<SearchConfig>(&Self::path(ctx)) {
            Ok(Some(c)) => self.config = c,
            Ok(None) => self.config = SearchConfig::default(),
            Err(e) => ctx.set_error(format!("search config: {e}")),
        }
    }

    fn save(&mut self, ctx: &mut PluginCtx) {
        if let Err(e) = write_toml(&Self::path(ctx), &self.config) {
            ctx.set_error(format!("save search config: {e}"));
        } else {
            ctx.set_status("search config saved");
        }
    }

    fn open_url(&mut self, ctx: &mut PluginCtx, url: &str) {
        let bin = &self.config.binary;
        if which_exists(bin) || PathBuf::from(bin).exists() {
            match run_capture(bin, &[url]) {
                Ok(out) if out.status.success() => {
                    ctx.set_status(format!("launched {bin}"));
                    return;
                }
                Ok(out) => {
                    ctx.set_error(format!(
                        "{bin} exited {}: {}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
                Err(e) => ctx.set_error(format!("{bin}: {e}")),
            }
        }
        // Fallback to herdr plugin action.
        match run_herdr_ok(&["plugin", "action", "invoke", &self.config.plugin_action]) {
            Ok(_) => ctx.set_status("invoked terminal-browser plugin action"),
            Err(e) => ctx.set_error(format!(
                "Could not launch browser ({e}). Install terminal-browser or set binary in config."
            )),
        }
    }
}

impl SubPlugin for SearchPlugin {
    fn id(&self) -> &'static str {
        "search"
    }
    fn title(&self) -> &'static str {
        "Lazy Search"
    }
    fn description(&self) -> &'static str {
        "In-terminal browser for docs, Grafana, etc. Configure the binary path or fall back to the zenbu-labs.terminal-browser Herdr plugin."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        ctx.set_status("Enter run · e edit config · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        if let Some(form) = self.form.as_mut() {
            match form.handle(key) {
                FormResult::Cancel => self.form = None,
                FormResult::Submit => {
                    let vals = form.values();
                    let title = form.title.clone();
                    if title.contains("URL") {
                        let url = vals.first().cloned().unwrap_or_default();
                        self.form = None;
                        self.open_url(ctx, &url);
                    } else {
                        self.config.binary = vals.first().cloned().unwrap_or_default();
                        self.config.default_url = vals.get(1).cloned().unwrap_or_default();
                        self.config.plugin_action = vals.get(2).cloned().unwrap_or_default();
                        self.save(ctx);
                        self.form = None;
                    }
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
            KeyCode::Char('e') => {
                self.form = Some(FormState::new(
                    "Search config",
                    vec![
                        FormField::new("binary").with_value(&self.config.binary),
                        FormField::new("default_url").with_value(&self.config.default_url),
                        FormField::new("plugin_action").with_value(&self.config.plugin_action),
                    ],
                ));
                NavAction::None
            }
            KeyCode::Enter => {
                match self.list.selected() {
                    Some(0) => {
                        let url = self.config.default_url.clone();
                        self.open_url(ctx, &url);
                    }
                    Some(1) => {
                        self.form = Some(FormState::new(
                            "Open URL",
                            vec![FormField::new("url").with_value(&self.config.default_url)],
                        ));
                    }
                    Some(2) => {
                        match run_herdr_ok(&[
                            "plugin",
                            "action",
                            "invoke",
                            &self.config.plugin_action,
                        ]) {
                            Ok(_) => ctx.set_status("plugin action invoked"),
                            Err(e) => ctx.set_error(e.to_string()),
                        }
                    }
                    Some(3) => {
                        self.form = Some(FormState::new(
                            "Search config",
                            vec![
                                FormField::new("binary").with_value(&self.config.binary),
                                FormField::new("default_url")
                                    .with_value(&self.config.default_url),
                                FormField::new("plugin_action")
                                    .with_value(&self.config.plugin_action),
                            ],
                        ));
                    }
                    _ => {}
                }
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        draw_select_list(
            frame,
            area,
            "Lazy Search",
            &self.list.items,
            self.list.selected(),
        );
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
    }
}
