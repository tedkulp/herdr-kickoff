## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues on `tedkulp/herdr-kickoff` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com). Every `feat`, `fix`, `perf` or breaking commit adds its entry under `## [Unreleased]` in the same commit, written for users (what changed for them, not how), under `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed` or `Security`. CI fails such commits without a `CHANGELOG.md` edit; `just changelog-check` runs the same check locally. Version sections are written by the release workflow when a release PR is cut; edit only `[Unreleased]`.
