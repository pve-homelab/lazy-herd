//! SSH profiles — Enter opens a manage pane (create / edit / delete / connect).

use crate::herdr::{run_herdr_ok, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{list_toml_stem_files, read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{ConfirmDelete, ConfirmResult};
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
    /// Profile list.
    List,
    /// Per-profile manage pane: Connect / Edit / Delete / Back.
    Manage {
        index: usize,
        actions: ScrollList,
    },
    Form {
        editing: Option<String>,
    },
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
        let mut items: Vec<String> = vec!["+ New SSH profile".into()];
        items.extend(
            self.profiles
                .iter()
                .map(|p| format!("{}  {}@{}:{}", p.name, p.user, p.host, p.port)),
        );
        self.list.set_items(items);
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

    fn open_manage(&mut self, profile_index: usize, ctx: &mut PluginCtx) {
        let name = self
            .profiles
            .get(profile_index)
            .map(|p| p.name.as_str())
            .unwrap_or("?");
        ctx.set_status(format!("manage {name} · Enter action · Esc list"));
        self.mode = Mode::Manage {
            index: profile_index,
            actions: ScrollList::new(vec![
                "Connect (SSH in new pane)".into(),
                "Edit profile".into(),
                "Delete profile".into(),
                "Back to list".into(),
            ]),
        };
    }

    fn start_new_form(&mut self) {
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
    }

    fn start_edit_form(&mut self, index: usize) {
        let Some(p) = self.profiles.get(index) else {
            return;
        };
        self.form = Some(FormState::new(
            "Edit SSH profile",
            vec![
                FormField::new("name").with_value(&p.name),
                FormField::new("host").with_value(&p.host),
                FormField::new("user").with_value(&p.user),
                FormField::new("port").with_value(p.port.to_string()),
                FormField::new("identity_file").with_value(&p.identity_file),
                FormField::new("extra_args").with_value(&p.extra_args),
            ],
        ));
        self.mode = Mode::Form {
            editing: Some(p.name.clone()),
        };
    }

    fn connect_ssh(&mut self, ctx: &mut PluginCtx, index: usize) {
        let Some(p) = self.profiles.get(index).cloned() else {
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
            Ok(_) => ctx.set_status(format!("SSH → {}", p.name)),
            Err(e) => ctx.set_error(format!(
                "pane split failed ({e}). Manual: ssh -p {} {}@{}",
                p.port, p.user, p.host
            )),
        }
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
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
        "SSH profiles for any machine. Enter opens a manage pane to create / edit / delete / connect — so you never hunt for credentials again."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        ctx.set_status("Enter = manage pane · Esc back");
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
                        match self.list.selected() {
                            Some(0) => self.start_new_form(),
                            Some(i) if i >= 1 => {
                                let idx = i - 1;
                                if idx < self.profiles.len() {
                                    self.open_manage(idx, ctx);
                                }
                            }
                            _ => {}
                        }
                        NavAction::None
                    }
                    _ => NavAction::None,
                }
            }
            Mode::Manage { index, actions } => {
                let index = *index;
                if actions.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => {
                        self.mode = Mode::List;
                        ctx.set_status("Enter = manage pane · Esc back");
                        NavAction::None
                    }
                    KeyCode::Enter => {
                        match actions.selected() {
                            Some(0) => self.connect_ssh(ctx, index),
                            Some(1) => self.start_edit_form(index),
                            Some(2) => {
                                if let Some(p) = self.profiles.get(index) {
                                    self.mode =
                                        Mode::Confirm(ConfirmDelete::new(p.name.clone()));
                                }
                            }
                            Some(3) => {
                                self.mode = Mode::List;
                                ctx.set_status("Enter = manage pane · Esc back");
                            }
                            _ => {}
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
        match &self.mode {
            Mode::Manage { index, actions } => {
                let title = self
                    .profiles
                    .get(*index)
                    .map(|p| format!("Manage · {} ({}@{}:{})", p.name, p.user, p.host, p.port))
                    .unwrap_or_else(|| "Manage".into());
                draw_select_list(
                    frame,
                    area,
                    &title,
                    &actions.items,
                    actions.selected(),
                );
            }
            _ => {
                draw_select_list(
                    frame,
                    area,
                    "Lazy Connect · Enter opens manage pane",
                    &self.list.items,
                    self.list.selected(),
                );
            }
        }
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
        if let Mode::Confirm(confirm) = &self.mode {
            confirm.draw(frame, area);
        }
    }
}
