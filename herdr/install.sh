#!/usr/bin/env bash
# herdr plugin build hook: install the prebuilt binary for this platform from
# the GitHub release matching herdr-plugin.toml's version, checking it against
# the published SHA-256. Falls back to building from source with cargo when no
# prebuilt binary exists. Set KICKOFF_BUILD_FROM_SOURCE=1 to skip the download.
set -euo pipefail
cd "$(dirname "$0")/.."

repo="tedkulp/herdr-kickoff"
bin="herdr-kickoff"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' herdr-plugin.toml | head -1)
mkdir -p bin

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  *) target="" ;;
esac

sha256() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

download() {
  local url="https://github.com/$repo/releases/download/v$version/$bin-$target"
  if ! curl -fsL "$url" -o "$tmp/$bin" || ! curl -fsL "$url.sha256" -o "$tmp/$bin.sha256"; then
    echo "$bin: no prebuilt binary at $url" >&2
    return 1
  fi
  local expected actual
  expected=$(cut -d' ' -f1 <"$tmp/$bin.sha256")
  actual=$(sha256 "$tmp/$bin")
  if [ "$expected" != "$actual" ]; then
    echo "$bin: checksum mismatch for $url (expected $expected, got $actual)" >&2
    exit 1
  fi
  chmod +x "$tmp/$bin" && mv "$tmp/$bin" "bin/$bin" || return 1
  echo "$bin: installed prebuilt v$version for $target" >&2
}

if [ -z "${KICKOFF_BUILD_FROM_SOURCE:-}" ] && [ -n "$target" ] && download; then
  exit 0
fi

if ! command -v cargo >/dev/null; then
  echo "$bin: no prebuilt v$version binary for ${target:-$(uname -s)-$(uname -m)} and cargo is not installed" >&2
  exit 1
fi
echo "$bin: building from source" >&2
cargo build --release --locked
cp "target/release/$bin" "bin/$bin"
