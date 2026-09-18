use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What the Launcher needs to know about the git repo behind a Target Directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    pub root: PathBuf,
    /// Checked-out branch, or `None` when HEAD is detached.
    pub current_branch: Option<String>,
    /// Ref new Worktree branches start from: the default branch, if one can be found.
    pub base: Option<String>,
    pub local_branches: HashSet<String>,
    /// Branch name to the path of the worktree that has it checked out.
    pub worktrees: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchStatus {
    Empty,
    Invalid,
    /// Will be created from `RepoInfo::base`.
    New,
    /// Exists locally with no worktree; a worktree will be created for it.
    Existing,
    /// Already checked out in a worktree, which will be reopened.
    CheckedOut(PathBuf),
}

impl RepoInfo {
    pub fn branch_status(&self, branch: &str) -> BranchStatus {
        if branch.is_empty() {
            BranchStatus::Empty
        } else if !is_valid_branch_name(branch) {
            BranchStatus::Invalid
        } else if let Some(path) = self.worktrees.get(branch) {
            BranchStatus::CheckedOut(path.clone())
        } else if self.local_branches.contains(branch) {
            BranchStatus::Existing
        } else {
            BranchStatus::New
        }
    }
}

/// `None` when `dir` is not inside a git work tree.
pub fn inspect(dir: &Path) -> Option<RepoInfo> {
    let root = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
    let current_branch = git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    let local_branches: HashSet<String> = git(
        &root,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .map(|out| out.lines().map(str::to_string).collect())
    .unwrap_or_default();
    let worktrees = git(&root, &["worktree", "list", "--porcelain"])
        .map(|out| parse_worktrees(&out))
        .unwrap_or_default();
    let base = default_branch(&root, &local_branches);
    Some(RepoInfo {
        root,
        current_branch,
        base,
        local_branches,
        worktrees,
    })
}

/// `origin/HEAD` when the remote advertises one, else a local `main` or `master`.
fn default_branch(root: &Path, local_branches: &HashSet<String>) -> Option<String> {
    git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .or_else(|| {
        ["main", "master"]
            .into_iter()
            .find(|b| local_branches.contains(*b))
            .map(str::to_string)
    })
}

fn parse_worktrees(porcelain: &str) -> HashMap<String, PathBuf> {
    let mut worktrees = HashMap::new();
    for block in porcelain.split("\n\n") {
        let mut path = None;
        let mut branch = None;
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                branch = Some(b.to_string());
            }
        }
        if let (Some(path), Some(branch)) = (path, branch) {
            worktrees.insert(branch, path);
        }
    }
    worktrees
}

/// The rules of `git check-ref-format --branch`, without spawning git on every keystroke.
pub fn is_valid_branch_name(name: &str) -> bool {
    const FORBIDDEN: &[char] = &[' ', '~', '^', ':', '?', '*', '[', '\\'];
    !name.is_empty()
        && name != "@"
        && !name.starts_with(['-', '/'])
        && !name.ends_with(['/', '.'])
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && !name.contains(FORBIDDEN)
        && !name.chars().any(char::is_control)
        && name
            .split('/')
            .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim_end_matches('\n');
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn non_repo_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(inspect(dir.path()), None);
    }

    #[test]
    fn inspects_branches_and_worktrees() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        run(&repo, &["init", "-q", "-b", "main"]);
        run(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        run(&repo, &["branch", "feat/idle"]);
        let wt = tmp.path().join("wt");
        run(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feat/busy",
                wt.to_str().unwrap(),
            ],
        );
        std::fs::create_dir(repo.join("sub")).unwrap();

        let info = inspect(&repo.join("sub")).unwrap();
        assert_eq!(
            info.root.canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
        assert_eq!(info.current_branch.as_deref(), Some("main"));
        assert_eq!(info.base.as_deref(), Some("main"));
        assert_eq!(info.branch_status("feat/idle"), BranchStatus::Existing);
        assert_eq!(info.branch_status("feat/new"), BranchStatus::New);
        assert_eq!(info.branch_status(""), BranchStatus::Empty);
        assert_eq!(info.branch_status("bad name"), BranchStatus::Invalid);
        match info.branch_status("feat/busy") {
            BranchStatus::CheckedOut(path) => {
                assert_eq!(path.canonicalize().unwrap(), wt.canonicalize().unwrap())
            }
            other => panic!("expected CheckedOut, got {other:?}"),
        }
    }

    #[test]
    fn branch_name_rules() {
        for ok in ["main", "feat/login", "fix-1.2", "user@host"] {
            assert!(is_valid_branch_name(ok), "{ok}");
        }
        for bad in [
            "", "@", "-x", "/x", "x/", "x.", "x.lock", "a..b", "a//b", "a@{b", "a b", "a~b", "a:b",
            "a/.b",
        ] {
            assert!(!is_valid_branch_name(bad), "{bad}");
        }
    }
}
