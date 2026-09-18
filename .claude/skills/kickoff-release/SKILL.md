---
name: kickoff-release
description: Use when releasing a new version of herdr-kickoff: checking main is green and the changelog's Unreleased entries are complete, running the release workflow, and verifying the published release and binaries.
---

# Releasing herdr-kickoff

A release is one run of `.github/workflows/release.yml` on `main`. The workflow does the mechanical part:

1. Picks the version (or takes the one you pass).
2. Moves `CHANGELOG.md`'s `## [Unreleased]` entries into `## [X.Y.Z] - <today>`.
3. Bumps `Cargo.toml`, `Cargo.lock` and `herdr-plugin.toml`.
4. Pushes a `chore: release X.Y.Z` commit and the `vX.Y.Z` tag to `main`.
5. Publishes the GitHub release, using the changelog section as its notes.
6. Calls `binaries.yml`, which attaches the four prebuilt binaries and their `.sha256` files.

**Your part:** make sure what gets released is right before the run, and confirm it landed after. Don't edit the changelog headings or the version fields yourself, and don't create the tag yourself.

Work through the steps in order. **A published tag can't be taken back.** `herdr plugin install` reads the version from `main` and downloads that tag's binaries. If you get a release wrong, fix it by releasing a new patch version.

## Step 1: Make sure local main is origin's main

The workflow releases `origin/main`, not your checkout.

```sh
git fetch --tags && git status -sb
```

Done when the first line is `## main...origin/main` with no `ahead` or `behind` and the tree is clean. Unpushed commits are not in the release. If you have any, push them and wait for CI.

## Step 2: Run the gate and confirm CI is green

```sh
just verify && just test-scripts
gh run list --branch main --workflow ci --limit 1
```

Done when both recipes pass locally **and** the latest `ci` run on `main` is `completed success` for the commit you are releasing. CI also runs the changelog check. A red changelog job means a user-facing commit has no entry.

## Step 3: Review the Unreleased entries

```sh
python3 scripts/release.py next
git log "v$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"..HEAD --format='%h %s'
```

Compare that commit list with `## [Unreleased]` in `CHANGELOG.md`. Done when both of these are true:

- Every `feat`, `fix`, `perf` and breaking commit is covered by an entry.
- Every entry is written for users: what changed for them, under `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed` or `Security`.

The CI check only proves `CHANGELOG.md` was touched, not that the entry is complete. So this review is the real check.

If entries need work, commit them as `docs: changelog for X.Y.Z`, push, and go back to Step 2.

## Step 4: Confirm the version with the user

`release.py next` picks the version from conventional commits:

- A breaking change (`type!:` or a `BREAKING CHANGE:` footer) bumps major, or minor while the version is `0.x`.
- `feat` bumps minor.
- `fix` or `perf` bumps patch.

Tell the user the version it picked and the one-line reason, and get their go-ahead. This is the last point where nothing is published. If the entries say otherwise (for example a `Removed` entry with no breaking commit), propose the version the changelog supports.

## Step 5: Run the workflow

```sh
gh workflow run release.yml                    # the version from Step 4
gh workflow run release.yml -f version=X.Y.Z   # or an explicit one
gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
```

Done when the run is `completed success`, including the `binaries` jobs. Expect about 5 to 10 minutes; the macOS and ARM builds are the slow ones.

**Timing gotcha:** the release commit lands on `main` a few minutes before the binaries are attached. A `herdr plugin install` in that window finds no binaries and falls back to building with cargo (or fails without cargo). Don't announce the release until Step 6 passes.

## Step 6: Verify what was published

```sh
gh release view vX.Y.Z
gh release view vX.Y.Z --json assets -q '.assets[].name' | sort
git pull
```

Done when all of these are true:

- The release notes match the changelog section.
- There are exactly eight assets: `herdr-kickoff-<target>` and `herdr-kickoff-<target>.sha256` for each of `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`.
- After `git pull`, `herdr-plugin.toml` says `version = "X.Y.Z"` and `CHANGELOG.md` has an empty `## [Unreleased]` above `## [X.Y.Z] - <date>`.

## When it goes wrong

The workflow pushes the commit and tag in one atomic push, then publishes the release. So a failure is either before anything is public or after the tag exists.

- **`nothing under '## [Unreleased]'`**: nothing was pushed. Add the entries (Step 3) and run again.
- **`no feat, fix, perf or breaking commits`**: nothing was pushed. If a release is still wanted (for example docs-only), pass `-f version=X.Y.Z`.
- **`tag vX.Y.Z already exists`**: that version is taken. Pick the next one.
- **Push rejected (`main` moved during the run)**: nothing was pushed. Run the workflow again.
- **Tag pushed, but `gh release create` failed**: create the release by hand from the changelog, then attach the binaries:
  ```sh
  git pull
  python3 -c 'import sys; sys.path.insert(0, "scripts"); from release import release_notes; print(release_notes(open("CHANGELOG.md").read(), "X.Y.Z"), end="")' > /tmp/notes.md
  gh release create vX.Y.Z --title vX.Y.Z --notes-file /tmp/notes.md
  gh workflow run binaries.yml -f tag=vX.Y.Z
  ```
- **A `binaries` job failed**: the release exists, but installs on that platform fall back to cargo. Rerun only the failed jobs with `gh run rerun <run-id> --failed`, or rebuild everything with `gh workflow run binaries.yml -f tag=vX.Y.Z`. `softprops/action-gh-release` replaces assets that already exist.
- **The release is wrong after it published**: leave the tag alone. Fix it on `main`, add the entry under `## [Unreleased]`, and release the next patch version.

## Quick reference

| Step | Command |
|---|---|
| In sync | `git fetch --tags && git status -sb` |
| Gate | `just verify && just test-scripts` |
| CI on main | `gh run list --branch main --workflow ci --limit 1` |
| Next version | `python3 scripts/release.py next` |
| Release | `gh workflow run release.yml [-f version=X.Y.Z]` |
| Watch | `gh run watch <run-id>` |
| Inspect | `gh release view vX.Y.Z` |
| Rebuild binaries | `gh workflow run binaries.yml -f tag=vX.Y.Z` |
