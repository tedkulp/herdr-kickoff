import subprocess
import tempfile
import unittest
from pathlib import Path

from changelog import ChangelogError, needs_entry, release

URL = "https://github.com/o/r"

BEFORE = f"""# Changelog

Intro.

## [Unreleased]

### Added

- New thing

## [0.2.0] - 2026-01-02

### Changed

- Old thing

[Unreleased]: {URL}/compare/v0.2.0...HEAD
[0.2.0]: {URL}/compare/v0.1.0...v0.2.0
"""

AFTER = f"""# Changelog

Intro.

## [Unreleased]

## [0.3.0] - 2026-03-04

### Added

- New thing

## [0.2.0] - 2026-01-02

### Changed

- Old thing

[Unreleased]: {URL}/compare/v0.3.0...HEAD
[0.3.0]: {URL}/compare/v0.2.0...v0.3.0
[0.2.0]: {URL}/compare/v0.1.0...v0.2.0
"""


class ReleaseTest(unittest.TestCase):
    def test_moves_unreleased_into_a_dated_version_with_links(self):
        self.assertEqual(release(BEFORE, "0.3.0", "2026-03-04"), AFTER)

    def test_is_idempotent(self):
        self.assertEqual(release(AFTER, "0.3.0", "2026-03-05"), AFTER)

    def test_first_release_links_to_its_tag(self):
        text = f"# Changelog\n\n## [Unreleased]\n\n- x\n\n[Unreleased]: {URL}/commits/HEAD\n"
        out = release(text, "0.1.0", "2026-01-01")
        self.assertIn("## [0.1.0] - 2026-01-01\n\n- x\n", out)
        self.assertIn(f"[Unreleased]: {URL}/compare/v0.1.0...HEAD\n", out)
        self.assertIn(f"[0.1.0]: {URL}/releases/tag/v0.1.0\n", out)

    def test_empty_unreleased_is_an_error(self):
        empty = BEFORE.replace("### Added\n\n- New thing\n\n", "")
        with self.assertRaisesRegex(ChangelogError, "nothing under"):
            release(empty, "0.3.0", "2026-03-04")

    def test_missing_unreleased_is_an_error(self):
        with self.assertRaisesRegex(ChangelogError, r"\[Unreleased\]"):
            release("# Changelog\n", "0.3.0", "2026-03-04")


class NeedsEntryTest(unittest.TestCase):
    def test_user_facing_commit_types_need_an_entry(self):
        for subject in [
            "feat: x",
            "fix(ui): x",
            "perf: x",
            "refactor!: x",
            "chore(deps)!: x",
        ]:
            self.assertTrue(needs_entry([subject]), subject)

    def test_other_commit_types_do_not(self):
        for subject in [
            "docs: x",
            "test: x",
            "ci: x",
            "chore(main): release 0.3.0",
            "Merge pull request #1 from x",
            "feature: not conventional",
        ]:
            self.assertFalse(needs_entry([subject]), subject)


class CheckCommandTest(unittest.TestCase):
    def git(self, *args):
        subprocess.run(
            ["git", "-C", self.repo, *args],
            check=True,
            capture_output=True,
            env={
                "GIT_CONFIG_GLOBAL": "/dev/null",
                "GIT_AUTHOR_NAME": "t",
                "GIT_AUTHOR_EMAIL": "t@example.com",
                "GIT_COMMITTER_NAME": "t",
                "GIT_COMMITTER_EMAIL": "t@example.com",
                "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin",
            },
        )

    def rev(self):
        return subprocess.run(
            ["git", "-C", self.repo, "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def check(self, base, head):
        script = Path(__file__).with_name("changelog.py")
        return subprocess.run(
            ["python3", str(script), "check", base, head],
            cwd=self.repo,
            capture_output=True,
            text=True,
        )

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = self.tmp.name
        self.git("init", "-q", "-b", "main")
        Path(self.repo, "CHANGELOG.md").write_text(BEFORE)
        self.git("add", ".")
        self.git("commit", "-q", "-m", "init")

    def tearDown(self):
        self.tmp.cleanup()

    def test_feat_without_changelog_fails_and_with_it_passes(self):
        base = self.rev()
        Path(self.repo, "a").write_text("a")
        self.git("add", "a")
        self.git("commit", "-q", "-m", "feat: a")
        result = self.check(base, self.rev())
        self.assertEqual(result.returncode, 1)
        self.assertIn("feat: a", result.stderr)

        Path(self.repo, "CHANGELOG.md").write_text(BEFORE.replace("- New thing", "- New thing\n- A"))
        self.git("commit", "-q", "-am", "docs: changelog")
        self.assertEqual(self.check(base, self.rev()).returncode, 0)

    def test_docs_only_passes(self):
        base = self.rev()
        Path(self.repo, "a").write_text("a")
        self.git("add", "a")
        self.git("commit", "-q", "-m", "docs: a")
        self.assertEqual(self.check(base, self.rev()).returncode, 0)


if __name__ == "__main__":
    unittest.main()
