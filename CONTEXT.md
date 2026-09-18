# Workspace Launcher

A herdr plugin that opens a popup form to start a new herdr workspace in a chosen directory with a chosen coding agent.

## Language

**Launcher**:
The popup form the user fills in to start a new workspace.
_Avoid_: dialog, wizard

**Target Directory**:
The directory the new workspace opens in, usually chosen from zoxide's list.
_Avoid_: project, folder

**Harness**:
A coding-agent CLI (for example Opencode, Codex, Claude, pi or omp) launched in the new workspace. "Shell" is the Harness that launches nothing extra.
_Avoid_: agent, tool, CLI

**Available Harness**:
A **Harness** whose command is on PATH right now. Unavailable ones are shown but can't be picked.

**Launch Mode**:
Whether the workspace opens the **Target Directory** as it is (**In Place**) or in a new git worktree for a named branch (**Worktree**). A **Target Directory** that isn't a git repo only allows **In Place**.
_Avoid_: "workspace" as the name of the mode (that word means the herdr workspace)

**Title**:
The label given to the new herdr workspace. It follows the **Target Directory** (`basename`, or `basename:branch` in **Worktree** mode) until the user edits it.
_Avoid_: name

## Relationships

- A **Launcher** submission produces exactly one herdr workspace running one **Harness**
- **In Place** never changes the checked-out branch of the **Target Directory**
- **Worktree** mode creates the named branch from the repo's default branch if it doesn't exist yet; an existing branch gets a new worktree; a branch that already has a worktree gets that worktree reopened
- **In Place** is the default **Launch Mode**; submitting always makes a new workspace, even if one is already open on the **Target Directory**
- Reopening a worktree that is already open in herdr just focuses it; no **Harness** is launched into it
- The preselected **Harness** is the last one launched, as long as it is still an **Available Harness**

## Example dialogue

> **Dev:** "If I pick a repo on `main` and type `feat/login`, what happens?"
> **Domain expert:** "That only makes sense in **Worktree** mode. The Launcher creates a worktree on `feat/login`, and **In Place** would just show `main` read-only."

## Flagged ambiguities

- "use a workspace instead" in the original ask meant a git **Worktree**, not a herdr workspace.
