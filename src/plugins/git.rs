//! Forge accounts + always-on remote repo browser (clone into your workspace).

use crate::herdr::{run_capture, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_json, read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{ConfirmDelete, ConfirmResult};
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AccountsFile {
    #[serde(default)]
    active: Option<String>,
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
    /// Lazy Secrets entry name holding the PAT/token.
    #[serde(default)]
    secret_ref: String,
}

#[derive(Debug, Clone)]
struct RemoteRepo {
    name: String,
    clone_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SecretsFile {
    #[serde(default)]
    secrets: Vec<SecretItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SecretItem {
    name: String,
    value: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Repos,
    Accounts,
    Ops,
}

enum Mode {
    Browse,
    FormAccount { editing: Option<usize> },
    FormClone { repo_idx: usize },
    FormCommit,
    Confirm(ConfirmDelete),
}

pub struct GitPlugin {
    view: View,
    list: ScrollList,
    accounts: Vec<ForgeAccount>,
    active: Option<String>,
    repos: Vec<RemoteRepo>,
    mode: Mode,
    form: Option<FormState>,
    log: String,
}

impl GitPlugin {
    pub fn new() -> Self {
        Self {
            view: View::Repos,
            list: ScrollList::default(),
            accounts: Vec::new(),
            active: None,
            repos: Vec::new(),
            mode: Mode::Browse,
            form: None,
            log: String::new(),
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("git/accounts.toml")
    }

    fn secrets_path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("secrets.json")
    }

    fn default_clone_parent(ctx: &PluginCtx) -> String {
        #[derive(Deserialize, Default)]
        struct Settings {
            #[serde(default)]
            default_workspace_dir: String,
        }
        let settings: Settings = read_toml(&ctx.paths.join_config("settings.toml"))
            .ok()
            .flatten()
            .unwrap_or_default();
        if !settings.default_workspace_dir.trim().is_empty() {
            return expand_home(settings.default_workspace_dir.trim());
        }
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(|h| format!("{h}/repos"))
            .unwrap_or_else(|_| ".".into())
    }

    fn reload_accounts(&mut self, ctx: &mut PluginCtx) {
        match read_toml::<AccountsFile>(&Self::path(ctx)) {
            Ok(Some(f)) => {
                self.accounts = f.accounts;
                self.active = f
                    .active
                    .or_else(|| self.accounts.first().map(|a| a.name.clone()));
            }
            Ok(None) => {
                self.accounts.clear();
                self.active = None;
            }
            Err(e) => ctx.set_error(format!("load accounts: {e}")),
        }
        if self.active.is_none() {
            self.active = self.accounts.first().map(|a| a.name.clone());
        }
    }

    fn save_accounts(&mut self, ctx: &mut PluginCtx) {
        let file = AccountsFile {
            active: self.active.clone(),
            accounts: self.accounts.clone(),
        };
        if let Err(e) = write_toml(&Self::path(ctx), &file) {
            ctx.set_error(format!("save accounts: {e}"));
        } else {
            ctx.set_status("accounts saved");
        }
    }

    fn active_account(&self) -> Option<&ForgeAccount> {
        let name = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.name == name)
    }

    fn lookup_secret(ctx: &PluginCtx, secret_ref: &str) -> Option<String> {
        if secret_ref.is_empty() {
            return None;
        }
        let file: SecretsFile = read_json(&Self::secrets_path(ctx)).ok().flatten()?;
        file.secrets
            .into_iter()
            .find(|s| s.name == secret_ref)
            .map(|s| s.value)
    }

    fn refresh_repos(&mut self, ctx: &mut PluginCtx) {
        self.repos.clear();
        let Some(account) = self.active_account().cloned() else {
            self.log = "No account yet. Tab → Accounts → n. Put the PAT in Lazy Secrets and set secret_ref.".into();
            self.sync_list();
            ctx.set_status("add an account to list repos");
            return;
        };

        ctx.set_status(format!("loading repos for {}…", account.name));
        let token = Self::lookup_secret(ctx, &account.secret_ref);
        let result = match account.provider.to_ascii_lowercase().as_str() {
            "github" => list_github_repos(&account, token.as_deref()),
            "gitlab" => list_gitlab_repos(&account, token.as_deref()),
            "forgejo" | "gitea" => list_gitea_repos(&account, token.as_deref()),
            "local" => Ok(Vec::new()),
            other => Err(format!("unsupported provider: {other}")),
        };

        match result {
            Ok(repos) => {
                self.repos = repos;
                self.log = format!(
                    "Account: {} [{}] @{}\nRepos: {}\nEnter = clone · r = refresh · Tab = Accounts / Ops",
                    account.name,
                    account.provider,
                    account.username,
                    self.repos.len()
                );
                if self.repos.is_empty() && account.provider != "local" {
                    self.log
                        .push_str("\n\nNo repos returned. Check secret_ref / gh auth / username.");
                }
                ctx.set_status(format!("{} repos", self.repos.len()));
            }
            Err(e) => {
                self.log = e.clone();
                ctx.set_error(e);
            }
        }
        self.sync_list();
    }

    fn sync_list(&mut self) {
        match self.view {
            View::Repos => {
                let mut items: Vec<String> = self.repos.iter().map(|r| r.name.clone()).collect();
                if items.is_empty() {
                    items.push("(no repos — Tab→Accounts or press r)".into());
                }
                self.list.set_items(items);
            }
            View::Accounts => {
                self.list.set_items(
                    self.accounts
                        .iter()
                        .map(|a| {
                            let mark = if Some(a.name.as_str()) == self.active.as_deref() {
                                "* "
                            } else {
                                "  "
                            };
                            format!(
                                "{mark}{}  [{}] {} {}",
                                a.name, a.provider, a.username, a.host
                            )
                        })
                        .collect(),
                );
            }
            View::Ops => {
                self.list.set_items(vec![
                    "status".into(),
                    "fetch".into(),
                    "pull".into(),
                    "push".into(),
                    "branch list".into(),
                    "commit (-a -m)".into(),
                ]);
            }
        }
    }

    fn clone_repo(&mut self, ctx: &mut PluginCtx, repo_idx: usize, parent: &str) {
        let Some(repo) = self.repos.get(repo_idx).cloned() else {
            ctx.set_error("no repo selected");
            return;
        };
        if !which_exists("git") {
            ctx.set_error("git not on PATH");
            return;
        }
        let leaf = repo
            .name
            .rsplit('/')
            .next()
            .unwrap_or(repo.name.as_str())
            .to_string();
        let dest = Path::new(parent).join(&leaf);
        if dest.exists() {
            ctx.set_error(format!("already exists: {}", dest.display()));
            self.log = format!("Skip clone — path exists:\n{}", dest.display());
            return;
        }
        if let Some(p) = dest.parent() {
            let _ = std::fs::create_dir_all(p);
        }

        let account = self.active_account().cloned();
        let token = account
            .as_ref()
            .and_then(|a| Self::lookup_secret(ctx, &a.secret_ref));

        let clone_url = match (&account, &token) {
            (Some(acc), Some(tok)) => inject_token_url(&repo.clone_url, tok, &acc.provider)
                .unwrap_or_else(|| repo.clone_url.clone()),
            _ => repo.clone_url.clone(),
        };

        match Command::new("git")
            .args(["clone", &clone_url, &dest.to_string_lossy()])
            .output()
        {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).to_string();
                if !out.stderr.is_empty() {
                    text.push('\n');
                    text.push_str(&String::from_utf8_lossy(&out.stderr));
                }
                if out.status.success() {
                    self.log = format!(
                        "Cloned {} → {}\n\nTip: Lazy Workspace → apply a template with working_dir = that path\n\n{text}",
                        repo.name,
                        dest.display()
                    );
                    ctx.set_status(format!("cloned {}", repo.name));
                } else {
                    self.log = text;
                    ctx.set_error(format!("git clone failed for {}", repo.name));
                }
            }
            Err(e) => ctx.set_error(e.to_string()),
        }
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

    fn activate_ops(&mut self, ctx: &mut PluginCtx) {
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
                self.mode = Mode::FormCommit;
            }
            _ => {}
        }
    }

    fn cycle_view(&mut self) {
        self.view = match self.view {
            View::Repos => View::Accounts,
            View::Accounts => View::Ops,
            View::Ops => View::Repos,
        };
        self.sync_list();
    }
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

