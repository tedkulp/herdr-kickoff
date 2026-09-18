//! Launcher form state and key handling, kept free of terminal and herdr IO.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Harness;
use crate::fuzzy;
use crate::git::{BranchStatus, RepoInfo};

/// The filesystem and git lookups the form needs while you type.
pub trait Probe {
    fn inspect(&self, dir: &Path) -> Option<RepoInfo>;
    fn is_dir(&self, path: &Path) -> bool;
    fn home(&self) -> Option<PathBuf>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Directory,
    Harness,
    Branch,
    Title,
}

impl Field {
    const ORDER: [Field; 4] = [
        Field::Directory,
        Field::Harness,
        Field::Branch,
        Field::Title,
    ];

    fn step(self, delta: isize) -> Field {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap() as isize;
        Self::ORDER[(i + delta).rem_euclid(Self::ORDER.len() as isize) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    InPlace,
    Worktree,
}

#[derive(Debug, Clone)]
pub struct HarnessChoice {
    pub harness: Harness,
    pub available: bool,
}

#[derive(Debug, Default)]
pub struct Picker {
    pub query: String,
    /// Entries shown, in order: an existing typed path first, then zoxide matches.
    pub entries: Vec<String>,
    pub selected: usize,
}

/// What submitting the form asks herdr to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub directory: PathBuf,
    pub harness: Harness,
    pub title: String,
    pub launch: Launch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    InPlace,
    /// Create a worktree; `base` is set only when the branch is new.
    CreateWorktree {
        repo_root: PathBuf,
        branch: String,
        base: Option<String>,
    },
    /// Reopen the worktree that already has the branch checked out.
    OpenWorktree {
        repo_root: PathBuf,
        branch: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Cancel,
    Submit(Request),
}

pub struct App<P: Probe> {
    probe: P,
    zoxide: Vec<String>,
    pub picker: Option<Picker>,
    pub field: Field,
    pub directory: Option<PathBuf>,
    pub repo: Option<RepoInfo>,
    pub harnesses: Vec<HarnessChoice>,
    pub harness: Option<usize>,
    pub mode: LaunchMode,
    pub branch: String,
    pub title: String,
    title_edited: bool,
    pub error: Option<String>,
}

impl<P: Probe> App<P> {
    /// `preferred` are harness names to preselect, in order of preference.
    pub fn new(
        probe: P,
        zoxide: Vec<String>,
        harnesses: Vec<HarnessChoice>,
        preferred: &[&str],
    ) -> Self {
        let mut app = Self {
            probe,
            zoxide,
            picker: None,
            field: Field::Directory,
            directory: None,
            repo: None,
            harnesses,
            harness: None,
            mode: LaunchMode::InPlace,
            branch: String::new(),
            title: String::new(),
            title_edited: false,
            error: None,
        };
        app.harness = preferred
            .iter()
            .find_map(|name| {
                app.harnesses
                    .iter()
                    .position(|h| h.available && h.harness.name.eq_ignore_ascii_case(name))
            })
            .or_else(|| app.harnesses.iter().position(|h| h.available));
        app.open_picker(String::new());
        app
    }

    pub fn directory_label(&self) -> Option<String> {
        Some(self.display_path(&self.directory.as_ref()?.to_string_lossy()))
    }

    /// `path` with the home directory shown as `~`.
    pub fn display_path(&self, path: &str) -> String {
        let home = self.probe.home();
        let home = home.as_ref().map(|h| h.to_string_lossy());
        match home.as_deref().and_then(|h| path.strip_prefix(h)) {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
            _ => path.to_string(),
        }
    }

    pub fn branch_status(&self) -> Option<BranchStatus> {
        match (self.mode, &self.repo) {
            (LaunchMode::Worktree, Some(repo)) => Some(repo.branch_status(&self.branch)),
            _ => None,
        }
    }

    /// Re-check which harnesses are installed, keeping the selection on an available one.
    pub fn refresh_availability(&mut self, is_available: impl Fn(&Harness) -> bool) {
        for choice in &mut self.harnesses {
            choice.available = is_available(&choice.harness);
        }
        if !self.harness.is_some_and(|i| self.harnesses[i].available) {
            self.harness = self.harnesses.iter().position(|h| h.available);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => return Outcome::Cancel,
            KeyCode::Char('s') if ctrl => return self.submit(),
            _ => {}
        }
        if self.picker.is_some() {
            return self.handle_picker_key(key, ctrl);
        }
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Tab | KeyCode::Down => self.field = self.field.step(1),
            KeyCode::BackTab | KeyCode::Up => self.field = self.field.step(-1),
            _ => match self.field {
                Field::Directory => self.handle_directory_key(key),
                Field::Harness => self.handle_harness_key(key),
                Field::Branch => self.handle_branch_key(key, ctrl),
                Field::Title => return self.handle_title_key(key, ctrl),
            },
        }
        Outcome::Continue
    }

    fn handle_picker_key(&mut self, key: KeyEvent, ctrl: bool) -> Outcome {
        let picker = self.picker.as_mut().expect("picker is open");
        match key.code {
            // Closing the picker before any directory is chosen leaves nothing to edit.
            KeyCode::Esc if self.directory.is_none() => return Outcome::Cancel,
            KeyCode::Esc => self.picker = None,
            KeyCode::Enter => {
                if let Some(entry) = picker.entries.get(picker.selected).cloned() {
                    self.select_directory(PathBuf::from(entry));
                }
            }
            KeyCode::Up | KeyCode::BackTab => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::Char('p') if ctrl => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Tab => {
                picker.selected = (picker.selected + 1).min(picker.entries.len().saturating_sub(1))
            }
            KeyCode::Char('n') if ctrl => {
                picker.selected = (picker.selected + 1).min(picker.entries.len().saturating_sub(1))
            }
            KeyCode::Char('u') if ctrl => {
                picker.query.clear();
                self.refilter();
            }
            KeyCode::Backspace => {
                picker.query.pop();
                self.refilter();
            }
            KeyCode::Char(c) if !ctrl => {
                picker.query.push(c);
                self.refilter();
            }
            _ => {}
        }
        Outcome::Continue
    }

    fn handle_directory_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.open_picker(String::new()),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_picker(c.to_string())
            }
            _ => {}
        }
    }

    fn handle_harness_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.cycle_harness(-1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => self.cycle_harness(1),
            KeyCode::Enter => self.field = Field::Branch,
            _ => {}
        }
    }

    fn handle_branch_key(&mut self, key: KeyEvent, ctrl: bool) {
        if self.repo.is_none() {
            if key.code == KeyCode::Enter {
                self.field = Field::Title;
            }
            return;
        }
        match (self.mode, key.code) {
            (_, KeyCode::Left | KeyCode::Right) | (LaunchMode::InPlace, KeyCode::Char('w')) => {
                self.toggle_mode()
            }
            (_, KeyCode::Enter) => self.field = Field::Title,
            (LaunchMode::Worktree, KeyCode::Char('u')) if ctrl => self.set_branch(String::new()),
            (LaunchMode::Worktree, KeyCode::Backspace) => {
                let mut branch = self.branch.clone();
                branch.pop();
                self.set_branch(branch);
            }
            (LaunchMode::Worktree, KeyCode::Char(c)) if !ctrl => {
                let branch = format!("{}{c}", self.branch);
                self.set_branch(branch);
            }
            _ => {}
        }
    }

    fn handle_title_key(&mut self, key: KeyEvent, ctrl: bool) -> Outcome {
        match key.code {
            KeyCode::Enter => return self.submit(),
            KeyCode::Char('u') if ctrl => {
                self.title.clear();
                self.title_edited = true;
            }
            KeyCode::Backspace => {
                self.title.pop();
                self.title_edited = true;
            }
            KeyCode::Char(c) if !ctrl => {
                self.title.push(c);
                self.title_edited = true;
            }
            _ => {}
        }
        Outcome::Continue
    }

    fn open_picker(&mut self, query: String) {
        self.picker = Some(Picker {
            query,
            ..Picker::default()
        });
        self.refilter();
    }

    fn refilter(&mut self) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        let mut entries = Vec::new();
        if let Some(path) = expand_typed_path(&picker.query, self.probe.home())
            && self.probe.is_dir(&path)
        {
            entries.push(path.to_string_lossy().into_owned());
        }
        for i in fuzzy::filter(&picker.query, &self.zoxide) {
            if !entries.contains(&self.zoxide[i]) {
                entries.push(self.zoxide[i].clone());
            }
        }
        picker.entries = entries;
        picker.selected = 0;
    }

    fn select_directory(&mut self, dir: PathBuf) {
        self.repo = self.probe.inspect(&dir);
        self.directory = Some(dir);
        self.picker = None;
        self.error = None;
        self.mode = LaunchMode::InPlace;
        self.branch = self.current_branch_label();
        self.field = Field::Harness;
        self.follow_title();
    }

    fn cycle_harness(&mut self, delta: isize) {
        let len = self.harnesses.len() as isize;
        let start = self.harness.map_or(-1, |i| i as isize);
        for step in 1..=len {
            let i = (start + delta * step).rem_euclid(len) as usize;
            if self.harnesses[i].available {
                self.harness = Some(i);
                return;
            }
        }
    }

    fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            LaunchMode::InPlace => LaunchMode::Worktree,
            LaunchMode::Worktree => LaunchMode::InPlace,
        };
        self.branch = match self.mode {
            LaunchMode::InPlace => self.current_branch_label(),
            LaunchMode::Worktree => String::new(),
        };
        self.follow_title();
    }

    fn set_branch(&mut self, branch: String) {
        self.branch = branch;
        self.follow_title();
    }

    fn current_branch_label(&self) -> String {
        match &self.repo {
            Some(repo) => repo
                .current_branch
                .clone()
                .unwrap_or_else(|| "(detached HEAD)".to_string()),
            None => String::new(),
        }
    }

    /// The Title follows the Target Directory (and Worktree branch) until edited.
    fn follow_title(&mut self) {
        if !self.title_edited {
            self.title = self.default_title();
        }
    }

    fn default_title(&self) -> String {
        let Some(dir) = &self.directory else {
            return String::new();
        };
        let base = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir.to_string_lossy().into_owned());
        match self.mode {
            LaunchMode::Worktree if !self.branch.is_empty() => format!("{base}:{}", self.branch),
            _ => base,
        }
    }

    fn submit(&mut self) -> Outcome {
        match self.build_request() {
            Ok(request) => Outcome::Submit(request),
            Err((message, field)) => {
                self.error = Some(message);
                self.field = field;
                Outcome::Continue
            }
        }
    }

    fn build_request(&self) -> Result<Request, (String, Field)> {
        let directory = self
            .directory
            .clone()
            .ok_or(("Pick a directory first".to_string(), Field::Directory))?;
        let harness = self
            .harness
            .map(|i| &self.harnesses[i])
            .filter(|h| h.available)
            .ok_or(("No available harness selected".to_string(), Field::Harness))?
            .harness
            .clone();
        let launch = match (self.mode, &self.repo) {
            (LaunchMode::Worktree, Some(repo)) => {
                let branch = self.branch.clone();
                let repo_root = repo.root.clone();
                match repo.branch_status(&branch) {
                    BranchStatus::Empty => {
                        return Err(("Enter a branch for the worktree".into(), Field::Branch));
                    }
                    BranchStatus::Invalid => {
                        return Err((
                            format!("'{branch}' is not a valid branch name"),
                            Field::Branch,
                        ));
                    }
                    BranchStatus::New => Launch::CreateWorktree {
                        repo_root,
                        branch,
                        base: repo.base.clone(),
                    },
                    BranchStatus::Existing => Launch::CreateWorktree {
                        repo_root,
                        branch,
                        base: None,
                    },
                    BranchStatus::CheckedOut(_) => Launch::OpenWorktree { repo_root, branch },
                }
            }
            _ => Launch::InPlace,
        };
        let title = match self.title.trim() {
            "" => self.default_title(),
            title => title.to_string(),
        };
        Ok(Request {
            directory,
            harness,
            title,
            launch,
        })
    }
}

