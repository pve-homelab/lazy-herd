# Lazy Herd — Design Spec

**Date:** 2026-09-14  
**Status:** Approved by user mandate (end-to-end, no further input)

## Problem

Herdr has many useful workflows (git, workspaces, secrets, SSH, agents, bootstrap, search, health, config) spread across separate plugins and manual setup. The user wants one memorable Rust plugin — **Lazy Herd** — that hosts those workflows as scrollable sub-plugins behind a single TUI menu.

## Goals

1. One Herdr plugin (`lazy-herd`) invoked as a popup/overlay pane.
2. Split-pane menu: left = scrollable sub-plugins (+ Docs); right = description.
3. Enter opens a sub-plugin; Esc backs out (root Esc closes the pane).
4. Sub-plugins are registry modules — add/remove by editing one registry file.
5. Durable config under `HERDR_PLUGIN_CONFIG_DIR`; runtime under `HERDR_PLUGIN_STATE_DIR`.
6. Prefer pure TUI; integrate forked `herdr-board` and `terminal-browser` when present.
7. Platforms: linux, macos, windows (Herdr 0.9.0+).

## Non-goals

- Vendoring the full herdr-board daemon into this binary.
- Replacing Herdr’s native Settings UI.
- Perfect forge API coverage for every Git host edge case.

## Architecture

```
herdr-plugin.toml
  [[actions]] open     → opens [[panes]] menu
  [[panes]]   menu     → ./bin/lazy-herd (popup ~90%x80%)

lazy-herd binary
  App (ratatui)
    Screen::Menu | Screen::Docs | Screen::Plugin(id)
  PluginRegistry: Vec<Box<dyn SubPlugin>>
  Storage: TOML/JSON under plugin config/state dirs
  HerdrCli: wraps HERDR_BIN_PATH
```

### SubPlugin contract

```rust
trait SubPlugin {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> Action;
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &PluginCtx);
}
```

Adding a sub-plugin = new module + one `registry.register(...)` line.

## Sub-plugins

| ID | Behavior |
|----|----------|
| docs | Built-in command/guide reader (always first or last menu item) |
| git | Forge accounts (GitHub/GitLab/Forgejo/Gitea/local), repo list via `gh`/`glab`/git remote, pull/push/commit/branch ops |
| workspace | Named workspace templates (tabs/panes/agents/board flag); apply via Herdr CLI |
| secrets | Named secrets store (masked values); create/edit/delete with typed name confirm |
| connect | SSH profiles; create/edit/delete; “open” runs `ssh` in a new Herdr pane when possible |
| agents | Custom agent defs (name, kind, master prompt, args); start via `herdr agent start` |
| bootstrap | Export / Import / Bootstrap multi-panel for all Lazy Herd stores |
| search | Launch terminal-browser or open URL; config for binary path / plugin entry |
| doctor | Health checks: herdr binary, socket, plugins, git, ssh, board, browser |
| config | Edit Lazy Herd settings + show pointers to Herdr config |

CRUD confirmation: delete always requires typing the item name.

## Navigation

- Menu: `↑/↓` `j/k` move, `Enter` open, `?` or Docs entry for guide, `Esc`/`q` quit.
- Sub-plugin: `Esc` returns to menu (does not quit).
- Forms: `Tab` fields, `Enter` save, `Esc` cancel.

## Integration strategy

- **Kanban:** If `herdr-board` plugin is linked, Doctor/Workspace can open its pane via `herdr plugin pane open --plugin herdr-board --entrypoint board`. Workspace templates may set `open_board = true`.
- **Terminal browser:** Search tries (1) configured binary, (2) `terminal-browser` on PATH, (3) `herdr plugin action invoke` for `zenbu-labs.terminal-browser.open-split`.

## Storage layout

```
$HERDR_PLUGIN_CONFIG_DIR/
  settings.toml
  secrets.json          # values never logged
  connect/*.toml
  agents/*.toml
  workspaces/*.toml
  git/accounts.toml
  search.toml
$HERDR_PLUGIN_STATE_DIR/
  bootstrap/pending.json
  doctor-last.json
```

## Testing

- Unit tests for registry, storage CRUD, delete-confirm, bootstrap round-trip.
- `cargo test` + `cargo build --release`.
- Manual: `herdr plugin link lazy-herd` then invoke action.

## Success criteria

- Plugin links and opens a working split menu.
- All nine feature sub-plugins + Docs are reachable and perform create/list/edit/delete (or health/config/search launch) without crashing.
- README documents install, keys, and how to add a sub-plugin.
