//! Lazy Board — open the Herdr kanban (herdr-board) when installed.

use crate::herdr::{run_herdr_ok, which_exists};
use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::ui::draw_select_list;
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub struct BoardPlugin {
    list: ScrollList,
    log: String,
}

impl BoardPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::new(vec![
                "Open herdr-board (kanban overlay)".into(),
                "Invoke herdr-board.open-board action".into(),
                "Show install / platform notes".into(),
            ]),
            log: String::new(),
        }
    }

    fn notes() -> String {
        r#"Lazy Board wraps the separate `herdr-board` Herdr plugin.

Install (Linux/macOS — herdr-board is not Windows yet):
  herdr plugin install <owner>/herdr-board
  # or link a local checkout of herdr-board

Then from here:
  Enter on "Open herdr-board" → plugin pane open
  Cards = agent prompts; columns = pipeline stages

In a Lazy Workspace template set:
  open_board = true
so applying that workspace also summons the board.

Role with coding-loop workspace:
  Orchestrator keeps the board moving from agent updates.
  Dev / Test / Audit do the work and report back."#
            .into()
    }

    fn open_pane(&mut self, ctx: &mut PluginCtx) {
        match run_herdr_ok(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            "herdr-board",
            "--entrypoint",
            "board",
        ]) {
            Ok(out) => {
                self.log = format!("Opened herdr-board pane.\n{out}");
                ctx.set_status("board opened");
            }
            Err(e) => {
                self.log = format!("{}\n\nError:\n{e}", Self::notes());
                ctx.set_error("herdr-board not available — see panel");
            }
        }
    }

    fn open_action(&mut self, ctx: &mut PluginCtx) {
        match run_herdr_ok(&[
            "plugin",
            "action",
            "invoke",
            "herdr-board.open-board",
        ]) {
            Ok(out) => {
                self.log = format!("Invoked open-board.\n{out}");
                ctx.set_status("board action ok");
            }
            Err(e) => {
                self.log = format!("{}\n\nError:\n{e}", Self::notes());
                ctx.set_error("herdr-board action failed — see panel");
            }
        }
    }
}

impl SubPlugin for BoardPlugin {
    fn id(&self) -> &'static str {
        "board"
    }
    fn title(&self) -> &'static str {
        "Lazy Board"
    }
    fn description(&self) -> &'static str {
        "Kanban for agent tasks (herdr-board). Open the board overlay; pair with Lazy Workspace coding-loop so Orchestrator can keep cards moving."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.log = Self::notes();
        let board_bin = which_exists("board") || which_exists("herdr-board");
        ctx.set_status(if board_bin {
            "board CLI on PATH · Enter to open"
        } else {
            "Enter open · install herdr-board if this fails (Linux/macOS)"
        });
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        if self.list.handle_nav(key) {
            return NavAction::None;
        }
        match key.code {
            KeyCode::Esc => NavAction::Back,
            KeyCode::Enter => {
                match self.list.selected() {
                    Some(0) => self.open_pane(ctx),
                    Some(1) => self.open_action(ctx),
                    Some(2) => {
                        self.log = Self::notes();
                        ctx.set_status("notes refreshed");
                    }
                    _ => {}
                }
                NavAction::None
            }
            _ => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        draw_select_list(
            frame,
            chunks[0],
            "Lazy Board",
            &self.list.items,
            self.list.selected(),
        );
        frame.render_widget(
            Paragraph::new(self.log.as_str())
                .wrap(Wrap { trim: false })
                .block(Block::default().title("Notes").borders(Borders::ALL)),
            chunks[1],
        );
    }
}
