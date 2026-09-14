# Lazy Herd

### One [Herdr](https://herdr.dev) plugin

## Open **Lazy Herd** and pick from a suite of _Lazy_ features served in a split-pane menu.

#### **Plugin id:** `lazy-herd`  
#### **Requires:** Herdr ≥ 0.9.0 · Rust/`cargo` on the machine for install builds

## Install

```bash
herdr plugin install pve-homelab/lazy-herd --yes
```

**Local / linked:**

```bash
# in this repo
powershell -File scripts/build.ps1   # Windows
# bash scripts/build.sh              # Linux/macOS
herdr plugin link .
```

## Run

```bash
# Windows
herdr plugin action invoke lazy-herd.open-windows

# Linux / macOS
herdr plugin action invoke lazy-herd.open
```

Optional keybind (`~/.config/herdr/config.toml`):

```toml
[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "lazy-herd.open-windows"   # Linux/macOS: lazy-herd.open
```

## Menu keys

| Key | Do |
|---|---|
| `↑` `↓` / `j` `k` | Move |
| `Enter` | Open |
| `?` | Docs |
| `Esc` | Back (or quit on main menu) |

## Features

| Feature | What it does |
|---|---|
| **Lazy Git** | Lists your remote repos. Enter → clone → opens a focused Herdr workspace in that folder → closes Lazy Herd so you’re already there. |
| **Lazy Workspace** | Premade multi-pane workspaces (agents + optional board). `i` installs coding-loop example. |
| **Lazy Board** | Opens herdr-board kanban when that plugin is installed (Linux/macOS today). |
| **Lazy Secrets** | Store PATs/tokens (masked). |
| **Lazy Connect** | SSH profiles. Enter → manage pane (create / edit / delete / connect). |
| **Lazy Agents** | Agent presets (kind + master prompt). |
| **Lazy Bootstrap** | Export / import / bootstrap all Lazy Herd config. |
| **Lazy Search** | Built-in text browser inside Lazy Herd (no extra plugin). Optional system-browser open. |
| **Lazy Doctor** | Health check. |
| **Lazy Config** | Lazy Herd settings. |

Config dir: `herdr plugin config-dir lazy-herd`

## CLI vs desktop (what works where)

Lazy Herd itself is a **keyboard TUI** (ratatui). It works on:

| Environment | Works? | Notes |
|---|---|---|
| **Linux server (SSH / no mouse)** | Yes | Fully keyboard-driven. Herdr mouse is optional. |
| **macOS / Windows desktop** | Yes | Same keys; mouse in *Herdr* chrome is fine, menu is still keys. |
| **Headless / no TTY** | No | Needs an interactive terminal pane. |

### Feature limits

| Feature | Limit |
|---|---|
| **Lazy Git** | Needs `git`. Repo list needs `gh` (GitHub) and/or `curl` + PAT, or `glab` (GitLab). |
| **Lazy Connect** | Needs `ssh`. “Connect” uses `herdr pane split` (needs a live Herdr session). |
| **Lazy Search** | Built-in. Needs network. System browser is optional. |
| **Lazy Workspace + board** | Optional `herdr-board` is Linux/macOS-oriented; may be missing on Windows. |
| **Lazy Agents start** | Best from a real Herdr pane (popup panes may lack `HERDR_PANE_ID`). |

## Quick doctor

```bash
lazy-herd doctor
```