fn inject_token_url(url: &str, token: &str, provider: &str) -> Option<String> {
    let url = url.trim();
    if !url.starts_with("https://") || token.is_empty() {
        return None;
    }
    let rest = url.strip_prefix("https://")?;
    let user = match provider.to_ascii_lowercase().as_str() {
        "github" => "x-access-token",
        _ => "oauth2",
    };
    Some(format!("https://{user}:{token}@{rest}"))
}

fn list_github_repos(
    account: &ForgeAccount,
    token: Option<&str>,
) -> Result<Vec<RemoteRepo>, String> {
    if which_exists("gh") {
        let mut cmd = Command::new("gh");
        if account.username.is_empty() {
            cmd.args([
                "repo",
                "list",
                "--limit",
                "200",
                "--json",
                "nameWithOwner,url",
            ]);
        } else {
            cmd.args([
                "repo",
                "list",
                &account.username,
                "--limit",
                "200",
                "--json",
                "nameWithOwner,url",
            ]);
        }
        if let Some(t) = token {
            cmd.env("GH_TOKEN", t);
        }
        let out = cmd.output().map_err(|e| e.to_string())?;
        if out.status.success() {
            if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                let mut repos = Vec::new();
                for v in arr {
                    let name = v
                        .get("nameWithOwner")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let clone_url = v
                        .get("url")
                        .and_then(|x| x.as_str())
                        .map(|u| {
                            if u.ends_with(".git") {
                                u.to_string()
                            } else {
                                format!("{u}.git")
                            }
                        })
                        .unwrap_or_else(|| format!("https://github.com/{name}.git"));
                    if !name.is_empty() {
                        repos.push(RemoteRepo { name, clone_url });
                    }
                }
                return Ok(repos);
            }
        }
    }

    let url = if account.username.is_empty() {
        "https://api.github.com/user/repos?per_page=100&affiliation=owner,collaborator,organization_member"
            .to_string()
    } else {
        format!(
            "https://api.github.com/users/{}/repos?per_page=100&type=all",
            account.username
        )
    };
    let body = http_get_json(&url, token)?;
    let arr = body
        .as_array()
        .ok_or_else(|| "unexpected GitHub JSON".to_string())?;
    let mut repos = Vec::new();
    for v in arr {
        let name = v
            .get("full_name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let clone_url = v
            .get("clone_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !name.is_empty() && !clone_url.is_empty() {
            repos.push(RemoteRepo { name, clone_url });
        }
    }
    Ok(repos)
}

