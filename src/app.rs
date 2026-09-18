//! Launcher form state and key handling, kept free of terminal and herdr IO.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Harness;
use crate::filter;
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PickerKind {
    #[default]
    Directory,
    Branch,
}

#[derive(Debug, Default)]
pub struct Picker {
    pub kind: PickerKind,
    pub query: String,
    /// Entries shown, in order. Directories: an existing typed path first, then
    /// zoxide matches. Branches: matching local branches, then the typed name
    /// if it would be a new branch.
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
    /// Open the Target Directory itself, switching its branch first if `switch` is set.
    InPlace { switch: Option<BranchSwitch> },
    /// Create a worktree; `base` is set only when the branch is new.
    CreateWorktree {
        repo_root: PathBuf,
        branch: String,
        base: Option<String>,
    },
    /// Reopen the worktree that already has the branch checked out.
    OpenWorktree { repo_root: PathBuf, branch: String },
}

/// Check out another branch in the Target Directory before launching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSwitch {
    pub repo_root: PathBuf,
    pub branch: String,
    /// The branch is new: create it from `base` (HEAD when `None`).
    pub create: bool,
    pub base: Option<String>,
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
        app.open_picker(PickerKind::Directory, String::new());
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

    /// In Place with a branch other than the one already checked out.
    pub fn switches_branch(&self) -> bool {
        self.mode == LaunchMode::InPlace
            && self.repo.is_some()
            && self.branch != self.current_branch_label()
    }

    /// An In Place `git switch` failed: nothing was launched, so show git's
    /// error on the Branch field for the user to fix or cancel.
    pub fn branch_switch_failed(&mut self, message: String) {
        self.error = Some(message);
        self.field = Field::Branch;
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
                    match picker.kind {
                        PickerKind::Directory => self.select_directory(PathBuf::from(entry)),
                        PickerKind::Branch => self.select_branch(entry),
                    }
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
            KeyCode::Enter => self.open_picker(PickerKind::Directory, String::new()),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_picker(PickerKind::Directory, c.to_string())
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
            (_, KeyCode::Enter) => self.open_picker(PickerKind::Branch, String::new()),
            (_, KeyCode::Char(c)) if !ctrl => self.open_picker(PickerKind::Branch, c.to_string()),
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

    fn open_picker(&mut self, kind: PickerKind, query: String) {
        self.picker = Some(Picker {
            kind,
            query,
            ..Picker::default()
        });
        self.refilter();
    }

    fn refilter(&mut self) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        let (typed, candidates, typed_first) = match picker.kind {
            PickerKind::Directory => (
                expand_typed_path(&picker.query, self.probe.home())
                    .filter(|path| self.probe.is_dir(path))
                    .map(|path| path.to_string_lossy().into_owned()),
                &self.zoxide,
                true,
            ),
            PickerKind::Branch => {
                let Some(repo) = &self.repo else {
                    return;
                };
                let query = picker.query.trim();
                (
                    (repo.branch_status(query) == BranchStatus::New).then(|| query.to_string()),
                    &repo.branches,
                    false,
                )
            }
        };
        let mut entries: Vec<String> = Vec::new();
        for i in filter::filter(&picker.query, candidates) {
            if typed.as_ref() != Some(&candidates[i]) {
                entries.push(candidates[i].clone());
            }
        }
        if let Some(typed) = typed {
            let at = if typed_first { 0 } else { entries.len() };
            entries.insert(at, typed);
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

    fn select_branch(&mut self, branch: String) {
        self.picker = None;
        self.error = None;
        self.field = Field::Title;
        self.set_branch(branch);
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
                .unwrap_or_else(|| "(detached)".to_string()),
            None => String::new(),
        }
    }

    /// The Title follows the Target Directory (and a chosen branch) until edited.
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
        let show_branch = match self.mode {
            LaunchMode::Worktree => !self.branch.is_empty(),
            LaunchMode::InPlace => self.switches_branch(),
        };
        if show_branch {
            format!("{base}:{}", self.branch)
        } else {
            base
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
            (LaunchMode::InPlace, Some(repo)) if self.switches_branch() => {
                let branch = self.branch.clone();
                let (create, base) = match repo.branch_status(&branch) {
                    BranchStatus::Empty => {
                        return Err(("Pick a branch".into(), Field::Branch));
                    }
                    BranchStatus::Invalid => {
                        return Err((
                            format!("'{branch}' is not a valid branch name"),
                            Field::Branch,
                        ));
                    }
                    BranchStatus::CheckedOut(path) => {
                        return Err((
                            format!(
                                "'{branch}' is checked out at {}, use Worktree mode",
                                path.display()
                            ),
                            Field::Branch,
                        ));
                    }
                    BranchStatus::New => (true, repo.base.clone()),
                    BranchStatus::Existing => (false, None),
                };
                Launch::InPlace {
                    switch: Some(BranchSwitch {
                        repo_root: repo.root.clone(),
                        branch,
                        create,
                        base,
                    }),
                }
            }
            _ => Launch::InPlace { switch: None },
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
    use std::collections::HashMap;

    struct FakeProbe;

    impl Probe for FakeProbe {
        fn inspect(&self, dir: &Path) -> Option<RepoInfo> {
            let current_branch = match dir.file_name()?.to_str()? {
                "repo" => Some("main".into()),
                "detached" => None,
                _ => return None,
            };
            Some(RepoInfo {
                root: dir.to_path_buf(),
                current_branch,
                base: Some("origin/main".into()),
                branches: vec!["main".into(), "feat/wt".into(), "feat/old".into()],
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
            vec![
                "/home/me/src/repo".into(),
                "/home/me/notes".into(),
                "/home/me/src/detached".into(),
            ],
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

    fn branch_status(app: &App<FakeProbe>) -> BranchStatus {
        app.repo.as_ref().unwrap().branch_status(&app.branch)
    }

    fn branch_picker(app: &App<FakeProbe>) -> &Picker {
        let picker = app.picker.as_ref().expect("picker is open");
        assert_eq!(picker.kind, PickerKind::Branch);
        picker
    }

    #[test]
    fn enter_on_branch_opens_picker_current_first() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Enter);
        assert_eq!(branch_picker(&app).entries, ["main", "feat/wt", "feat/old"]);
    }

    #[test]
    fn typing_on_branch_opens_filtered_picker() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        type_text(&mut app, "feat");
        let picker = branch_picker(&app);
        assert_eq!(picker.query, "feat");
        assert_eq!(picker.entries, ["feat/wt", "feat/old", "feat"]);
        type_text(&mut app, " old");
        assert_eq!(branch_picker(&app).entries, ["feat/old"]);
    }

    #[test]
    fn picking_a_branch_in_place_updates_branch_and_title() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        pick(&mut app, "old");
        assert!(app.picker.is_none());
        assert_eq!(app.mode, LaunchMode::InPlace);
        assert_eq!(app.branch, "feat/old");
        assert_eq!(app.title, "repo:feat/old");
        assert_eq!(app.field, Field::Title);
    }

    #[test]
    fn typed_new_branch_is_offered_last_unless_invalid() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        type_text(&mut app, "feat/");
        // "feat/" is not a valid branch name, so it isn't offered.
        assert_eq!(branch_picker(&app).entries, ["feat/wt", "feat/old"]);
        type_text(&mut app, "w");
        // "feat/w" is new but matches feat/wt, which is picked by default.
        assert_eq!(branch_picker(&app).entries, ["feat/wt", "feat/w"]);
        type_text(&mut app, "x");
        assert_eq!(branch_picker(&app).entries, ["feat/wx"]);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.branch, "feat/wx");
        assert_eq!(branch_status(&app), BranchStatus::New);
    }

    #[test]
    fn escape_closes_branch_picker_and_keeps_branch() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        type_text(&mut app, "old");
        assert_eq!(press(&mut app, KeyCode::Esc), Outcome::Continue);
        assert!(app.picker.is_none());
        assert_eq!(app.branch, "main");
        assert_eq!(app.title, "repo");
    }

    #[test]
    fn mode_toggles_still_work_on_branch_field() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        assert_eq!(app.mode, LaunchMode::Worktree);
        press(&mut app, KeyCode::Left);
        assert_eq!(app.mode, LaunchMode::InPlace);
        press(&mut app, KeyCode::Char('w'));
        assert_eq!(app.mode, LaunchMode::Worktree);
        assert!(app.picker.is_none());
    }

    #[test]
    fn worktree_mode_picks_branch_and_title_follows() {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Char('w'));
        assert_eq!(app.mode, LaunchMode::Worktree);
        assert_eq!(app.branch, "");
        pick(&mut app, "feat/new");
        assert_eq!(app.title, "repo:feat/new");
        assert_eq!(branch_status(&app), BranchStatus::New);
        app.field = Field::Branch;
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
        assert_eq!(request.launch, Launch::InPlace { switch: None });
        assert_eq!(request.harness.name, "Claude");
        assert_eq!(request.title, "repo");
    }

    fn submit_worktree(branch: &str) -> Launch {
        let mut app = app();
        pick(&mut app, "repo");
        app.field = Field::Branch;
        press(&mut app, KeyCode::Right);
        pick(&mut app, branch);
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

    /// Submit In Place after picking `branch` (`None` leaves it as prefilled).
    fn submit_in_place_on(dir: &str, branch: Option<&str>) -> (App<FakeProbe>, Outcome) {
        let mut app = app();
        pick(&mut app, dir);
        if let Some(branch) = branch {
            app.field = Field::Branch;
            pick(&mut app, branch);
        }
        let outcome = ctrl(&mut app, 's');
        (app, outcome)
    }

    fn in_place_switch(branch: Option<&str>) -> Option<BranchSwitch> {
        match submit_in_place_on("repo", branch) {
            (
                _,
                Outcome::Submit(Request {
                    launch: Launch::InPlace { switch },
                    ..
                }),
            ) => switch,
            (app, other) => panic!("expected in-place submit, got {other:?} ({:?})", app.error),
        }
    }

    #[test]
    fn in_place_unchanged_branch_does_not_switch() {
        assert_eq!(in_place_switch(None), None);
        assert_eq!(in_place_switch(Some("main")), None);
    }

    #[test]
    fn in_place_existing_branch_switches() {
        assert_eq!(
            in_place_switch(Some("feat/old")),
            Some(BranchSwitch {
                repo_root: PathBuf::from("/home/me/src/repo"),
                branch: "feat/old".into(),
                create: false,
                base: None,
            })
        );
    }

    #[test]
    fn in_place_new_branch_is_created_from_default_branch() {
        assert_eq!(
            in_place_switch(Some("feat/new")),
            Some(BranchSwitch {
                repo_root: PathBuf::from("/home/me/src/repo"),
                branch: "feat/new".into(),
                create: true,
                base: Some("origin/main".into()),
            })
        );
    }

    #[test]
    fn in_place_branch_with_worktree_is_refused() {
        let (app, outcome) = submit_in_place_on("repo", Some("feat/wt"));
        assert_eq!(outcome, Outcome::Continue);
        assert_eq!(app.field, Field::Branch);
        assert_eq!(
            app.error.as_deref(),
            Some("'feat/wt' is checked out at /wt/feat-wt, use Worktree mode")
        );
    }

    #[test]
    fn failed_branch_switch_keeps_form_open_on_branch() {
        let (mut app, outcome) = submit_in_place_on("repo", Some("feat/old"));
        assert!(matches!(outcome, Outcome::Submit(_)));
        app.field = Field::Title;
        app.branch_switch_failed("error: your local changes would be overwritten".into());
        assert_eq!(app.field, Field::Branch);
        assert!(app.picker.is_none());
        assert_eq!(
            app.error.as_deref(),
            Some("error: your local changes would be overwritten")
        );
        assert_eq!(
            app.branch, "feat/old",
            "the choice is kept to retry or change"
        );
    }

    #[test]
    fn detached_head_left_alone_does_not_switch() {
        let (_, outcome) = submit_in_place_on("detached", None);
        let Outcome::Submit(request) = outcome else {
            panic!("expected submit");
        };
        assert_eq!(request.launch, Launch::InPlace { switch: None });
        assert_eq!(request.title, "detached");
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
