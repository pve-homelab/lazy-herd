//! Export / import / bootstrap for all Lazy Herd configuration.

use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_json, write_json};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Bundle {
    version: u32,
    #[serde(default)]
    settings: Option<Value>,
    #[serde(default)]
    secrets: Option<Value>,
    #[serde(default)]
    git_accounts: Option<Value>,
    #[serde(default)]
    search: Option<Value>,
    #[serde(default)]
    connect: Vec<(String, Value)>,
    #[serde(default)]
    agents: Vec<(String, Value)>,
    #[serde(default)]
    workspaces: Vec<(String, Value)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Export,
    Import,
    Bootstrap,
}

pub struct BootstrapPlugin {
    panel: Panel,
    list: ScrollList,
    pending: Option<Bundle>,
    form: Option<FormState>,
    log: String,
    /// Fine-grained export toggles (name, enabled).
    export_flags: Vec<(String, bool)>,
}

impl BootstrapPlugin {
    pub fn new() -> Self {
        let export_flags = vec![
            ("settings".into(), true),
            ("secrets".into(), true),
            ("git_accounts".into(), true),
            ("search".into(), true),
            ("connect".into(), true),
            ("agents".into(), true),
            ("workspaces".into(), true),
        ];
        Self {
            panel: Panel::Export,
            list: ScrollList::new(
                export_flags
                    .iter()
                    .map(|(n, on)| format!("{} [{}]", n, if *on { "x" } else { " " }))
                    .collect(),
            ),
            pending: None,
            form: None,
            log: String::new(),
            export_flags,
        }
    }

    fn refresh_list(&mut self) {
        match self.panel {
            Panel::Export => {
                self.list.set_items(
                    self.export_flags
                        .iter()
                        .map(|(n, on)| format!("{} [{}]", n, if *on { "x" } else { " " }))
                        .collect(),
                );
            }
            Panel::Import => {
                self.list.set_items(vec![
                    "Load bundle from path…".into(),
                    "Clear pending import".into(),
                    "Show pending summary".into(),
                ]);
            }
            Panel::Bootstrap => {
                self.list.set_items(vec![
                    "Apply pending import now".into(),
                    "Show pending summary".into(),
                ]);
            }
        }
    }

    fn pending_path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_state("bootstrap/pending.json")
    }

    fn load_pending(&mut self, ctx: &mut PluginCtx) {
        match read_json::<Bundle>(&Self::pending_path(ctx)) {
            Ok(v) => self.pending = v,
            Err(e) => ctx.set_error(format!("pending: {e}")),
        }
    }

    fn collect_bundle(&self, ctx: &PluginCtx) -> anyhow::Result<Bundle> {
        let mut bundle = Bundle {
            version: 1,
            ..Default::default()
        };
        let flag = |name: &str| {
            self.export_flags
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, on)| *on)
                .unwrap_or(false)
        };

        if flag("settings") {
            bundle.settings = read_value_file(&ctx.paths.join_config("settings.toml"))?;
        }
        if flag("secrets") {
            bundle.secrets = read_value_file(&ctx.paths.join_config("secrets.json"))?;
        }
        if flag("git_accounts") {
            bundle.git_accounts = read_value_file(&ctx.paths.join_config("git/accounts.toml"))?;
        }
        if flag("search") {
            bundle.search = read_value_file(&ctx.paths.join_config("search.toml"))?;
        }
        if flag("connect") {
            bundle.connect = read_dir_toml_values(&ctx.paths.join_config("connect"))?;
        }
        if flag("agents") {
            bundle.agents = read_dir_toml_values(&ctx.paths.join_config("agents"))?;
        }
        if flag("workspaces") {
            bundle.workspaces = read_dir_toml_values(&ctx.paths.join_config("workspaces"))?;
        }
        Ok(bundle)
    }

    fn write_export(&mut self, ctx: &mut PluginCtx, path: &str) {
        match self.collect_bundle(ctx) {
            Ok(bundle) => match write_json(Path::new(path), &bundle) {
                Ok(()) => {
                    self.log = format!("Exported to {path}");
                    ctx.set_status("export complete");
                }
                Err(e) => ctx.set_error(format!("export write: {e}")),
            },
            Err(e) => ctx.set_error(format!("export: {e}")),
        }
    }

    fn import_file(&mut self, ctx: &mut PluginCtx, path: &str) {
        match read_json::<Bundle>(Path::new(path)) {
            Ok(Some(bundle)) => {
                self.pending = Some(bundle.clone());
                if let Err(e) = write_json(&Self::pending_path(ctx), &bundle) {
                    ctx.set_error(format!("save pending: {e}"));
                } else {
                    self.log = summarize(&bundle);
                    ctx.set_status("import loaded — review then Bootstrap");
                }
            }
            Ok(None) => ctx.set_error("file empty/missing"),
            Err(e) => ctx.set_error(format!("import: {e}")),
        }
    }

    fn apply_pending(&mut self, ctx: &mut PluginCtx) {
        let Some(bundle) = self.pending.clone() else {
            ctx.set_error("no pending import — Import a file first");
            return;
        };
        if let Err(e) = apply_bundle(ctx, &bundle) {
            ctx.set_error(format!("bootstrap: {e}"));
            return;
        }
        self.log = "Bootstrap applied successfully.".into();
        ctx.set_status("bootstrap complete");
    }
}

