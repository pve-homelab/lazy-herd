//! Premade Herdr workspace templates (multi-pane + agents + optional board).

use crate::herdr::run_herdr_ok;
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{list_toml_stem_files, read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{ConfirmDelete, ConfirmResult};
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
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
    #[serde(default)]
    open_board: bool,
    #[serde(default)]
    open_browser: bool,
    /// Ordered panes. First uses the workspace root pane; later panes are splits.
    #[serde(default)]
    panes: Vec<PaneSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaneSpec {
    name: String,
    /// Herdr agent kind: codex, cursor, claude, opencode, …
    #[serde(default = "default_kind")]
    agent_kind: String,
    #[serde(default)]
    master_prompt: String,
    /// right | down — how to split from the previous pane (ignored for first).
    #[serde(default = "default_dir")]
    direction: String,
}

fn default_kind() -> String {
    "codex".into()
}

fn default_dir() -> String {
    "right".into()
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
    log: String,
}

impl WorkspacePlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            items: Vec::new(),
            mode: Mode::List,
            form: None,
            log: String::new(),
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
                    let mut bits = Vec::new();
                    if w.open_board {
                        bits.push("board".to_string());
                    }
                    if !w.panes.is_empty() {
                        bits.push(format!("{} panes", w.panes.len()));
                    }
                    if bits.is_empty() {
                        format!("{} — {}", w.name, w.description)
                    } else {
                        format!("{} [{}] — {}", w.name, bits.join(","), w.description)
                    }
                })
                .collect(),
        );
    }

    fn install_example(&mut self, ctx: &mut PluginCtx) {
        let Ok(dir) = Self::dir(ctx) else {
            ctx.set_error("workspaces dir missing");
            return;
        };
        let path = dir.join("coding-loop.toml");
        if path.exists() {
            ctx.set_status("coding-loop.toml already exists — edit it or press e");
            self.reload(ctx);
            return;
        }
        if let Err(e) = fs::write(&path, CODING_LOOP_EXAMPLE) {
            ctx.set_error(format!("write example: {e}"));
            return;
        }
        self.log = format!(
            "Wrote example workspace:\n  {}\n\nEdit working_dir, then Enter to apply.\nSee docs/CONFIG.md for the full guide.",
            path.display()
        );
        ctx.set_status("installed coding-loop example");
        self.reload(ctx);
    }

    fn apply_selected(&mut self, ctx: &mut PluginCtx) {
        let Some(i) = self.list.selected() else {
            return;
        };
        let Some(w) = self.items.get(i).cloned() else {
            return;
        };
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

        let created = match run_herdr_ok(&[
            "workspace",
            "create",
            "--cwd",
            &cwd,
            "--label",
            &label,
            "--focus",
        ]) {
            Ok(out) => out,
            Err(e) => {
                ctx.set_error(format!("workspace create: {e}"));
                return;
            }
        };

        let mut log = format!("Created workspace `{label}` @ {cwd}\n");
        let mut last_pane = extract_json_str(&created, &["result", "root_pane", "pane_id"])
            .or_else(|| extract_json_str(&created, &["root_pane", "pane_id"]));

        if w.panes.is_empty() {
            log.push_str("No [[panes]] in template — empty workspace shell.\n");
        } else {
            for (idx, pane) in w.panes.iter().enumerate() {
                let pane_id = if idx == 0 {
                    match last_pane.clone() {
                        Some(id) => id,
                        None => {
                            log.push_str("Missing root pane id — stop.\n");
                            break;
                        }
                    }
                } else {
                    let from = last_pane.clone().unwrap_or_default();
                    let dir = if pane.direction == "down" {
                        "down"
                    } else {
                        "right"
                    };
                    match run_herdr_ok(&[
                        "pane",
                        "split",
                        &from,
                        "--direction",
                        dir,
                        "--cwd",
                        &cwd,
                        "--no-focus",
                    ]) {
                        Ok(out) => {
                            if let Some(id) = extract_json_str(&out, &["result", "pane", "pane_id"])
                                .or_else(|| extract_json_str(&out, &["pane", "pane_id"]))
                            {
                                id
                            } else {
                                log.push_str(&format!("split ok but no pane id: {out}\n"));
                                break;
                            }
                        }
                        Err(e) => {
                            log.push_str(&format!("split {} failed: {e}\n", pane.name));
                            break;
                        }
                    }
                };

                let _ = run_herdr_ok(&["pane", "rename", &pane_id, &pane.name]);
                last_pane = Some(pane_id.clone());

                match run_herdr_ok(&[
                    "agent",
                    "start",
                    &pane.name,
                    "--kind",
                    &pane.agent_kind,
                    "--pane",
                    &pane_id,
                ]) {
                    Ok(_) => {
                        log.push_str(&format!(
                            "Pane {} ({}) started as {}\n",
                            pane.name, pane_id, pane.agent_kind
                        ));
                        if !pane.master_prompt.trim().is_empty() {
                            match run_herdr_ok(&["agent", "prompt", &pane.master_prompt]) {
                                Ok(_) => log.push_str("  master prompt sent\n"),
                                Err(e) => log.push_str(&format!("  prompt failed: {e}\n")),
                            }
                        }
                    }
                    Err(e) => {
                        log.push_str(&format!(
                            "Pane {} ready but agent start failed: {e}\n",
                            pane.name
                        ));
                    }
                }
            }
        }

        if w.open_board {
            match run_herdr_ok(&[
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-board",
                "--entrypoint",
                "board",
            ]) {
                Ok(_) => log.push_str("Lazy Board / herdr-board opened\n"),
                Err(e) => log.push_str(&format!("board open failed: {e}\n")),
            }
        }
        if w.open_browser {
            let _ = run_herdr_ok(&[
                "plugin",
                "action",
                "invoke",
                "zenbu-labs.terminal-browser.open-split",
            ]);
        }

        self.log = log;
        ctx.set_status(format!("applied `{}`", w.name));
    }
}

