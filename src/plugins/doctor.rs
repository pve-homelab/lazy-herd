//! Environment and plugin health checks.

use crate::herdr::{herdr_bin, run_herdr, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::write_json;
use crate::ui::draw_select_list;
use crate::ui::ScrollList;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub struct DoctorPlugin {
    list: ScrollList,
    checks: Vec<Check>,
    detail: String,
}

impl DoctorPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::default(),
            checks: Vec::new(),
            detail: String::new(),
        }
    }

    fn run_checks(&mut self, ctx: &mut PluginCtx) {
        self.checks = gather_checks(&ctx.paths.config_dir, &ctx.paths.state_dir);
        self.list.set_items(
            self.checks
                .iter()
                .map(|c| {
                    format!(
                        "{} {}",
                        if c.ok { "OK " } else { "FAIL" },
                        c.name
                    )
                })
                .collect(),
        );
        let _ = write_json(&ctx.paths.join_state("doctor-last.json"), &self.checks);
        let fails = self.checks.iter().filter(|c| !c.ok).count();
        ctx.set_status(format!(
            "{} checks · {} failed · Enter for detail · r re-run",
            self.checks.len(),
            fails
        ));
        self.sync_detail();
    }

    fn sync_detail(&mut self) {
        self.detail = self
            .list
            .selected()
            .and_then(|i| self.checks.get(i))
            .map(|c| format!("{}\n\n{}", c.name, c.detail))
            .unwrap_or_default();
    }
}

pub fn gather_checks(config_dir: &PathBuf, state_dir: &PathBuf) -> Vec<Check> {
    let mut checks = Vec::new();

    let bin = herdr_bin();
    match run_herdr(&["--version"]) {
        Ok(out) if out.status.success() => {
            checks.push(Check {
                name: "herdr binary".into(),
                ok: true,
                detail: format!(
                    "{bin}\n{}",
                    String::from_utf8_lossy(&out.stdout).trim()
                ),
            });
        }
        Ok(out) => checks.push(Check {
            name: "herdr binary".into(),
            ok: false,
            detail: format!("{bin} failed: {}", String::from_utf8_lossy(&out.stderr)),
        }),
        Err(e) => checks.push(Check {
            name: "herdr binary".into(),
            ok: false,
            detail: e.to_string(),
        }),
    }

    let socket = std::env::var_os("HERDR_SOCKET_PATH").is_some();
    let in_herdr = socket || std::env::var_os("HERDR_ENV").is_some();
    checks.push(Check {
        name: "HERDR_SOCKET_PATH".into(),
        ok: true,
        detail: if in_herdr {
            format!("{:?}", std::env::var_os("HERDR_SOCKET_PATH"))
        } else {
            "Not set — expected when running outside a Herdr session".into()
        },
    });

    match run_herdr(&["plugin", "list", "--json"]) {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout);
            let has_board = body.contains("herdr-board");
            let has_browser = body.contains("terminal-browser");
            checks.push(Check {
                name: "herdr plugin list".into(),
                ok: true,
                detail: body.chars().take(500).collect(),
            });
            checks.push(Check {
                name: "herdr-board plugin (optional)".into(),
                ok: true,
                detail: if has_board {
                    "Detected in plugin list".into()
                } else {
                    "Not installed — workspace open_board will no-op".into()
                },
            });
            checks.push(Check {
                name: "terminal-browser plugin (optional)".into(),
                ok: true,
                detail: if has_browser {
                    "Detected in plugin list".into()
                } else {
                    "Not installed — Search falls back to configured binary".into()
                },
            });
        }
        Ok(out) => checks.push(Check {
            name: "herdr plugin list".into(),
            ok: false,
            detail: String::from_utf8_lossy(&out.stderr).to_string(),
        }),
        Err(e) => checks.push(Check {
            name: "herdr plugin list".into(),
            ok: false,
            detail: e.to_string(),
        }),
    }

    for (name, cmd, required) in [
        ("git", "git", true),
        ("ssh", "ssh", true),
        ("gh (optional)", "gh", false),
        ("glab (optional)", "glab", false),
        ("terminal-browser binary (optional)", "terminal-browser", false),
    ] {
        let ok = which_exists(cmd);
        checks.push(Check {
            name: name.into(),
            ok: ok || !required,
            detail: if ok {
                format!("{cmd} found on PATH")
            } else if required {
                format!("{cmd} missing")
            } else {
                format!("{cmd} not found (optional)")
            },
        });
    }

    checks.push(Check {
        name: "plugin config dir".into(),
        ok: config_dir.exists(),
        detail: config_dir.display().to_string(),
    });
    checks.push(Check {
        name: "plugin state dir".into(),
        ok: state_dir.exists(),
        detail: state_dir.display().to_string(),
    });

    match run_herdr(&["agent", "list"]) {
        Ok(out) if out.status.success() => checks.push(Check {
            name: "agent list".into(),
            ok: true,
            detail: String::from_utf8_lossy(&out.stdout).chars().take(400).collect(),
        }),
        Ok(out) => checks.push(Check {
            name: "agent list".into(),
            ok: false,
            detail: String::from_utf8_lossy(&out.stderr).to_string(),
        }),
        Err(e) => checks.push(Check {
            name: "agent list".into(),
            ok: false,
            detail: e.to_string(),
        }),
    }

    checks
}

pub fn run_cli(json: bool) -> Result<()> {
    let paths = crate::storage::Paths::resolve();
    let checks = gather_checks(&paths.config_dir, &paths.state_dir);
    if json {
        println!("{}", serde_json::to_string_pretty(&checks)?);
    } else {
        for c in &checks {
            println!(
                "[{}] {} — {}",
                if c.ok { "OK" } else { "FAIL" },
                c.name,
                c.detail.lines().next().unwrap_or("")
            );
        }
    }
    let failed = checks.iter().any(|c| !c.ok);
    if failed {
        std::process::exit(1);
    }
    Ok(())
}

impl SubPlugin for DoctorPlugin {
    fn id(&self) -> &'static str {
        "doctor"
    }
    fn title(&self) -> &'static str {
        "Lazy Doctor"
    }
    fn description(&self) -> &'static str {
        "Herdr + agent + toolchain health check. Verifies binary, plugins, git/ssh, board, and browser availability."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.run_checks(ctx);
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        if self.list.handle_nav(key) {
            self.sync_detail();
            return NavAction::None;
        }
        match key.code {
            KeyCode::Esc => NavAction::Back,
            KeyCode::Char('r') => {
                self.run_checks(ctx);
                NavAction::None
            }
            KeyCode::Enter => {
                self.sync_detail();
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        draw_select_list(
            frame,
            chunks[0],
            "Lazy Doctor · r refresh",
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.detail.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Detail").borders(Borders::ALL)),
            chunks[1],
        );
    }
}
