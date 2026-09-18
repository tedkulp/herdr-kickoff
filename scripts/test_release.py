import unittest

from changelog import ChangelogError
from release import bump_version, next_version, release_notes


class NextVersionTest(unittest.TestCase):
    def test_bump_follows_the_biggest_change(self):
        cases = [
            ("1.2.3", ["fix: a", "docs: b"], "1.2.4"),
            ("1.2.3", ["perf: a"], "1.2.4"),
            ("1.2.3", ["fix: a", "feat(ui): b"], "1.3.0"),
            ("1.2.3", ["feat: a", "refactor!: b"], "2.0.0"),
            ("1.2.3", ["feat: a\n\nBREAKING CHANGE: config renamed"], "2.0.0"),
            # Before 1.0 a breaking change bumps the minor version, as Cargo reads it.
            ("0.2.0", ["feat!: a"], "0.3.0"),
            ("0.2.0", ["feat: a"], "0.3.0"),
        ]
        for current, messages, expected in cases:
            self.assertEqual(next_version(current, messages), expected, (current, messages))

    def test_nothing_user_facing_is_an_error(self):
        with self.assertRaisesRegex(ChangelogError, "no feat, fix, perf or breaking"):
            next_version("1.0.0", ["docs: a", "chore: b"])


class BumpVersionTest(unittest.TestCase):
    def test_replaces_only_the_first_top_level_version(self):
        cargo = (
            '[package]\nname = "x"\nversion = "0.2.0"\n\n'
            '[dependencies]\nserde = { version = "1.0" }\nfoo = "1"\n'
        )
        self.assertEqual(
            bump_version(cargo, "0.3.0"),
            cargo.replace('version = "0.2.0"', 'version = "0.3.0"'),
        )

    def test_missing_version_is_an_error(self):
        with self.assertRaises(ChangelogError):
            bump_version('name = "x"\n', "0.3.0")


class ReleaseNotesTest(unittest.TestCase):
    TEXT = """# Changelog

## [Unreleased]

## [0.3.0] - 2026-03-04

### Added

- New thing

## [0.2.0] - 2026-01-02

- Old thing

[Unreleased]: https://x/compare/v0.3.0...HEAD
"""

    def test_extracts_the_version_section_body(self):
        self.assertEqual(release_notes(self.TEXT, "0.3.0"), "### Added\n\n- New thing\n")

    def test_last_section_stops_at_the_links(self):
        self.assertEqual(release_notes(self.TEXT, "0.2.0"), "- Old thing\n")

    def test_unknown_version_is_an_error(self):
        with self.assertRaises(ChangelogError):
            release_notes(self.TEXT, "9.9.9")


if __name__ == "__main__":
    unittest.main()
