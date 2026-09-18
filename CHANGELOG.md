# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Branch picker for both Launch Modes: local branches, the current one first and the rest by most recent commit, filtered like the directory picker. Branches checked out in a worktree are tagged `⎇ worktree`, and typing a name that doesn't exist creates that branch.
- In Place mode can switch the directory to another branch before launching. A new branch starts from the default branch. If `git switch` fails, git's error is shown and nothing is launched. A branch checked out in another worktree needs Worktree mode.

### Changed

- The Title defaults to `name:branch` when In Place switches branch.

## [0.2.0] - 2026-09-18

### Changed

- Installing downloads a prebuilt binary for your platform, checked against its SHA-256, instead of requiring cargo. It falls back to building from source when there is no binary or `KICKOFF_BUILD_FROM_SOURCE=1` is set.

## [0.1.0] - 2026-09-18

First release.

### Added

- Popup form (`kickoff.open`) that opens a directory as a new, focused herdr workspace and starts a coding-agent harness in it
- Directory picker over `zoxide query --list`, filtered like `zoxide query -i` (every word must appear in the path, zoxide's ranking kept), plus typed `/path` or `~/path`
- Harness choice of Opencode, Codex, Claude, pi, omp or Shell, with harnesses not on PATH greyed out and PATH re-checked live; the last harness used is remembered
- Harness list and default configurable through `config.toml` in the plugin config directory
- In place or Worktree launch mode: a new branch starts from the repo's default branch, an existing branch gets a worktree, and a branch that already has a worktree gets it reopened
- Title that follows the directory name (`name:branch` for worktrees) until edited
- Clear messages when zoxide is missing or fails, and herdr errors shown inline without closing the popup

[Unreleased]: https://github.com/tedkulp/herdr-kickoff/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/tedkulp/herdr-kickoff/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tedkulp/herdr-kickoff/releases/tag/v0.1.0