fn read_value_file(path: &Path) -> anyhow::Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        Ok(Some(serde_json::from_str(&raw)?))
    } else {
        let table: toml::Value = toml::from_str(&raw)?;
        Ok(Some(toml_to_json(table)))
    }
}

fn read_dir_toml_values(dir: &Path) -> anyhow::Result<Vec<(String, Value)>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("item")
            .to_string();
        let raw = fs::read_to_string(&path)?;
        let table: toml::Value = toml::from_str(&raw)?;
        out.push((stem, toml_to_json(table)));
    }
    Ok(out)
}

fn toml_to_json(v: toml::Value) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn json_to_toml_string(v: &Value) -> anyhow::Result<String> {
    let toml_val: toml::Value = serde_json::from_value(v.clone())?;
    Ok(toml::to_string_pretty(&toml_val)?)
}

fn apply_bundle(ctx: &PluginCtx, bundle: &Bundle) -> anyhow::Result<()> {
    if let Some(v) = &bundle.settings {
        fs::write(ctx.paths.join_config("settings.toml"), json_to_toml_string(v)?)?;
    }
    if let Some(v) = &bundle.secrets {
        write_json(&ctx.paths.join_config("secrets.json"), v)?;
    }
    if let Some(v) = &bundle.git_accounts {
        let path = ctx.paths.join_config("git/accounts.toml");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json_to_toml_string(v)?)?;
    }
    if let Some(v) = &bundle.search {
        fs::write(ctx.paths.join_config("search.toml"), json_to_toml_string(v)?)?;
    }
    write_named_tomls(&ctx.paths.join_config("connect"), &bundle.connect)?;
    write_named_tomls(&ctx.paths.join_config("agents"), &bundle.agents)?;
    write_named_tomls(&ctx.paths.join_config("workspaces"), &bundle.workspaces)?;
    Ok(())
}

fn write_named_tomls(dir: &Path, items: &[(String, Value)]) -> anyhow::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    for (name, value) in items {
        fs::write(dir.join(format!("{name}.toml")), json_to_toml_string(value)?)?;
    }
    Ok(())
}

fn summarize(bundle: &Bundle) -> String {
    format!(
        "Pending bundle v{}\nsettings: {}\nsecrets: {}\ngit: {}\nsearch: {}\nconnect: {}\nagents: {}\nworkspaces: {}",
        bundle.version,
        bundle.settings.is_some(),
        bundle.secrets.is_some(),
        bundle.git_accounts.is_some(),
        bundle.search.is_some(),
        bundle.connect.len(),
        bundle.agents.len(),
        bundle.workspaces.len(),
    )
}

