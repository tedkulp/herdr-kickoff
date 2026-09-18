#!/usr/bin/env python3
"""Prepare a release commit; the release workflow commits, tags and publishes it.

  release.py next                         print the version `prepare` would pick
  release.py prepare [VERSION] NOTES_FILE

Without VERSION, the next version comes from the conventional commits since the
tag of the current version. Stamps [Unreleased] in CHANGELOG.md as that version,
bumps Cargo.toml and herdr-plugin.toml, writes the version's changelog section to
NOTES_FILE and prints the version.
"""

import re
import sys
from datetime import date
from pathlib import Path

from changelog import CHANGELOG, USER_FACING, ChangelogError, git, release

VERSIONED_FILES = [Path("Cargo.toml"), Path("herdr-plugin.toml")]
TOP_LEVEL_VERSION = re.compile(r'^version = "(?P<version>[^"]+)"$', re.M)
BREAKING_SUBJECT = re.compile(r"^[a-z]+(?:\([^)]*\))?!: ")


def next_version(current: str, messages: list[str]) -> str:
    """The version after `current`, given the full messages of the commits since it."""
    major, minor, patch = (int(part) for part in current.split("."))
    subjects = [m.split("\n", 1)[0] for m in messages]
    if any(BREAKING_SUBJECT.match(s) for s in subjects) or any(
        "BREAKING CHANGE:" in m for m in messages
    ):
        # Before 1.0 the minor version is the breaking one, as Cargo reads it.
        return f"{major + 1}.0.0" if major else f"0.{minor + 1}.0"
    if any(re.match(r"^feat(?:\([^)]*\))?: ", s) for s in subjects):
        return f"{major}.{minor + 1}.0"
    if any(USER_FACING.match(s) for s in subjects):
        return f"{major}.{minor}.{patch + 1}"
    raise ChangelogError(
        f"no feat, fix, perf or breaking commits since v{current}; pass a version to release anyway"
    )


def bump_version(text: str, version: str) -> str:
    """`text` (a TOML manifest) with its top-level `version = "…"` set to `version`."""
    if not TOP_LEVEL_VERSION.search(text):
        raise ChangelogError('no top-level `version = "…"` line')
    return TOP_LEVEL_VERSION.sub(f'version = "{version}"', text, count=1)


def release_notes(text: str, version: str) -> str:
    """The body of the `## [version]` section of the changelog."""
    heading = re.search(rf"^## \[{re.escape(version)}\].*\n", text, re.M)
    if not heading:
        raise ChangelogError(f"no '## [{version}]' section in {CHANGELOG}")
    end = re.compile(r"^## |^\[", re.M).search(text, heading.end())
    return text[heading.end() : end.start() if end else len(text)].strip() + "\n"


def next_release() -> str:
    """The next version, from the commits since the tag of the current one."""
    current = TOP_LEVEL_VERSION.search(VERSIONED_FILES[0].read_text())["version"]
    log = git("log", "--format=%B%x00", f"v{current}..HEAD")
    return next_version(current, [m.strip() for m in log.split("\0") if m.strip()])


def prepare(version: str | None, notes_file: Path) -> str:
    version = version or next_release()
    if git("tag", "--list", f"v{version}").strip():
        raise ChangelogError(f"tag v{version} already exists")

    changelog = release(CHANGELOG.read_text(), version, date.today().isoformat())
    CHANGELOG.write_text(changelog)
    for path in VERSIONED_FILES:
        path.write_text(bump_version(path.read_text(), version))
    notes_file.write_text(release_notes(changelog, version))
    return version


def main(argv: list[str]) -> int:
    match argv:
        case ["next"]:
            run = next_release
        case ["prepare", notes_file]:
            run = lambda: prepare(None, Path(notes_file))  # noqa: E731
        case ["prepare", version, notes_file]:
            run = lambda: prepare(version, Path(notes_file))  # noqa: E731
        case _:
            print(__doc__, file=sys.stderr)
            return 2
    try:
        print(run())
    except ChangelogError as err:
        print(f"release: {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
