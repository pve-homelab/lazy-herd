# Lazy Herd

One [Herdr](https://herdr.dev) plugin: open **Lazy Herd** and pick from Git, Workspace, Secrets, Connect, Agents, Bootstrap, Search, Doctor, Config (and Docs) in a single split-pane menu.

**Plugin id:** `lazy-herd`  
**Requires:** Herdr ≥ 0.9.0 · Rust/`cargo` on the machine for install builds

## Install from GitHub

```bash
herdr plugin install pve-homelab/lazy-herd --yes
```

Herdr clones this repo, runs the manifest `[[build]]` step, and registers the plugin. Then open it:

```bash
# Linux / macOS
herdr plugin action invoke lazy-herd.open

# Windows
herdr plugin action invoke lazy-herd.open-windows
```

### Local development

```bash
# from this repo root
powershell -File scripts/build.ps1   # or: bash scripts/build.sh
herdr plugin link .
```

### Keybinding

```toml
[[keys.command]]
key = "prefix+l"
type = "plugin_action"
command = "lazy-herd.open"            # Windows: lazy-herd.open-windows
description = "open Lazy Herd"
```

## Menu

| Key | Action |
|---|---|
| `↑`/`↓` or `j`/`k` | Move between features |
| `Enter` | Open feature |
| `?` | Docs |
| `Esc` / `q` | Quit (closes popup) |

Inside a feature, **Esc** returns to the Lazy Herd menu.

| Feature | Purpose |
|---|---|
| Docs | Command guide |
| Lazy Git | Forge accounts + git ops |
| Lazy Workspace | Premade Herdr workspace templates |
| Lazy Secrets | Masked token store |
| Lazy Connect | SSH profiles |
| Lazy Agents | Agent presets |
| Lazy Bootstrap | Export / Import / Bootstrap |
| Lazy Search | Terminal browser launcher |
| Lazy Doctor | Health checks |
| Lazy Config | Settings + Herdr hints |

## Layout (Herdr plugin format)

```text
.
├── herdr-plugin.toml    # required manifest (repo root)
├── Cargo.toml
├── scripts/build.*      # install-time build
├── scripts/open.*       # action → opens menu pane
├── bin/lazy-herd[.exe]  # produced by build
└── src/                 # Rust TUI + sub-plugins
```

Config: `herdr plugin config-dir lazy-herd`

## License

MIT