fn list_gitlab_repos(
    account: &ForgeAccount,
    token: Option<&str>,
) -> Result<Vec<RemoteRepo>, String> {
    if which_exists("glab") {
        let mut cmd = Command::new("glab");
        cmd.args(["repo", "list", "-P", "100"]);
        if let Some(t) = token {
            cmd.env("GITLAB_TOKEN", t);
        }
        let out = cmd.output().map_err(|e| e.to_string())?;
        if out.status.success() {
            let host = if account.host.is_empty() {
                "gitlab.com"
            } else {
                account.host.as_str()
            };
            let mut repos = Vec::new();
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let name = line.split_whitespace().next().unwrap_or("").to_string();
                if name.is_empty() || name.eq_ignore_ascii_case("ID") {
                    continue;
                }
                repos.push(RemoteRepo {
                    clone_url: format!("https://{host}/{name}.git"),
                    name,
                });
            }
            return Ok(repos);
        }
    }
    let host = if account.host.is_empty() {
        "gitlab.com"
    } else {
        account.host.as_str()
    };
    let url = format!("https://{host}/api/v4/projects?membership=true&per_page=100");
    let body = http_get_json(&url, token)?;
    let arr = body
        .as_array()
        .ok_or_else(|| "unexpected GitLab JSON".to_string())?;
    let mut repos = Vec::new();
    for v in arr {
        let name = v
            .get("path_with_namespace")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let clone_url = v
            .get("http_url_to_repo")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !name.is_empty() {
            repos.push(RemoteRepo { name, clone_url });
        }
    }
    Ok(repos)
}

