# Changelog

## [0.2.0](https://github.com/tedkulp/herdr-kickoff/compare/v0.1.0...v0.2.0) (2026-09-18)


### Features

* install prebuilt binaries instead of requiring cargo ([e7a26ae](https://github.com/tedkulp/herdr-kickoff/commit/e7a26aef26740985f5787bac981f08cbd94cf2ea))

## 0.1.0 (2026-09-18)

First release.

### Features

* Popup form (`kickoff.open`) that opens a directory as a new, focused herdr workspace and starts a coding-agent harness in it
* Directory picker over `zoxide query --list`, filtered like `zoxide query -i` (every word must appear in the path, zoxide's ranking kept), plus typed `/path` or `~/path`
* Harness choice of Opencode, Codex, Claude, pi, omp or Shell, with harnesses not on PATH greyed out and PATH re-checked live; the last harness used is remembered
* Harness list and default configurable through `config.toml` in the plugin config directory
* In place or Worktree launch mode: a new branch starts from the repo's default branch, an existing branch gets a worktree, and a branch that already has a worktree gets it reopened
* Title that follows the directory name (`name:branch` for worktrees) until edited
* Clear messages when zoxide is missing or fails, and herdr errors shown inline without closing the popup