/// A picker query that looks like a path (`/…` or `~…`), expanded.
fn expand_typed_path(query: &str, home: Option<PathBuf>) -> Option<PathBuf> {
    let query = query.trim();
    let path = if query == "~" {
        home?
    } else if let Some(rest) = query.strip_prefix("~/") {
        home?.join(rest)
    } else if query.starts_with('/') {
        PathBuf::from(query)
    } else {
        return None;
    };
    // Normalise a trailing slash so it matches zoxide's spelling.
    let text = path.to_string_lossy();
    match text.trim_end_matches('/') {
        "" => Some(PathBuf::from("/")),
        trimmed => Some(PathBuf::from(trimmed)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    struct FakeProbe;

    impl Probe for FakeProbe {
        fn inspect(&self, dir: &Path) -> Option<RepoInfo> {
            (dir.ends_with("repo")).then(|| RepoInfo {
                root: dir.to_path_buf(),
                current_branch: Some("main".into()),
                base: Some("origin/main".into()),
                local_branches: HashSet::from(["main".into(), "feat/old".into(), "feat/wt".into()]),
                worktrees: HashMap::from([
                    ("main".into(), dir.to_path_buf()),
                    ("feat/wt".into(), PathBuf::from("/wt/feat-wt")),
                ]),
            })
        }
        fn is_dir(&self, path: &Path) -> bool {
            path == Path::new("/typed/dir")
        }
        fn home(&self) -> Option<PathBuf> {
            Some(PathBuf::from("/home/me"))
        }
    }

    fn harness(name: &str, available: bool) -> HarnessChoice {
        HarnessChoice {
            harness: Harness {
                name: name.into(),
                command: name.to_lowercase(),
            },
            available,
        }
    }

    fn app() -> App<FakeProbe> {
        App::new(
            FakeProbe,
            vec!["/home/me/src/repo".into(), "/home/me/notes".into()],
            vec![
                harness("Opencode", true),
                harness("pi", false),
                harness("Claude", true),
            ],
            &["pi", "Claude"],
        )
    }

    fn press(app: &mut App<FakeProbe>, code: KeyCode) -> Outcome {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(app: &mut App<FakeProbe>, c: char) -> Outcome {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    fn type_text(app: &mut App<FakeProbe>, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    fn pick(app: &mut App<FakeProbe>, query: &str) {
        type_text(app, query);
        press(app, KeyCode::Enter);
    }

    #[test]
    fn opens_with_picker_and_skips_unavailable_preferred_harness() {
        let app = app();
        assert!(app.picker.is_some());
        assert_eq!(
            app.harness,
            Some(2),
            "pi is unavailable so Claude is preselected"
        );
    }

    #[test]
    fn escape_in_initial_picker_cancels() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Esc), Outcome::Cancel);
    }

    #[test]
    fn picking_a_repo_fills_branch_and_title() {
        let mut app = app();
        pick(&mut app, "repo");
        assert!(app.picker.is_none());
        assert_eq!(app.directory, Some(PathBuf::from("/home/me/src/repo")));
        assert_eq!(app.field, Field::Harness);
        assert_eq!(app.mode, LaunchMode::InPlace);
        assert_eq!(app.branch, "main");
        assert_eq!(app.title, "repo");
        assert_eq!(app.directory_label().as_deref(), Some("~/src/repo"));
    }

    #[test]
    fn typed_path_is_offered_first() {
        let mut app = app();
        type_text(&mut app, "/typed/dir/");
        assert_eq!(app.picker.as_ref().unwrap().entries, ["/typed/dir"]);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.directory, Some(PathBuf::from("/typed/dir")));
        assert!(app.repo.is_none());
        assert_eq!(app.title, "dir");
    }

    #[test]
    fn escape_closes_picker_once_a_directory_is_chosen() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Directory;
        press(&mut app, KeyCode::Enter);
        assert!(app.picker.is_some());
        assert_eq!(press(&mut app, KeyCode::Esc), Outcome::Continue);
        assert!(app.picker.is_none());
        assert_eq!(press(&mut app, KeyCode::Esc), Outcome::Cancel);
    }

    #[test]
    fn harness_cycling_skips_unavailable() {
        let mut app = app();
        pick(&mut app, "repo");
        press(&mut app, KeyCode::Right);
        assert_eq!(app.harness, Some(0));
        press(&mut app, KeyCode::Right);
        assert_eq!(app.harness, Some(2));
        press(&mut app, KeyCode::Left);
        assert_eq!(app.harness, Some(0));
    }

    #[test]
    fn refresh_moves_off_a_harness_that_disappears() {
        let mut app = app();
        app.refresh_availability(|h| h.name != "Claude");
        assert_eq!(app.harness, Some(0));
        assert!(app.harnesses[1].available);
    }

    #[test]
    fn in_place_branch_is_read_only() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.branch, "main");
    }

    #[test]
    fn worktree_mode_edits_branch_and_title_follows() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Char('w'));
        assert_eq!(app.mode, LaunchMode::Worktree);
        assert_eq!(app.branch, "");
        type_text(&mut app, "feat/new");
        assert_eq!(app.title, "repo:feat/new");
        assert_eq!(app.branch_status(), Some(BranchStatus::New));
        press(&mut app, KeyCode::Left);
        assert_eq!(app.mode, LaunchMode::InPlace);
        assert_eq!(app.branch, "main");
        assert_eq!(app.title, "repo");
    }

    #[test]
    fn edited_title_stops_following() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Title;
        type_text(&mut app, "!");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        type_text(&mut app, "x");
        assert_eq!(app.title, "repo!");
    }

    #[test]
    fn non_repo_forces_in_place() {
        let mut app = app();
        pick(&mut app, "notes");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        assert_eq!(app.mode, LaunchMode::InPlace);
        assert_eq!(app.branch, "");
    }

    #[test]
    fn submit_in_place() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Title;
        let Outcome::Submit(request) = press(&mut app, KeyCode::Enter) else {
            panic!("expected submit");
        };
        assert_eq!(request.launch, Launch::InPlace);
        assert_eq!(request.harness.name, "Claude");
        assert_eq!(request.title, "repo");
    }

    fn submit_worktree(branch: &str) -> Launch {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        type_text(&mut app, branch);
        match ctrl(&mut app, 's') {
            Outcome::Submit(request) => request.launch,
            other => panic!("expected submit, got {other:?} ({:?})", app.error),
        }
    }

    #[test]
    fn submit_worktree_variants() {
        let root = PathBuf::from("/home/me/src/repo");
        assert_eq!(
            submit_worktree("feat/new"),
            Launch::CreateWorktree {
                repo_root: root.clone(),
                branch: "feat/new".into(),
                base: Some("origin/main".into())
            }
        );
        assert_eq!(
            submit_worktree("feat/old"),
            Launch::CreateWorktree {
                repo_root: root.clone(),
                branch: "feat/old".into(),
                base: None
            }
        );
        assert_eq!(
            submit_worktree("feat/wt"),
            Launch::OpenWorktree {
                repo_root: root,
                branch: "feat/wt".into()
            }
        );
    }

    #[test]
    fn invalid_submit_reports_error_and_focuses_field() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        app.field = Field::Title;
        assert_eq!(ctrl(&mut app, 's'), Outcome::Continue);
        assert_eq!(app.field, Field::Branch);
        assert!(app.error.is_some());
    }

    #[test]
    fn submit_without_directory_fails() {
        let mut app = app();
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(ctrl(&mut app, 's'), Outcome::Continue);
        assert_eq!(app.error.as_deref(), Some("Pick a directory first"));
    }
}
