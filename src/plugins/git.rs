//! Forge accounts + local git operations.

use crate::herdr::{run_capture, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_toml, write_toml};
use crate::ui::{ConfirmDelete, ConfirmResult};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AccountsFile {
    accounts: Vec<ForgeAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ForgeAccount {
    name: String,
    /// github | gitlab | forgejo | gitea | local
    provider: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    username: String,
    /// Name of a Lazy Secrets entry holding the token (not the token itself).
    #[serde(default)]
    secret_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Accounts,
    GitOps,
}

enum Mode {
    Browse,
    Form { editing: Option<usize> },
    Confirm(ConfirmDelete),
}

pub struct GitPlugin {
    view: View,
    list: ScrollList,
    accounts: Vec<ForgeAccount>,
    mode: Mode,
    form: Option<FormState>,
    log: String,
}

impl GitPlugin {
    pub fn new() -> Self {
        Self {
            view: View::Accounts,
            list: ScrollList::default(),
            accounts: Vec::new(),
            mode: Mode::Browse,
            form: None,
            log: String::new(),
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("git/accounts.toml")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        match read_toml::<AccountsFile>(&Self::path(ctx)) {
            Ok(Some(f)) => self.accounts = f.accounts,
            Ok(None) => self.accounts = Vec::new(),
            Err(e) => ctx.set_error(format!("load accounts: {e}")),
        }
        self.sync_list();
    }

    fn sync_list(&mut self) {
        match self.view {
            View::Accounts => {
                self.list.set_items(
                    self.accounts
                        .iter()
                        .map(|a| {
                            format!(
                                "{}  [{}] {} {}",
                                a.name, a.provider, a.username, a.host
                            )
                        })
                        .collect(),
                );
            }
            View::GitOps => {
                self.list.set_items(vec![
                    "status".into(),
                    "fetch".into(),
                    "pull".into(),
                    "push".into(),
                    "branch list".into(),
                    "commit (-a -m … via form)".into(),
                    "repo list (gh/glab if available)".into(),
                ]);
            }
        }
    }

    fn save(&mut self, ctx: &mut PluginCtx) {
        let file = AccountsFile {
            accounts: self.accounts.clone(),
        };
        if let Err(e) = write_toml(&Self::path(ctx), &file) {
            ctx.set_error(format!("save accounts: {e}"));
        } else {
            ctx.set_status("accounts saved");
        }
        self.sync_list();
    }

    fn run_git(&mut self, ctx: &mut PluginCtx, args: &[&str]) {
        if !which_exists("git") {
            ctx.set_error("git not on PATH");
            return;
        }
        match run_capture("git", args) {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).to_string();
                if !out.stderr.is_empty() {
                    text.push('\n');
                    text.push_str(&String::from_utf8_lossy(&out.stderr));
                }
                if !out.status.success() {
                    ctx.set_error(format!("git {} failed", args.join(" ")));
                } else {
                    ctx.set_status(format!("git {} ok", args.join(" ")));
                }
                self.log = text;
            }
            Err(e) => ctx.set_error(e.to_string()),
        }
    }

    fn run_repo_list(&mut self, ctx: &mut PluginCtx) {
        if which_exists("gh") {
            match run_capture("gh", &["repo", "list", "--limit", "30"]) {
                Ok(out) => {
                    self.log = String::from_utf8_lossy(&out.stdout).to_string();
                    ctx.set_status("gh repo list");
                }
                Err(e) => ctx.set_error(e.to_string()),
            }
            return;
        }
        if which_exists("glab") {
            match run_capture("glab", &["repo", "list"]) {
                Ok(out) => {
                    self.log = String::from_utf8_lossy(&out.stdout).to_string();
                    ctx.set_status("glab repo list");
                }
                Err(e) => ctx.set_error(e.to_string()),
            }
            return;
        }
        ctx.set_error("Install `gh` or `glab` for remote repo listing, or use local git ops.");
    }

    fn activate_git_op(&mut self, ctx: &mut PluginCtx) {
        match self.list.selected() {
            Some(0) => self.run_git(ctx, &["status", "-sb"]),
            Some(1) => self.run_git(ctx, &["fetch", "--all", "--prune"]),
            Some(2) => self.run_git(ctx, &["pull", "--ff-only"]),
            Some(3) => self.run_git(ctx, &["push"]),
            Some(4) => self.run_git(ctx, &["branch", "-vv"]),
            Some(5) => {
                self.form = Some(FormState::new(
                    "git commit",
                    vec![FormField::new("message")],
                ));
                self.mode = Mode::Form { editing: None };
            }
            Some(6) => self.run_repo_list(ctx),
            _ => {}
        }
    }
}