fn extract_json_str(raw: &str, path: &[&str]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let mut cur = &v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(str::to_string)
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
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

const CODING_LOOP_EXAMPLE: &str = r#"name = "coding-loop"
description = "Orchestrator + Dev + Test + Audit + Board"
working_dir = "~/repos/my-project"
label = "coding-loop"
open_board = true

[[panes]]
name = "Orchestrator"
agent_kind = "codex"
direction = "right"
master_prompt = """
You are Orchestrator. You do NOT write app code.
1) Receive updates from Dev, Test, and Audit.
2) Keep the Herdr kanban board accurate: create/move cards for coding work.
3) Unstick agents: if someone is idle or blocked, nudge them with the next task.
4) Own the workflow loop: code → test → audit → board update → next coding task.
"""

[[panes]]
name = "Dev"
agent_kind = "codex"
direction = "right"
master_prompt = """
You are Dev. Implement features and fixes from Orchestrator's board/tasks.
Write code in this repo. When a task is done, summarize what changed and notify Orchestrator
so the kanban card can move forward.
"""

[[panes]]
name = "Test"
agent_kind = "codex"
direction = "down"
master_prompt = """
You are Test. Write tests and run them to find bugs.
Report failures clearly (repro, expected vs actual). Send findings to Audit and Orchestrator.
"""

[[panes]]
name = "Audit"
agent_kind = "codex"
direction = "right"
master_prompt = """
You are Audit. Review Test results and code risk.
Package a neat report for Orchestrator: bugs, severity, and how to fix.
Do not implement fixes yourself unless Orchestrator asks.
"""
"#;

impl SubPlugin for WorkspacePlugin {
    fn id(&self) -> &'static str {
        "workspace"
    }
    fn title(&self) -> &'static str {
        "Lazy Workspace"
    }
    fn description(&self) -> &'static str {
        "Premade multi-pane workspaces (agents + optional board). n = simple form, i = install coding-loop example, Enter = apply. See docs/CONFIG.md."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::List;
        self.log = "n new (basic) · i install coding-loop example · e edit meta · d delete · Enter apply\nFull pane [[panes]] editing: open the TOML under workspaces/ (see docs/CONFIG.md).".into();
        ctx.set_status("Enter apply · i example · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        match &mut self.mode {
            Mode::List => {
                if self.list.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => NavAction::Back,
                    KeyCode::Char('i') => {
                        self.install_example(ctx);
                        NavAction::None
                    }
                    KeyCode::Enter => {
                        self.apply_selected(ctx);
                        NavAction::None
                    }
                    KeyCode::Char('n') => {
                        self.form = Some(FormState::new(
                            "New workspace (basic meta)",
                            vec![
                                FormField::new("name"),
                                FormField::new("description"),
                                FormField::new("working_dir").with_value("~/repos/"),
                                FormField::new("label"),
                                FormField::new("open_board").with_value("false"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') => {
                        if let Some(i) = self.list.selected() {
                            if let Some(w) = self.items.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit workspace meta (panes: edit TOML)",
                                    vec![
                                        FormField::new("name").with_value(&w.name),
                                        FormField::new("description").with_value(&w.description),
                                        FormField::new("working_dir").with_value(&w.working_dir),
                                        FormField::new("label").with_value(&w.label),
                                        FormField::new("open_board")
                                            .with_value(w.open_board.to_string()),
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
                                matches!(
                                    s.trim().to_ascii_lowercase().as_str(),
                                    "1" | "true" | "yes" | "y"
                                )
                            };
                            // Preserve existing panes when editing meta only.
                            let existing_panes = old
                                .as_ref()
                                .and_then(|n| {
                                    self.items
                                        .iter()
                                        .find(|w| &w.name == n)
                                        .map(|w| w.panes.clone())
                                })
                                .unwrap_or_default();
                            let tmpl = WorkspaceTemplate {
                                name: name.clone(),
                                description: vals.get(1).cloned().unwrap_or_default(),
                                working_dir: vals.get(2).cloned().unwrap_or_default(),
                                label: vals.get(3).cloned().unwrap_or_default(),
                                open_board: vals.get(4).map(|s| parse_bool(s)).unwrap_or(false),
                                open_browser: false,
                                panes: existing_panes,
                            };
                            if let Ok(dir) = Self::dir(ctx) {
                                if let Some(old_name) = old {
                                    if old_name != name {
                                        let _ = fs::remove_file(
                                            dir.join(format!("{}.toml", sanitize(&old_name))),
                                        );
                                    }
                                }
                                if let Err(e) = write_toml(
                                    &dir.join(format!("{}.toml", sanitize(&name))),
                                    &tmpl,
                                ) {
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        draw_select_list(
            frame,
            chunks[0],
            "Lazy Workspace · i = example",
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.log.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Output / tips").borders(Borders::ALL)),
            chunks[1],
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