fn list_gitea_repos(
    account: &ForgeAccount,
    token: Option<&str>,
) -> Result<Vec<RemoteRepo>, String> {
    if account.host.is_empty() {
        return Err("forgejo/gitea account needs a host".into());
    }
    let host = account.host.trim_end_matches('/');
    let base = if host.starts_with("http") {
        host.to_string()
    } else {
        format!("https://{host}")
    };
    let url = if account.username.is_empty() {
        format!("{base}/api/v1/user/repos?limit=100")
    } else {
        format!("{base}/api/v1/users/{}/repos?limit=100", account.username)
    };
    let body = http_get_json(&url, token)?;
    let arr = body
        .as_array()
        .ok_or_else(|| "unexpected Gitea JSON".to_string())?;
    let mut repos = Vec::new();
    for v in arr {
        let name = v
            .get("full_name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let clone_url = v
            .get("clone_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !name.is_empty() {
            repos.push(RemoteRepo { name, clone_url });
        }
    }
    Ok(repos)
}

fn http_get_json(url: &str, token: Option<&str>) -> Result<serde_json::Value, String> {
    if !which_exists("curl") {
        return Err("need `gh`/`glab` or `curl` to list remote repos".into());
    }
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", url, "-H", "Accept: application/json"]);
    if let Some(t) = token {
        cmd.args(["-H", &format!("Authorization: Bearer {t}")]);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "HTTP failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

impl SubPlugin for GitPlugin {
    fn id(&self) -> &'static str {
        "git"
    }
    fn title(&self) -> &'static str {
        "Lazy Git"
    }
    fn description(&self) -> &'static str {
        "After you add a forge account, opens on your full remote repo list. Enter clones a repo. Typical flow: Lazy Workspace → Lazy Git → pick repo → clone → work."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload_accounts(ctx);
        self.view = View::Repos;
        self.mode = Mode::Browse;
        self.refresh_repos(ctx);
        ctx.set_status("Enter clone · r refresh · Tab Accounts/Ops · Esc back");
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
                        self.cycle_view();
                        if self.view == View::Repos {
                            self.refresh_repos(ctx);
                        }
                        NavAction::None
                    }
                    KeyCode::Char('r') if self.view == View::Repos => {
                        self.refresh_repos(ctx);
                        NavAction::None
                    }
                    KeyCode::Enter => {
                        match self.view {
                            View::Repos => {
                                if self.repos.is_empty() {
                                    ctx.set_error("no repos to clone");
                                } else if let Some(i) = self.list.selected() {
                                    if i < self.repos.len() {
                                        let parent = Self::default_clone_parent(ctx);
                                        let leaf = self.repos[i]
                                            .name
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or("repo");
                                        self.form = Some(FormState::new(
                                            "Clone repo",
                                            vec![FormField::new("parent_dir")
                                                .with_value(parent), FormField::new("note")
                                                .with_value(format!("→ {leaf}/"))],
                                        ));
                                        self.mode = Mode::FormClone { repo_idx: i };
                                    }
                                }
                            }
                            View::Accounts => {
                                if let Some(i) = self.list.selected() {
                                    if let Some(a) = self.accounts.get(i) {
                                        self.active = Some(a.name.clone());
                                        self.save_accounts(ctx);
                                        self.view = View::Repos;
                                        self.refresh_repos(ctx);
                                    }
                                }
                            }
                            View::Ops => self.activate_ops(ctx),
                        }
                        NavAction::None
                    }
                    KeyCode::Char('n') if self.view == View::Accounts => {
                        self.form = Some(FormState::new(
                            "New forge account",
                            vec![
                                FormField::new("name"),
                                FormField::new("provider").with_value("github"),
                                FormField::new("host").with_value("github.com"),
                                FormField::new("username"),
                                FormField::new("secret_ref"),
                            ],
                        ));
                        self.mode = Mode::FormAccount { editing: None };
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
                                self.mode = Mode::FormAccount {
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
            Mode::FormAccount { editing } => {
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
                        let name = vals.first().cloned().unwrap_or_default().trim().to_string();
                        if name.is_empty() {
                            ctx.set_error("name required");
                        } else {
                            let account = ForgeAccount {
                                name: name.clone(),
                                provider: vals
                                    .get(1)
                                    .cloned()
                                    .unwrap_or_else(|| "github".into()),
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
                            if self.active.is_none() {
                                self.active = Some(name);
                            }
                            self.save_accounts(ctx);
                            self.form = None;
                            self.mode = Mode::Browse;
                            self.view = View::Repos;
                            self.refresh_repos(ctx);
                        }
                    }
                    FormResult::Continue => {}
                }
                NavAction::None
            }
            Mode::FormClone { repo_idx } => {
                let repo_idx = *repo_idx;
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
                        let parent = expand_home(
                            form.values()
                                .first()
                                .map(String::as_str)
                                .unwrap_or(".")
                                .trim(),
                        );
                        self.form = None;
                        self.mode = Mode::Browse;
                        self.clone_repo(ctx, repo_idx, &parent);
                    }
                    FormResult::Continue => {}
                }
                NavAction::None
            }
            Mode::FormCommit => {
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
                        let msg = form.values().first().cloned().unwrap_or_default();
                        if msg.trim().is_empty() {
                            ctx.set_error("commit message required");
                        } else {
                            self.run_git(ctx, &["commit", "-a", "-m", msg.trim()]);
                            self.form = None;
                            self.mode = Mode::Browse;
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
                    if self.active.as_deref() == Some(name.as_str()) {
                        self.active = self.accounts.first().map(|a| a.name.clone());
                    }
                    self.save_accounts(ctx);
                    self.mode = Mode::Browse;
                    self.sync_list();
                    NavAction::None
                }
                ConfirmResult::Continue => NavAction::None,
            },
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        let title = match self.view {
            View::Repos => "Lazy Git · Repos (Enter = clone)",
            View::Accounts => "Lazy Git · Accounts (* = active, Enter = use)",
            View::Ops => "Lazy Git · Local ops",
        };
        draw_select_list(
            frame,
            chunks[0],
            title,
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.log.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Output").borders(Borders::ALL)),
            chunks[1],
        );
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
        if let Mode::Confirm(confirm) = &self.mode {
            confirm.draw(frame, area);
        }
    }
}