impl SubPlugin for GitPlugin {
    fn id(&self) -> &'static str {
        "git"
    }
    fn title(&self) -> &'static str {
        "Lazy Git"
    }
    fn description(&self) -> &'static str {
        "Log into GitHub/GitLab/Forgejo/Gitea/local accounts, list repos (via gh/glab), and run pull/push/commit/branch against the current repo."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.view = View::Accounts;
        self.mode = Mode::Browse;
        self.sync_list();
        ctx.set_status("Tab switch Accounts/Ops · n/e/d accounts · Enter run · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        match &mut self.mode {
            Mode::Browse => {
                if self.list.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => NavAction::Back,
                    KeyCode::Tab => {
                        self.view = match self.view {
                            View::Accounts => View::GitOps,
                            View::GitOps => View::Accounts,
                        };
                        self.sync_list();
                        NavAction::None
                    }
                    KeyCode::Enter => {
                        if self.view == View::GitOps {
                            self.activate_git_op(ctx);
                        } else {
                            ctx.set_status("Account selected — Tab to Git Ops, or e to edit");
                        }
                        NavAction::None
                    }
                    KeyCode::Char('n') if self.view == View::Accounts => {
                        self.form = Some(FormState::new(
                            "New forge account",
                            vec![
                                FormField::new("name"),
                                FormField::new("provider")
                                    .with_value("github"),
                                FormField::new("host").with_value("github.com"),
                                FormField::new("username"),
                                FormField::new("secret_ref"),
                            ],
                        ));
                        self.mode = Mode::Form { editing: None };
                        NavAction::None
                    }
                    KeyCode::Char('e') if self.view == View::Accounts => {
                        if let Some(i) = self.list.selected() {
                            if let Some(a) = self.accounts.get(i) {
                                self.form = Some(FormState::new(
                                    "Edit forge account",
                                    vec![
                                        FormField::new("name").with_value(&a.name),
                                        FormField::new("provider").with_value(&a.provider),
                                        FormField::new("host").with_value(&a.host),
                                        FormField::new("username").with_value(&a.username),
                                        FormField::new("secret_ref").with_value(&a.secret_ref),
                                    ],
                                ));
                                self.mode = Mode::Form {
                                    editing: Some(i),
                                };
                            }
                        }
                        NavAction::None
                    }
                    KeyCode::Char('d') if self.view == View::Accounts => {
                        if let Some(i) = self.list.selected() {
                            if let Some(a) = self.accounts.get(i) {
                                self.mode = Mode::Confirm(ConfirmDelete::new(a.name.clone()));
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
                    self.mode = Mode::Browse;
                    return NavAction::None;
                };
                match form.handle(key) {
                    FormResult::Cancel => {
                        self.form = None;
                        self.mode = Mode::Browse;
                    }
                    FormResult::Submit => {
                        let vals = form.values();
                        if self.view == View::GitOps {
                            let msg = vals.first().cloned().unwrap_or_default();
                            if msg.trim().is_empty() {
                                ctx.set_error("commit message required");
                            } else {
                                self.run_git(ctx, &["commit", "-a", "-m", msg.trim()]);
                                self.form = None;
                                self.mode = Mode::Browse;
                            }
                        } else {
                            let name = vals.first().cloned().unwrap_or_default().trim().to_string();
                            if name.is_empty() {
                                ctx.set_error("name required");
                            } else {
                                let account = ForgeAccount {
                                    name,
                                    provider: vals.get(1).cloned().unwrap_or_else(|| "github".into()),
                                    host: vals.get(2).cloned().unwrap_or_default(),
                                    username: vals.get(3).cloned().unwrap_or_default(),
                                    secret_ref: vals.get(4).cloned().unwrap_or_default(),
                                };
                                if let Some(i) = editing {
                                    if i < self.accounts.len() {
                                        self.accounts[i] = account;
                                    }
                                } else {
                                    self.accounts.push(account);
                                }
                                self.save(ctx);
                                self.form = None;
                                self.mode = Mode::Browse;
                            }
                        }
                    }
                    FormResult::Continue => {}
                }
                NavAction::None
            }
            Mode::Confirm(confirm) => match confirm.handle(key) {
                ConfirmResult::Cancel => {
                    self.mode = Mode::Browse;
                    NavAction::None
                }
                ConfirmResult::Confirmed => {
                    let name = confirm.item_name.clone();
                    self.accounts.retain(|a| a.name != name);
                    self.save(ctx);
                    self.mode = Mode::Browse;
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
        let title = match self.view {
            View::Accounts => "Lazy Git · Accounts (Tab → Ops)",
            View::GitOps => "Lazy Git · Ops (Tab → Accounts)",
        };
        draw_select_list(
            frame,
            chunks[0],
            title,
            &self.list.items,
            self.list.selected(),
        );
        let log = Paragraph::new(self.log.as_str())
            .wrap(Wrap { trim: false })
            .block(Block::default().title("Output").borders(Borders::ALL));
        frame.render_widget(log, chunks[1]);

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
