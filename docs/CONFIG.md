# Configure Lazy Herd (TUI + files)

Paths:

```bash
herdr plugin config-dir lazy-herd
```

Typical layout:

```text
settings.toml
secrets.json
search.toml
git/accounts.toml
connect/*.toml
agents/*.toml
workspaces/*.toml
```

## Menu keys (all features)

| Key | Action |
|---|---|
| `↑` `↓` / `j` `k` | Move |
| `Enter` | Open / apply |
| `n` | New (where supported) |
| `e` | Edit |
| `d` | Delete (type name to confirm) |
| `Esc` | Back to Lazy Herd menu |

---

## Lazy Workspace

### Via TUI

1. Open **Lazy Workspace**
2. `i` → install **coding-loop** example template
3. `e` → edit name / description / `working_dir` / `open_board`
4. For full pane/agent setup, edit the TOML on disk (TUI form is meta only)
5. `Enter` → create Herdr workspace, split panes, start agents, optionally open board

### Example: coding-loop (Orchestrator / Dev / Test / Audit + Board)

File: `workspaces/coding-loop.toml`

```toml
name = "coding-loop"
description = "Orchestrator + Dev + Test + Audit + Board"
working_dir = "~/repos/my-project"
label = "coding-loop"
open_board = true

[[panes]]
name = "Orchestrator"
agent_kind = "codex"
master_prompt = """
You are Orchestrator. You do NOT write app code.
1) Receive updates from Dev, Test, and Audit.
2) Keep the kanban board accurate: create/move cards for coding work.
3) Unstick agents: if someone is idle or blocked, nudge them.
4) Own the loop: code → test → audit → board update → next task.
"""

[[panes]]
name = "Dev"
agent_kind = "codex"
direction = "right"
master_prompt = """
You are Dev. Implement features/fixes from Orchestrator tasks.
When done, summarize changes and notify Orchestrator.
"""

[[panes]]
name = "Test"
agent_kind = "codex"
direction = "down"
master_prompt = """
You are Test. Write/run tests, find bugs, report clearly to Audit + Orchestrator.
"""

[[panes]]
name = "Audit"
agent_kind = "codex"
direction = "right"
master_prompt = """
You are Audit. Review Test results. Send Orchestrator a neat fix report.
"""
```

Flow:

1. Set `working_dir` to a real repo (or clone one with Lazy Git first)
2. Set `agent_kind` to whatever Herdr supports on your machine (`codex`, `cursor`, `claude`, …)
3. Apply from Lazy Workspace
4. Open **Lazy Board** (needs `herdr-board` installed — Linux/macOS today)

---

## Lazy Board

Separate Herdr plugin (`herdr-board`). Lazy Board just opens it.

```bash
# Linux/macOS
herdr plugin install <owner>/herdr-board
```

Then: Lazy Herd → **Lazy Board** → Enter.

---

## Lazy Git

1. **Lazy Secrets** → add PAT (`n`)
2. Lazy Git → Tab → **Accounts** → `n` (set `secret_ref` to that secret name)
3. Enter account to make it active
4. Tab back to **Repos** → Enter clones → opens workspace in that folder

---

## Lazy Search

Built into Lazy Herd (no other plugin):

| Menu item | What |
|---|---|
| Browse default URL | In-app text browser |
| Browse custom URL | Same |
| System browser | OS default browser |

In browser: `j/k` scroll · `Tab` links · `Enter` follow · `b` back · `o` OS browser · `Esc` menu

---

## Lazy Connect / Agents / Secrets / Config

Same CRUD pattern: list → Enter manage (Connect) or `n`/`e`/`d`.  
Agents: presets with `kind` + `master_prompt`.  
Config: theme / default clone parent dir.
