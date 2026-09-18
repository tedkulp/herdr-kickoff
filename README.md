# Kickoff

A herdr plugin that opens a popup form for starting a new workspace:

1. **Directory**: pick from `zoxide query --list`, filtered like `zoxide query -i` (every word must appear in the path, zoxide ranking kept), or type a `/path` or `~/path`
2. **Harness**: Opencode, Codex, Claude, pi, omp or Shell. Harnesses that aren't on PATH are greyed out, and PATH is re-checked live.
3. **Branch**: *In place* shows the current branch. *Worktree* creates a worktree for the branch you type:
   - a new branch starts from the default branch
   - an existing branch gets a worktree
   - a branch that already has a worktree gets that worktree reopened
4. **Title**: the workspace label. It defaults to the directory name (`name:branch` for worktrees) until you edit it.

Submitting creates the workspace, focuses it, starts the harness in its first pane and closes the popup.
The last harness you used is remembered.

## Keys

| Key | Action |
| --- | --- |
| Tab / Shift-Tab, ↑↓ | move between fields |
| Enter | open the directory picker / pick / next field; on Title it launches |
| ←→ (Harness) | choose harness |
| ←→ or `w` (Branch) | switch between In place and Worktree |
| Ctrl-S | launch |
| Ctrl-U | clear the current text |
| Esc | close the picker, or cancel |

## Install (local)

```sh
cargo build --release --locked
herdr plugin link .
```

```toml
# ~/.config/herdr/config.toml
[[keys.command]]
key = "prefix+shift+n"
type = "plugin_action"
command = "kickoff.open"
description = "new workspace (kickoff)"
```

## Config

`$(herdr plugin config-dir kickoff)/config.toml`:

```toml
default_harness = "Claude"      # used when there's no remembered harness

[[harnesses]]                   # a name matching a built-in replaces it; new names are added before Shell
name = "Claude"
command = "claude --continue"
```
