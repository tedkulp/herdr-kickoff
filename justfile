set shell := ["bash", "-cu"]

default:
    @just --list

# Format Rust sources.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all -- --check

# Typecheck every target.
check:
    cargo check --all-targets

# Lint every target, treating warnings as errors.
lint:
    cargo clippy --all-targets -- -D warnings

# Run the automated test suite.
test:
    cargo test --all-targets

# Run one test by name, for example: just test-one worktree
test-one name:
    cargo test {{name}}

# Run formatting checks, lints, and the full test suite.
verify: fmt-check lint test

# Build a debug binary.
build:
    cargo build --locked

# Build the release plugin binary.
release:
    cargo build --release --locked

# Link the release plugin into Herdr for development.
link: release
    herdr plugin link .

# Open the launcher popup.
open:
    herdr plugin action invoke kickoff.open

# Open the launcher popup with zoxide removed from PATH.
open-no-zoxide:
    #!/usr/bin/env bash
    set -euo pipefail
    # Drops every PATH directory holding zoxide, so its neighbours vanish too.
    path=""
    IFS=: read -ra dirs <<< "$PATH"
    for dir in "${dirs[@]}"; do
        [[ -x "$dir/zoxide" ]] && { echo "dropping $dir" >&2; continue; }
        path="${path:+$path:}$dir"
    done
    herdr plugin pane open --plugin kickoff --entrypoint launcher \
        --env "PATH=$path" --focus >/dev/null

# List this plugin's command logs.
logs:
    herdr plugin log list --plugin kickoff
