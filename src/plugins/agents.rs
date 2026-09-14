//! Custom agent presets with master prompts.

use crate::herdr::run_herdr_ok;
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
struct AgentPreset {
    name: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    master_prompt: String,
    #[serde(default)]
    args: String,
    #[serde(default)]
    notes: String,
}

fn default_kind() -> String {
    "codex".into()
}

enum Mode {
    List,
    Form { editing: Option<String> },
    Confirm(ConfirmDelete),
}

pub struct AgentsPlugin {
    list: ScrollList,
    agents: Vec<AgentPreset>,
    mode: Mode,
    form: Option<FormState>,
}

impl AgentsPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            agents: Vec::new(),
            mode: Mode::List,
            form: None,
        }
    }

    fn dir(ctx: &PluginCtx) -> anyhow::Result<PathBuf> {
        ctx.paths.ensure_subdir(true, "agents")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        self.agents.clear();
        if let Ok(files) = Self::dir(ctx).and_then(|d| list_toml_stem_files(&d)) {
            for path in files {
                if let Ok(Some(a)) = read_toml::<AgentPreset>(&path) {
                    self.agents.push(a);
                }
            }
        }
        self.agents.sort_by(|a, b| a.name.cmp(&b.name));
        self.list.set_items(
            self.agents
                .iter()
                .map(|a| format!("{}  [{}]", a.name, a.kind))
                .collect(),
        );
    }

    fn save_one(&self, ctx: &mut PluginCtx, agent: &AgentPreset) {
        if let Ok(dir) = Self::dir(ctx) {
            let path = dir.join(format!("{}.toml", sanitize(&agent.name)));
            if let Err(e) = write_toml(&path, agent) {
                ctx.set_error(format!("save agent: {e}"));
            } else {
                ctx.set_status(format!("saved {}", agent.name));
            }
        }
    }

    fn start_selected(&mut self, ctx: &mut PluginCtx) {
        let Some(i) = self.list.selected() else { return };
        let Some(a) = self.agents.get(i) else { return };
        // Start requires a pane id; use current pane when available.
        let pane = std::env::var("HERDR_PANE_ID").ok();
        let Some(pane) = pane else {
            ctx.set_error(
                "No HERDR_PANE_ID (popup panes lack one). Open Lazy Herd as overlay/split or start agents from a shell pane.",
            );
            return;
        };
        let mut args = vec![
            "agent".to_string(),
            "start".to_string(),
            "--kind".to_string(),
            a.kind.clone(),
            "--pane".to_string(),
            pane,
        ];
        if !a.args.is_empty() {
            args.push("--".into());
            for part in a.args.split_whitespace() {
                args.push(part.to_string());
            }
        }
        let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
        match run_herdr_ok(&str_args) {
            Ok(_) => {
                if !a.master_prompt.is_empty() {
                    let prompt_args = [
                        "agent",
                        "prompt",
                        &a.master_prompt,
                    ];
                    let _ = run_herdr_ok(&prompt_args);
                }
                ctx.set_status(format!("started agent {}", a.name));
            }
            Err(e) => ctx.set_error(format!("agent start: {e}")),
        }
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

impl SubPlugin for AgentsPlugin {
    fn id(&self) -> &'static str {
        "agents"
    }
    fn title(&self) -> &'static str {
        "Lazy Agents"
    }
    fn description(&self) -> &'static str {
        "Create custom agent presets with kind, master prompt, and args. Enter starts the agent when Herdr provides a pane id."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        ctx.set_status("n new · e edit · d delete · Enter start · Esc back");
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
                        self.start_selected(ctx);
                        NavAction::None
                    }
                    KeyCode::Char('n') => {
                        self.form = Some(FormState::new(
                            "New agent",
                            vec![
                                FormField::new("name"),
                                FormField::new("kind").with_value("codex"),
                                FormField::new("master_prompt"),
                                FormField::new("args"),
                                FormField::new("notes"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(a) = self.agents.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit agent",
                                    vec![
                                        FormField::new("name").with_value(&a.name),
                                        FormField::new("kind").with_value(&a.kind),
                                        FormField::new("master_prompt")
                                            .with_value(&a.master_prompt),
                                        FormField::new("args").with_value(&a.args),
                                        FormField::new("notes").with_value(&a.notes),
                                    ],
                                ));
                                self.mode = Mode::Form {
                                    editing: Some(a.name.clone()),
                                };
                            }
                        }
                        NavAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(a) = self.agents.get(i) {
                                self.mode = Mode::Confirm(ConfirmDelete::new(a.name.clone()));
                            }
                        }
                        NavAction::None
                    }
                    _ => NavAction::None,
                }
            }
            Mode::Form { editing } => {
                let old = editing.clone();
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
                            ctx.set_error("name required");
                        } else {
                            let agent = AgentPreset {
                                name: name.clone(),
                                kind: vals.get(1).cloned().unwrap_or_else(default_kind),
                                master_prompt: vals.get(2).cloned().unwrap_or_default(),
                                args: vals.get(3).cloned().unwrap_or_default(),
                                notes: vals.get(4).cloned().unwrap_or_default(),
                            };
                            if let Some(old_name) = old {
                                if old_name != name {
                                    if let Ok(dir) = Self::dir(ctx) {
                                        let _ = fs::remove_file(
                                            dir.join(format!("{}.toml", sanitize(&old_name))),
                                        );
                                    }
                                }
                            }
                            self.save_one(ctx, &agent);
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
                    if let Ok(dir) = Self::dir(ctx) {
                        let _ = fs::remove_file(dir.join(format!("{}.toml", sanitize(&name))));
                    }
                    self.reload(ctx);
                    ctx.set_status(format!("deleted {name}"));
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
            "Lazy Agents · Enter starts",
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
