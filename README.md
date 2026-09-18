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

Releases are automated with [release-please](https://github.com/googleapis/release-please):

1. Write commit messages (or squash-merge PR titles) as [conventional commits](https://www.conventionalcommits.org/). Use `feat:` for new behaviour and `fix:` for bug fixes; these appear in the changelog. `docs:`, `chore:`, `ci:`, `refactor:` and `test:` are allowed but left out of it. `feat!:` marks a breaking change.
2. On each push to `main`, release-please opens or updates a release PR. That PR bumps the version in `Cargo.toml`, `Cargo.lock` and `herdr-plugin.toml` and adds the new commits to `CHANGELOG.md`.
3. Merging the release PR tags `vX.Y.Z` and publishes the GitHub release. The `release` workflow then builds the binaries and attaches them to it, with `.sha256` checksums. To rebuild them for an existing tag, run `gh workflow run release.yml -f tag=vX.Y.Z`.

The repo setting *Settings → Actions → General → Allow GitHub Actions to create and approve pull requests* must be on, or release-please can't open its PR.
