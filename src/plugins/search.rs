//! URL opener — system browser by default; optional terminal-browser when installed.

use crate::herdr::{run_capture, run_herdr_ok, which_exists};
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
use std::process::Command;

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
    status_panel: String,
}

impl SearchPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::new(vec![
                "Open default URL (system browser)".into(),
                "Open custom URL… (system browser)".into(),
                "Try terminal-browser (in-Herdr)".into(),
                "Show why Search may fail".into(),
                "Edit search config".into(),
            ]),
            config: SearchConfig::default(),
            form: None,
            status_panel: String::new(),
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
        self.status_panel = self.diagnose();
    }

    fn save(&mut self, ctx: &mut PluginCtx) {
        if let Err(e) = write_toml(&Self::path(ctx), &self.config) {
            ctx.set_error(format!("save search config: {e}"));
        } else {
            ctx.set_status("search config saved");
        }
    }

    fn diagnose(&self) -> String {
        let bin_ok = which_exists(&self.config.binary) || PathBuf::from(&self.config.binary).exists();
        let mut lines = vec![
            "Lazy Search needs a browser backend.".into(),
            String::new(),
            format!(
                "terminal-browser binary: {}",
                if bin_ok {
                    "FOUND"
                } else {
                    "NOT FOUND (normal on many machines)"
                }
            ),
            format!("configured binary: {}", self.config.binary),
            format!("plugin action: {}", self.config.plugin_action),
            String::new(),
            "Why in-Herdr browse often fails:".into(),
            "- zenbu-labs.terminal-browser is Linux/macOS only (not Windows)".into(),
            "- that plugin is a separate install; Lazy Herd does not bundle it".into(),
            "- no terminal-browser on PATH → in-Herdr option cannot start".into(),
            String::new(),
            "What works everywhere: Open with system browser (first two menu items).".into(),
        ];
        if cfg!(windows) {
            lines.push("On Windows that uses: cmd /C start <url>".into());
        }
        lines.join("\n")
    }

    fn open_system_browser(&mut self, ctx: &mut PluginCtx, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            ctx.set_error("URL is empty");
            return;
        }
        let result = {
            #[cfg(windows)]
            {
                Command::new("cmd")
                    .args(["/C", "start", "", url])
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("open")
                    .arg(url)
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                Command::new("xdg-open")
                    .arg(url)
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        };
        match result {
            Ok(()) => {
                self.status_panel = format!("Opened in system browser:\n  {url}");
                ctx.set_status("opened system browser");
            }
            Err(e) => ctx.set_error(format!("system browser failed: {e}")),
        }
    }

    fn open_terminal_browser(&mut self, ctx: &mut PluginCtx, url: &str) {
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

        match run_herdr_ok(&["plugin", "action", "invoke", &self.config.plugin_action]) {
            Ok(_) => ctx.set_status("invoked terminal-browser plugin action"),
            Err(e) => {
                self.status_panel = self.diagnose();
                ctx.set_error(format!(
                    "terminal-browser unavailable ({e}). Use 'system browser' instead — see panel."
                ));
            }
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
        "Open docs/URLs. System browser works everywhere. In-Herdr terminal-browser is optional and usually Linux/macOS only."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        ctx.set_status("Enter run · prefer system browser on Windows · Esc back");
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
                        self.open_system_browser(ctx, &url);
                    } else {
                        self.config.binary = vals.first().cloned().unwrap_or_default();
                        self.config.default_url = vals.get(1).cloned().unwrap_or_default();
                        self.config.plugin_action = vals.get(2).cloned().unwrap_or_default();
                        self.save(ctx);
                        self.form = None;
                        self.status_panel = self.diagnose();
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
            KeyCode::Enter => {
                match self.list.selected() {
                    Some(0) => {
                        let url = self.config.default_url.clone();
                        self.open_system_browser(ctx, &url);
                    }
                    Some(1) => {
                        self.form = Some(FormState::new(
                            "Open URL",
                            vec![FormField::new("url").with_value(&self.config.default_url)],
                        ));
                    }
                    Some(2) => {
                        let url = self.config.default_url.clone();
                        self.open_terminal_browser(ctx, &url);
                    }
                    Some(3) => {
                        self.status_panel = self.diagnose();
                        ctx.set_status("diagnostics refreshed");
                    }
                    Some(4) => {
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        draw_select_list(
            frame,
            chunks[0],
            "Lazy Search",
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.status_panel.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Status / why").borders(Borders::ALL)),
            chunks[1],
        );
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
    }
}
