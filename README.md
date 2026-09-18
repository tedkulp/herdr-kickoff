# Kickoff

A herdr plugin that opens a popup form for starting a new workspace:

1. **Directory**: pick from `zoxide query --list`, filtered like `zoxide query -i` (every word must appear in the path, zoxide ranking kept), or type a `/path` or `~/path`
2. **Harness**: Opencode, Codex, Claude, pi, omp or Shell. Harnesses that aren't on PATH are greyed out, and PATH is re-checked live.
3. **Branch**: pick from the local branches (current first, then most recently committed), filtered the same way as directories. Branches checked out in a worktree are tagged `⎇ worktree`. Typing a name that doesn't exist creates it from the default branch.
   - *In place* opens the directory itself. Leave the current branch to open it as is; pick another branch and the directory is switched to it (`git switch`) before launching. Uncommitted changes come along if git allows it; if the switch fails, git's error is shown and nothing launches. A branch checked out in another worktree needs *Worktree* mode.
   - *Worktree* creates a worktree for the branch: an existing branch gets a worktree, and a branch that already has a worktree gets that worktree reopened.
4. **Title**: the workspace label. It defaults to the directory name (`name:branch` for worktrees, or when *In place* switches branch) until you edit it.

Submitting creates the workspace, focuses it, starts the harness in its first pane and closes the popup.
The last harness you used is remembered.

## Keys

| Key | Action |
| --- | --- |
| Tab / Shift-Tab, ↑↓ | move between fields |
| Enter | open the directory or branch picker / pick / next field; on Title it launches |
| typing (Directory, Branch) | open the picker with what you typed as the filter |
| ←→ (Harness) | choose harness |
| ←→ or `w` (Branch) | switch between In place and Worktree |
| Ctrl-S | launch |
| Ctrl-U | clear the current text |
| Esc | close the picker, or cancel |

## Install

```sh
herdr plugin install tedkulp/herdr-kickoff
```

The install hook (`herdr/install.sh`) downloads the prebuilt binary for your platform from the matching GitHub release and checks its SHA-256. Binaries exist for macOS (arm64, x86_64) and Linux (x86_64, arm64, static musl). If there isn't one for your platform, or you set `KICKOFF_BUILD_FROM_SOURCE=1`, it builds from source with cargo instead.

For a local checkout, `just link` builds into `bin/` and links the directory. `herdr plugin link` doesn't run the install hook, so run `just release` again after changing the code.

Then bind a key:

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

## Releasing

1. Write commit messages (or squash-merge PR titles) as [conventional commits](https://www.conventionalcommits.org/). `feat:` is new behaviour, `fix:` a bug fix and `perf:` a speedup; `feat!:` (or a `BREAKING CHANGE:` footer) marks a breaking change. `docs:`, `chore:`, `ci:`, `refactor:` and `test:` are for everything else.
2. Record every user-facing change under `## [Unreleased]` in [`CHANGELOG.md`](CHANGELOG.md), which follows [Keep a Changelog](https://keepachangelog.com). CI fails `feat`, `fix`, `perf` and breaking commits that don't touch it; `just changelog-check` runs the same check locally.
3. To release, run `gh workflow run release.yml` (or *Actions → release → Run workflow*). It picks the next version from the commits since the last release (breaking: major, or minor before 1.0; `feat`: minor; `fix`/`perf`: patch), or takes one with `-f version=X.Y.Z`. It turns `[Unreleased]` into the dated version section, bumps `Cargo.toml`, `Cargo.lock` and `herdr-plugin.toml`, pushes a `chore: release X.Y.Z` commit and `vX.Y.Z` tag to `main`, and publishes the GitHub release with that changelog section as its notes.
4. The `binaries` workflow then builds the binaries and attaches them to the release, with `.sha256` checksums. To rebuild them for an existing tag, run `gh workflow run binaries.yml -f tag=vX.Y.Z`.
