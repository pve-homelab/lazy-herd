# Lazy Herd

One [Herdr](https://herdr.dev) plugin: open **Lazy Herd** and pick from Git, Workspace, Secrets, Connect, Agents, Bootstrap, Search, Doctor, Config (and Docs) in a single split-pane menu.

**Plugin id:** `lazy-herd`  
**Requires:** Herdr ≥ 0.9.0 · Rust/`cargo` on the machine for install builds

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
# Linux / macOS
herdr plugin action invoke lazy-herd.open

# Windows
herdr plugin action invoke lazy-herd.open-windows
```

Optional keybind (`~/.config/herdr/config.toml`):

```toml
[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "lazy-herd.open"   # Windows: lazy-herd.open-windows
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
| **Lazy Git** | After you add a forge account (+ PAT in Secrets), always shows your remote repos. Enter → clone. Flow: Workspace → Git → pick repo → clone → work. |
| **Lazy Workspace** | Premade Herdr workspace templates (apply with Enter). Still evolving — try it and tweak. |
| **Lazy Secrets** | Store PATs/tokens (masked). |
| **Lazy Connect** | SSH profiles. Enter → manage pane (create / edit / delete / connect). |
| **Lazy Agents** | Agent presets (kind + master prompt). |
| **Lazy Bootstrap** | Export / import / bootstrap all Lazy Herd config. |
| **Lazy Search** | Launch terminal-browser (if installed). |
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
| **Lazy Search** | Needs `terminal-browser` binary **or** that Herdr plugin. Often **not** on minimal Linux servers. |
| **Lazy Workspace + board** | Optional `herdr-board` is Linux/macOS-oriented; may be missing on Windows. |
| **Lazy Agents start** | Best from a real Herdr pane (popup panes may lack `HERDR_PANE_ID`). |

## Quick doctor

```bash
lazy-herd doctor
```
