//! Premade Herdr workspace templates.

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
struct WorkspaceTemplate {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    working_dir: String,
    #[serde(default)]
    label: String,
    /// Comma-separated agent kinds to start after create (best-effort).
    #[serde(default)]
    agent_kinds: String,
    #[serde(default)]
    open_board: bool,
    #[serde(default)]
    open_browser: bool,
}

enum Mode {
    List,
    Form { editing: Option<String> },
    Confirm(ConfirmDelete),
}

pub struct WorkspacePlugin {
    list: ScrollList,
    items: Vec<WorkspaceTemplate>,
    mode: Mode,
    form: Option<FormState>,
}

impl WorkspacePlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            items: Vec::new(),
            mode: Mode::List,
            form: None,
        }
    }

    fn dir(ctx: &PluginCtx) -> anyhow::Result<PathBuf> {
        ctx.paths.ensure_subdir(true, "workspaces")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        self.items.clear();
        if let Ok(files) = Self::dir(ctx).and_then(|d| list_toml_stem_files(&d)) {
            for path in files {
                if let Ok(Some(w)) = read_toml::<WorkspaceTemplate>(&path) {
                    self.items.push(w);
                }
            }
        }
        self.items.sort_by(|a, b| a.name.cmp(&b.name));
        self.list.set_items(
            self.items
                .iter()
                .map(|w| {
                    let mut flags = Vec::new();
                    if w.open_board {
                        flags.push("board");
                    }
                    if w.open_browser {
                        flags.push("browser");
                    }
                    if flags.is_empty() {
                        format!("{} — {}", w.name, w.description)
                    } else {
                        format!("{} [{}] — {}", w.name, flags.join(","), w.description)
                    }
                })
                .collect(),
        );
    }

    fn apply_selected(&mut self, ctx: &mut PluginCtx) {
        let Some(i) = self.list.selected() else { return };
        let Some(w) = self.items.get(i).cloned() else { return };
        let cwd = if w.working_dir.is_empty() {
            ".".to_string()
        } else {
            shellexpand_home(&w.working_dir)
        };
        let label = if w.label.is_empty() {
            w.name.clone()
        } else {
            w.label.clone()
        };
        match run_herdr_ok(&[
            "workspace",
            "create",
            "--cwd",
            &cwd,
            "--label",
            &label,
        ]) {
            Ok(out) => {
                ctx.set_status(format!("workspace created: {out}"));
                if w.open_board {
                    let _ = run_herdr_ok(&[
                        "plugin",
                        "pane",
                        "open",
                        "--plugin",
                        "herdr-board",
                        "--entrypoint",
                        "board",
                    ]);
                }
                if w.open_browser {
                    let _ = run_herdr_ok(&[
                        "plugin",
                        "action",
                        "invoke",
                        "zenbu-labs.terminal-browser.open-split",
                    ]);
                }
                // Agent kinds are informational for now; full multi-pane layout
                // can be expanded later via herdr layout.apply.
                if !w.agent_kinds.is_empty() {
                    ctx.set_status(format!(
                        "workspace ok; configure agents manually for kinds: {}",
                        w.agent_kinds
                    ));
                }
            }
            Err(e) => ctx.set_error(format!("workspace create: {e}")),
        }
    }
}

fn shellexpand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return format!("{home}/{rest}");
        }
    }
    if path == "~" {
        return std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| path.to_string());
    }
    path.to_string()
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

impl SubPlugin for WorkspacePlugin {
    fn id(&self) -> &'static str {
        "workspace"
    }
    fn title(&self) -> &'static str {
        "Lazy Workspace"
    }
    fn description(&self) -> &'static str {
        "Premade workspaces (coding with agents/kanban, research with browser, etc.). Create / edit / delete; Enter applies via herdr workspace create."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        ctx.set_status("n new · e edit · d delete · Enter apply · Esc back");
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
                        self.apply_selected(ctx);
                        NavAction::None
                    }
                    KeyCode::Char('n') => {
                        self.form = Some(FormState::new(
                            "New workspace",
                            vec![
                                FormField::new("name"),
                                FormField::new("description"),
                                FormField::new("working_dir").with_value("~/"),
                                FormField::new("label"),
                                FormField::new("agent_kinds").with_value("codex"),
                                FormField::new("open_board").with_value("false"),
                                FormField::new("open_browser").with_value("false"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(w) = self.items.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit workspace",
                                    vec![
                                        FormField::new("name").with_value(&w.name),
                                        FormField::new("description").with_value(&w.description),
                                        FormField::new("working_dir").with_value(&w.working_dir),
                                        FormField::new("label").with_value(&w.label),
                                        FormField::new("agent_kinds").with_value(&w.agent_kinds),
                                        FormField::new("open_board")
                                            .with_value(w.open_board.to_string()),
                                        FormField::new("open_browser")
                                            .with_value(w.open_browser.to_string()),
                                    ],
                                ));
                                self.mode = Mode::Form {
                                    editing: Some(w.name.clone()),
                                };
                            }
                        }
                        NavAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(w) = self.items.get(i) {
                                self.mode = Mode::Confirm(ConfirmDelete::new(w.name.clone()));
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
                            let parse_bool = |s: &str| {
                                matches!(s.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "y")
                            };
                            let tmpl = WorkspaceTemplate {
                                name: name.clone(),
                                description: vals.get(1).cloned().unwrap_or_default(),
                                working_dir: vals.get(2).cloned().unwrap_or_default(),
                                label: vals.get(3).cloned().unwrap_or_default(),
                                agent_kinds: vals.get(4).cloned().unwrap_or_default(),
                                open_board: vals.get(5).map(|s| parse_bool(s)).unwrap_or(false),
                                open_browser: vals.get(6).map(|s| parse_bool(s)).unwrap_or(false),
                            };
                            if let Ok(dir) = Self::dir(ctx) {
                                if let Some(old_name) = old {
                                    if old_name != name {
                                        let _ = fs::remove_file(
                                            dir.join(format!("{}.toml", sanitize(&old_name))),
                                        );
                                    }
                                }
                                if let Err(e) =
                                    write_toml(&dir.join(format!("{}.toml", sanitize(&name))), &tmpl)
                                {
                                    ctx.set_error(format!("save: {e}"));
                                } else {
                                    ctx.set_status(format!("saved {name}"));
                                }
                            }
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
            "Lazy Workspace · Enter applies",
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
