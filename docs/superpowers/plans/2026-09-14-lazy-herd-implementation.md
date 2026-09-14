# Lazy Herd Implementation Plan

> **For agentic workers:** Implement task-by-task. Steps use checkbox syntax.

**Goal:** Ship a working Herdr Rust plugin `lazy-herd` with a split-pane menu and all Prompt.md sub-plugins.

**Architecture:** Single `lazy-herd` binary (ratatui) launched as a Herdr popup pane; sub-plugins registered via a trait registry; TOML/JSON storage under Herdr plugin dirs.

**Tech Stack:** Rust 2021, ratatui 0.28, crossterm 0.28, serde/toml/serde_json, clap, anyhow, thiserror, uuid, herdr CLI via `HERDR_BIN_PATH`.

## Global Constraints

- Plugin id: `lazy-herd`
- min_herdr_version: `0.9.0`
- Platforms: linux, macos, windows
- Delete requires typing the item name
- Esc from root quits; Esc from sub-plugin returns to menu
- No secrets in logs or README examples with real tokens

---

## Task 1: Scaffold crate + manifest

**Files:**
- `lazy-herd/Cargo.toml`
- `lazy-herd/herdr-plugin.toml`
- `lazy-herd/scripts/build.sh`
- `lazy-herd/scripts/build.ps1`
- `lazy-herd/src/main.rs` (stub)

- [ ] Create package + build scripts producing `bin/lazy-herd` / `bin/lazy-herd.exe`
- [ ] Manifest: action `open`, pane `menu` (popup 90%x80%), Windows powershell twin entries
- [ ] `cargo build` succeeds

## Task 2: Core TUI shell + registry

**Files:**
- `lazy-herd/src/{app,registry,storage,herdr,ui/*}.rs`
- `lazy-herd/src/plugins/mod.rs`

- [ ] Implement `SubPlugin` trait + `PluginRegistry`
- [ ] Split menu (list left, description right) + Docs entry
- [ ] Screen stack: Menu ↔ Plugin
- [ ] Storage path resolution (env → fallback `~/.config/herdr/plugins/...`)

## Task 3: Sub-plugins

Implement each under `src/plugins/`:

- [ ] docs, secrets, connect, agents (CRUD + confirm delete)
- [ ] git (accounts + local git ops)
- [ ] workspace (templates + apply via herdr)
- [ ] bootstrap (export/import/bootstrap)
- [ ] search (browser launch + config)
- [ ] doctor (health checks)
- [ ] config (settings editor)

## Task 4: Tests, README, root docs

- [ ] Unit tests for storage + bootstrap round-trip + registry ids
- [ ] `lazy-herd/README.md` + root README update
- [ ] `cargo test` and `cargo build --release` pass