impl SubPlugin for BootstrapPlugin {
    fn id(&self) -> &'static str {
        "bootstrap"
    }
    fn title(&self) -> &'static str {
        "Lazy Bootstrap"
    }
    fn description(&self) -> &'static str {
        "Export / Import / Bootstrap all Lazy Herd settings. Fine-grained export toggles; import stages a pending bundle; bootstrap applies it."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.load_pending(ctx);
        self.panel = Panel::Export;
        self.refresh_list();
        ctx.set_status("Tab panels · Space toggle · Enter action · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        if self.form.is_some() {
            let Some(form) = self.form.as_mut() else {
                return NavAction::None;
            };
            match form.handle(key) {
                FormResult::Cancel => self.form = None,
                FormResult::Submit => {
                    let path = form.values().first().cloned().unwrap_or_default();
                    match self.panel {
                        Panel::Export => self.write_export(ctx, &path),
                        Panel::Import => self.import_file(ctx, &path),
                        Panel::Bootstrap => {}
                    }
                    self.form = None;
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
            KeyCode::Tab => {
                self.panel = match self.panel {
                    Panel::Export => Panel::Import,
                    Panel::Import => Panel::Bootstrap,
                    Panel::Bootstrap => Panel::Export,
                };
                self.refresh_list();
                NavAction::None
            }
            KeyCode::Char(' ') if self.panel == Panel::Export => {
                if let Some(i) = self.list.selected() {
                    if let Some((_, on)) = self.export_flags.get_mut(i) {
                        *on = !*on;
                    }
                    self.refresh_list();
                }
                NavAction::None
            }
            KeyCode::Enter => {
                match self.panel {
                    Panel::Export => {
                        self.form = Some(FormState::new(
                            "Export path",
                            vec![FormField::new("path").with_value("lazy-herd-backup.json")],
                        ));
                    }
                    Panel::Import => match self.list.selected() {
                        Some(0) => {
                            self.form = Some(FormState::new(
                                "Import path",
                                vec![FormField::new("path").with_value("lazy-herd-backup.json")],
                            ));
                        }
                        Some(1) => {
                            self.pending = None;
                            let _ = fs::remove_file(Self::pending_path(ctx));
                            self.log = "Pending cleared".into();
                            ctx.set_status("pending cleared");
                        }
                        Some(2) => {
                            self.log = self
                                .pending
                                .as_ref()
                                .map(summarize)
                                .unwrap_or_else(|| "No pending import".into());
                        }
                        _ => {}
                    },
                    Panel::Bootstrap => match self.list.selected() {
                        Some(0) => self.apply_pending(ctx),
                        Some(1) => {
                            self.log = self
                                .pending
                                .as_ref()
                                .map(summarize)
                                .unwrap_or_else(|| "No pending import".into());
                        }
                        _ => {}
                    },
                }
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let title = match self.panel {
            Panel::Export => "Bootstrap · Export (Tab)",
            Panel::Import => "Bootstrap · Import (Tab)",
            Panel::Bootstrap => "Bootstrap · Apply (Tab)",
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
                .block(Block::default().title("Log / summary").borders(Borders::ALL)),
            chunks[1],
        );
        if let Some(form) = &self.form {
            form.draw(frame, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Paths;
    use tempfile::tempdir;

    #[test]
    fn export_import_round_trip() {
        let dir = tempdir().unwrap();
        let config = dir.path().join("config");
        let state = dir.path().join("state");
        fs::create_dir_all(config.join("connect")).unwrap();
        fs::write(
            config.join("secrets.json"),
            r#"{"secrets":[{"name":"t","value":"x","note":""}]}"#,
        )
        .unwrap();
        fs::write(
            config.join("connect/box.toml"),
            "name = \"box\"\nhost = \"h\"\nuser = \"u\"\nport = 22\n",
        )
        .unwrap();

        let paths = Paths {
            config_dir: config.clone(),
            state_dir: state,
        };
        let ctx = PluginCtx::new(paths);
        let plugin = BootstrapPlugin::new();
        let bundle = plugin.collect_bundle(&ctx).unwrap();
        assert!(bundle.secrets.is_some());
        assert_eq!(bundle.connect.len(), 1);

        let out = dir.path().join("bundle.json");
        write_json(&out, &bundle).unwrap();
        let loaded: Bundle = read_json(&out).unwrap().unwrap();
        assert_eq!(loaded.connect.len(), 1);
    }
}
