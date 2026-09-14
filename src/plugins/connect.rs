//! SSH profile quick-connect list.

use crate::herdr::{run_herdr_ok, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{list_toml_stem_files, read_toml, write_toml};
use crate::ui::{ConfirmDelete, ConfirmResult};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SshProfile {
    name: String,
    host: String,
    user: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default)]
    identity_file: String,
    #[serde(default)]
    extra_args: String,
}

fn default_port() -> u16 {
    22
}

enum Mode {
    List,
    Form { editing: Option<String> },
    Confirm(ConfirmDelete),
}

pub struct ConnectPlugin {
    list: ScrollList,
    profiles: Vec<SshProfile>,
    mode: Mode,
    form: Option<FormState>,
}

impl ConnectPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            profiles: Vec::new(),
            mode: Mode::List,
            form: None,
        }
    }

    fn dir(ctx: &PluginCtx) -> Result<PathBuf, anyhow::Error> {
        ctx.paths.ensure_subdir(true, "connect")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        self.profiles.clear();
        match Self::dir(ctx).and_then(|d| list_toml_stem_files(&d)) {
            Ok(files) => {
                for path in files {
                    match read_toml::<SshProfile>(&path) {
                        Ok(Some(p)) => self.profiles.push(p),
                        Ok(None) => {}
                        Err(e) => ctx.set_error(format!("read {}: {e}", path.display())),
                    }
                }
            }
            Err(e) => ctx.set_error(format!("connect dir: {e}")),
        }
        self.profiles.sort_by(|a, b| a.name.cmp(&b.name));
        self.list.set_items(
            self.profiles
                .iter()
                .map(|p| format!("{}  {}@{}:{}", p.name, p.user, p.host, p.port))
                .collect(),
        );
    }

    fn save_one(&self, ctx: &mut PluginCtx, profile: &SshProfile) {
        match Self::dir(ctx) {
            Ok(dir) => {
                let path = dir.join(format!("{}.toml", sanitize(&profile.name)));
                if let Err(e) = write_toml(&path, profile) {
                    ctx.set_error(format!("save profile: {e}"));
                } else {
                    ctx.set_status(format!("saved {}", profile.name));
                }
            }
            Err(e) => ctx.set_error(e.to_string()),
        }
    }

    fn delete_named(&mut self, ctx: &mut PluginCtx, name: &str) {
        if let Ok(dir) = Self::dir(ctx) {
            let path = dir.join(format!("{}.toml", sanitize(name)));
            let _ = fs::remove_file(path);
        }
        self.reload(ctx);
        ctx.set_status(format!("deleted {name}"));
    }

    fn open_selected(&mut self, ctx: &mut PluginCtx) {
        let Some(i) = self.list.selected() else {
            return;
        };
        let Some(p) = self.profiles.get(i) else {
            return;
        };
        if !which_exists("ssh") {
            ctx.set_error("ssh not found on PATH");
            return;
        }
        let target = format!("{}@{}", p.user, p.host);
        let mut args = vec![
            "pane".into(),
            "split".into(),
            "--direction".into(),
            "right".into(),
            "--".into(),
            "ssh".into(),
            "-p".into(),
            p.port.to_string(),
        ];
        if !p.identity_file.is_empty() {
            args.push("-i".into());
            args.push(p.identity_file.clone());
        }
        if !p.extra_args.is_empty() {
            for part in p.extra_args.split_whitespace() {
                args.push(part.to_string());
            }
        }
        args.push(target);
        let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
        match run_herdr_ok(&str_args) {
            Ok(_) => ctx.set_status(format!("opening ssh {}", p.name)),
            Err(e) => {
                // Fallback: record the command for the user to run.
                ctx.set_error(format!(
                    "herdr pane split failed ({e}). Manual: ssh -p {} {}@{}",
                    p.port, p.user, p.host
                ));
            }
        }
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

impl SubPlugin for ConnectPlugin {
    fn id(&self) -> &'static str {
        "connect"
    }
    fn title(&self) -> &'static str {
        "Lazy Connect"
    }
    fn description(&self) -> &'static str {
        "Quick SSH profiles for agents and yourself. Create / edit / delete (type name to confirm). Enter opens ssh in a Herdr split when possible."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        ctx.set_status("n new · e edit · d delete · Enter open · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        match &mut self.mode {
            Mode::List => {
                if self.list.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => NavAction::Back,
                    KeyCode::Enter => {
                        self.open_selected(ctx);
                        NavAction::None
                    }
                    KeyCode::Char('n') => {
                        self.form = Some(FormState::new(
                            "New SSH profile",
                            vec![
                                FormField::new("name"),
                                FormField::new("host"),
                                FormField::new("user").with_value("root"),
                                FormField::new("port").with_value("22"),
                                FormField::new("identity_file"),
                                FormField::new("extra_args"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(p) = self.profiles.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit SSH profile",
                                    vec![
                                        FormField::new("name").with_value(&p.name),
                                        FormField::new("host").with_value(&p.host),
                                        FormField::new("user").with_value(&p.user),
                                        FormField::new("port").with_value(p.port.to_string()),
                                        FormField::new("identity_file")
                                            .with_value(&p.identity_file),
                                        FormField::new("extra_args").with_value(&p.extra_args),
                                    ],
                                ));
                                self.mode = Mode::Form {
                                    editing: Some(p.name.clone()),
                                };
                            }
                        }
                        NavAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(p) = self.profiles.get(i) {
                                self.mode = Mode::Confirm(ConfirmDelete::new(p.name.clone()));
                            }
                        }
                        NavAction::None
                    }
                    _ => NavAction::None,
                }
            }
            Mode::Form { editing } => {
                let old_name = editing.clone();
                let Some(form) = self.form.as_mut() else {
                    self.mode = Mode::List;
                    return NavAction::None;
                };
                match form.handle(key) {
                    FormResult::Cancel => {
                        self.form = None;
                        self.mode = Mode::List;
                    }
                    FormResult::Submit => {
                        let vals = form.values();
                        let name = vals.first().cloned().unwrap_or_default().trim().to_string();
                        if name.is_empty() {
                            ctx.set_error("name is required");
                        } else {
                            let port = vals
                                .get(3)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(22);
                            let profile = SshProfile {
                                name: name.clone(),
                                host: vals.get(1).cloned().unwrap_or_default(),
                                user: vals.get(2).cloned().unwrap_or_default(),
                                port,
                                identity_file: vals.get(4).cloned().unwrap_or_default(),
                                extra_args: vals.get(5).cloned().unwrap_or_default(),
                            };
                            if let Some(old) = old_name {
                                if old != name {
                                    if let Ok(dir) = Self::dir(ctx) {
                                        let _ = fs::remove_file(
                                            dir.join(format!("{}.toml", sanitize(&old))),
                                        );
                                    }
                                }
                            }
                            self.save_one(ctx, &profile);
                            self.form = None;
                            self.mode = Mode::List;
                            self.reload(ctx);
                        }
                    }
                    FormResult::Continue => {}
                }
                NavAction::None
            }
            Mode::Confirm(confirm) => match confirm.handle(key) {
                ConfirmResult::Cancel => {
                    self.mode = Mode::List;
                    NavAction::None
                }
                ConfirmResult::Confirmed => {
                    let name = confirm.item_name.clone();
                    self.delete_named(ctx, &name);
                    self.mode = Mode::List;
                    NavAction::None
                }
                ConfirmResult::Continue => NavAction::None,
            },
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        draw_select_list(
            frame,
            area,
            "Lazy Connect · Enter opens ssh",
            &self.list.items,
            self.list.selected(),
        );
        if let Mode::Form { .. } = &self.mode {
            if let Some(form) = &self.form {
                form.draw(frame, area);
            }
        }
        if let Mode::Confirm(confirm) = &self.mode {
            confirm.draw(frame, area);
        }
    }
}
