//! Minimal secrets manager for agent PATs and similar values.

use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_json, write_json};
use crate::ui::{ConfirmDelete, ConfirmResult};
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crate::ui::{draw_select_list, mask_secret};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SecretsFile {
    secrets: Vec<SecretItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretItem {
    name: String,
    value: String,
    note: String,
}

enum Mode {
    List,
    Form { editing: Option<usize> },
    Confirm(ConfirmDelete),
}

pub struct SecretsPlugin {
    list: ScrollList,
    items: Vec<SecretItem>,
    mode: Mode,
    form: Option<FormState>,
    reveal: bool,
}

impl SecretsPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            items: Vec::new(),
            mode: Mode::List,
            form: None,
            reveal: false,
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("secrets.json")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        match read_json::<SecretsFile>(&Self::path(ctx)) {
            Ok(Some(file)) => self.items = file.secrets,
            Ok(None) => self.items = Vec::new(),
            Err(e) => {
                ctx.set_error(format!("load secrets: {e}"));
                self.items = Vec::new();
            }
        }
        self.sync_list();
    }

    fn sync_list(&mut self) {
        let labels = self
            .items
            .iter()
            .map(|s| {
                format!(
                    "{}  {}",
                    s.name,
                    mask_secret(&s.value, self.reveal)
                )
            })
            .collect();
        self.list.set_items(labels);
    }

    fn save(&mut self, ctx: &mut PluginCtx) {
        let file = SecretsFile {
            secrets: self.items.clone(),
        };
        match write_json(&Self::path(ctx), &file) {
            Ok(()) => ctx.set_status("secrets saved"),
            Err(e) => ctx.set_error(format!("save secrets: {e}")),
        }
        self.sync_list();
    }
}

impl SubPlugin for SecretsPlugin {
    fn id(&self) -> &'static str {
        "secrets"
    }
    fn title(&self) -> &'static str {
        "Lazy Secrets"
    }
    fn description(&self) -> &'static str {
        "Minimal secrets store for PATs and tokens. Values are masked by default. Create / edit / delete (type name to confirm)."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        ctx.set_status("n new · e edit · d delete · r reveal · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        match &mut self.mode {
            Mode::List => {
                if self.list.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => NavAction::Back,
                    KeyCode::Char('r') => {
                        self.reveal = !self.reveal;
                        self.sync_list();
                        NavAction::None
                    }
                    KeyCode::Char('n') => {
                        self.form = Some(FormState::new(
                            "New secret",
                            vec![
                                FormField::new("name"),
                                FormField::new("value").secret(),
                                FormField::new("note"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(item) = self.items.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit secret",
                                    vec![
                                        FormField::new("name").with_value(&item.name),
                                        FormField::new("value")
                                            .secret()
                                            .with_value(&item.value),
                                        FormField::new("note").with_value(&item.note),
                                    ],
                                ));
                                self.mode = Mode::Form {
                                    editing: Some(i),
                                };
                            }
                        }
                        NavAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(item) = self.items.get(i) {
                                self.mode = Mode::Confirm(ConfirmDelete::new(item.name.clone()));
                            }
                        }
                        NavAction::None
                    }
                    _ => NavAction::None,
                }
            }
            Mode::Form { editing } => {
                let editing = *editing;
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
                            let item = SecretItem {
                                name,
                                value: vals.get(1).cloned().unwrap_or_default(),
                                note: vals.get(2).cloned().unwrap_or_default(),
                            };
                            if let Some(i) = editing {
                                if i < self.items.len() {
                                    self.items[i] = item;
                                }
                            } else {
                                self.items.push(item);
                            }
                            self.save(ctx);
                            self.form = None;
                            self.mode = Mode::List;
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
                    self.items.retain(|s| s.name != name);
                    self.save(ctx);
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
            "Lazy Secrets · n/e/d/r",
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
