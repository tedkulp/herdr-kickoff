#!/usr/bin/env python3
"""Keep CHANGELOG.md in Keep a Changelog form (https://keepachangelog.com).

  changelog.py release VERSION [DATE]   move [Unreleased] into a dated VERSION section
  changelog.py check BASE HEAD          fail if BASE..HEAD has user-facing commits
                                        (feat, fix, perf or breaking) but no CHANGELOG.md edit
"""

import re
import subprocess
import sys
from datetime import date
from pathlib import Path

CHANGELOG = Path("CHANGELOG.md")
UNRELEASED = "## [Unreleased]"
# Conventional commit types that change what users see.
USER_FACING = re.compile(r"^(?:(?:feat|fix|perf)(?:\([^)]*\))?!?|[a-z]+(?:\([^)]*\))?!): ")
UNRELEASED_LINK = re.compile(r"^\[Unreleased\]: (?P<base>\S+?)/(?:compare/v(?P<prev>\S+)\.\.\.HEAD|commits/HEAD)$", re.M)


class ChangelogError(Exception):
    pass


def release(text: str, version: str, day: str) -> str:
    """`text` with the [Unreleased] entries moved under `## [version] - day`."""
    if f"## [{version}]" in text:
        return text
    start = text.find(UNRELEASED + "\n")
    if start == -1:
        raise ChangelogError(f"no '{UNRELEASED}' heading in {CHANGELOG}")
    body_start = start + len(UNRELEASED) + 1
    next_heading = re.compile(r"^## |^\[", re.M).search(text, body_start)
    body_end = next_heading.start() if next_heading else len(text)
    if not text[body_start:body_end].strip():
        raise ChangelogError(f"nothing under '{UNRELEASED}' to release as {version}")

    link = UNRELEASED_LINK.search(text)
    if not link:
        raise ChangelogError("no '[Unreleased]: <repo url>/compare/v<prev>...HEAD' link")
    base, prev = link["base"], link["prev"]
    version_link = (
        f"[{version}]: {base}/compare/v{prev}...v{version}"
        if prev
        else f"[{version}]: {base}/releases/tag/v{version}"
    )
    links = f"[Unreleased]: {base}/compare/v{version}...HEAD\n{version_link}"

    text = text[:link.start()] + links + text[link.end():]
    return text[:body_start] + f"\n## [{version}] - {day}\n" + text[body_start:]


def needs_entry(subjects: list[str]) -> bool:
    return any(USER_FACING.match(s) for s in subjects)


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def check(base: str, head: str) -> int:
    subjects = git("log", "--format=%s", f"{base}..{head}").splitlines()
    user_facing = [s for s in subjects if USER_FACING.match(s)]
    if not user_facing or git("diff", "--name-only", base, head, "--", str(CHANGELOG)).strip():
        return 0
    print(
        f"{CHANGELOG} was not updated, but these commits change user-facing behaviour:",
        *(f"  {s}" for s in user_facing),
        f"Add an entry under '{UNRELEASED}' (see https://keepachangelog.com).",
        sep="\n",
        file=sys.stderr,
    )
    return 1


def main(argv: list[str]) -> int:
    match argv:
        case ["release", version, *rest] if len(rest) <= 1:
            day = rest[0] if rest else date.today().isoformat()
            try:
                CHANGELOG.write_text(release(CHANGELOG.read_text(), version, day))
            except ChangelogError as err:
                print(f"changelog: {err}", file=sys.stderr)
                return 1
            return 0
        case ["check", base, head]:
            return check(base, head)
        case _:
            print(__doc__, file=sys.stderr)
            return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
