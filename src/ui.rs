use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, Field, LaunchMode, Picker, PickerKind, Probe};
use crate::git::BranchStatus;

const ACCENT: Color = Color::Cyan;

pub fn draw<P: Probe>(frame: &mut Frame, app: &App<P>, busy: bool) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(" New workspace ".bold());
    let area = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let [body, status, help] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    match &app.picker {
        Some(picker) => draw_picker(frame, body, app, picker),
        None => draw_form(frame, body, app),
    }

    let status_line = if busy {
        Line::from("Launching…".fg(ACCENT))
    } else if let Some(error) = &app.error {
        Line::from(error.replace('\n', " · ").red())
    } else {
        Line::default()
    };
    frame.render_widget(Paragraph::new(status_line), status);
    frame.render_widget(Paragraph::new(help_line(app)).dim(), help);
}

fn help_line<P: Probe>(app: &App<P>) -> Line<'static> {
    let text = match (&app.picker, app.field) {
        (Some(picker), _) if picker.kind == PickerKind::Branch => {
            "type to filter or name a new branch · ↑↓ select · enter pick · esc back"
        }
        (Some(_), _) => "type to filter · ↑↓ select · enter pick · esc back",
        (None, Field::Directory) => {
            "enter/type: choose directory · tab next · ^S launch · esc cancel"
        }
        (None, Field::Harness) => "←→ choose harness · tab next · ^S launch · esc cancel",
        (None, Field::Branch) if app.repo.is_none() => {
            "not a git repo · tab next · ^S launch · esc cancel"
        }
        (None, Field::Branch) => match app.mode {
            LaunchMode::InPlace => {
                "enter/type: choose branch · ←→/w: worktree · tab next · ^S launch · esc cancel"
            }
            LaunchMode::Worktree => {
                "enter/type: choose branch · ←→ switch mode · tab next · ^S launch · esc cancel"
            }
        },
        (None, Field::Title) => "type title · enter/^S launch · esc cancel",
    };
    Line::from(text)
}

fn draw_picker<P: Probe>(frame: &mut Frame, area: Rect, app: &App<P>, picker: &Picker) {
    let [query, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);
    let (label, empty_hint) = match picker.kind {
        PickerKind::Directory => ("Directory ", "no matches (type /path or ~/path)"),
        PickerKind::Branch => ("Branch ", "no matches (type a new branch name)"),
    };
    let mut header = vec![
        label.fg(ACCENT).bold(),
        "› ".fg(ACCENT),
        Span::raw(picker.query.as_str()),
        "▏".fg(ACCENT),
        format!("  {}", picker.entries.len()).dim(),
    ];
    let typed = picker.query.trim();
    if picker.kind == PickerKind::Branch && !typed.is_empty() {
        header.push(branch_hint(app, typed));
    }
    frame.render_widget(Paragraph::new(Line::from(header)), query);
    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|e| match picker.kind {
            PickerKind::Directory => ListItem::new(app.display_path(e)),
            PickerKind::Branch => {
                ListItem::new(Line::from(vec![Span::raw(e.clone()), branch_tag(app, e)]))
            }
        })
        .collect();
    let empty = items.is_empty();
    let list_widget = List::new(items)
        .highlight_style(Style::new().fg(Color::Black).bg(ACCENT))
        .highlight_symbol("› ");
    let mut state = ListState::default().with_selected((!empty).then_some(picker.selected));
    frame.render_stateful_widget(list_widget, list, &mut state);
    if empty {
        frame.render_widget(Paragraph::new(empty_hint).dim(), list);
    }
}

fn draw_form<P: Probe>(frame: &mut Frame, area: Rect, app: &App<P>) {
    let rows = Layout::vertical([Constraint::Length(2); 4]).split(area);

    let directory = match app.directory_label() {
        Some(dir) => Line::from(dir),
        None => Line::from("press enter to choose".dim()),
    };
    field(
        frame,
        rows[0],
        app,
        Field::Directory,
        "Directory",
        directory,
    );
    field(
        frame,
        rows[1],
        app,
        Field::Harness,
        "Harness",
        harness_line(app),
    );
    field(
        frame,
        rows[2],
        app,
        Field::Branch,
        "Branch",
        branch_line(app),
    );
    let mut title = vec![Span::raw(app.title.as_str())];
    if app.field == Field::Title {
        title.push("▏".fg(ACCENT));
    }
    field(
        frame,
        rows[3],
        app,
        Field::Title,
        "Title",
        Line::from(title),
    );
}

fn field<P: Probe>(
    frame: &mut Frame,
    area: Rect,
    app: &App<P>,
    which: Field,
    label: &str,
    value: Line,
) {
    let focused = app.field == which;
    let [label_area, value_area] =
        Layout::horizontal([Constraint::Length(13), Constraint::Min(1)]).areas(area);
    let label = if focused {
        Line::from(vec!["› ".fg(ACCENT), label.fg(ACCENT).bold()])
    } else {
        Line::from(vec!["  ".into(), label.into()])
    };
    frame.render_widget(Paragraph::new(label), label_area);
    frame.render_widget(Paragraph::new(value).wrap(Wrap { trim: false }), value_area);
}

fn harness_line<P: Probe>(app: &App<P>) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, choice) in app.harnesses.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let name = choice.harness.name.clone();
        spans.push(if !choice.available {
            Span::styled(
                format!("{name} (not on PATH)"),
                Style::new().add_modifier(Modifier::DIM | Modifier::CROSSED_OUT),
            )
        } else if app.harness == Some(i) {
            Span::styled(
                format!(" {name} "),
                Style::new().fg(Color::Black).bg(ACCENT),
            )
        } else {
            Span::raw(name)
        });
    }
    Line::from(spans)
}

fn branch_line<P: Probe>(app: &App<P>) -> Line<'static> {
    if app.repo.is_none() {
        return Line::from("not a git repo; opens in place".dim());
    }
    let toggle = |mode: LaunchMode, text: &'static str| {
        if app.mode == mode {
            Span::styled(format!("[{text}]"), Style::new().fg(ACCENT).bold())
        } else {
            Span::raw(format!(" {text} ")).dim()
        }
    };
    let mut spans = vec![
        toggle(LaunchMode::InPlace, "In place"),
        toggle(LaunchMode::Worktree, "Worktree"),
        Span::raw("  "),
    ];
    spans.push(Span::raw(app.branch.clone()));
    if app.mode == LaunchMode::InPlace && !app.switches_branch() {
        // Left as prefilled, including a detached HEAD: nothing to switch.
        spans.push(" (current)".dim());
    } else {
        spans.push(branch_hint(app, &app.branch));
    }
    Line::from(spans)
}

/// What launching on `branch` would do in the current Launch Mode.
fn branch_hint<P: Probe>(app: &App<P>, branch: &str) -> Span<'static> {
    let Some(repo) = &app.repo else {
        return Span::default();
    };
    let current = repo.is_current(branch);
    let new_from = || match &repo.base {
        Some(base) => format!("  new branch from {base}").green(),
        None => "  new branch from HEAD".green(),
    };
    match (app.mode, repo.branch_status(branch)) {
        (_, BranchStatus::Invalid) => "  invalid branch name".red(),
        (LaunchMode::InPlace, _) if current => "  current".dim(),
        (LaunchMode::InPlace, BranchStatus::New) => new_from(),
        (LaunchMode::InPlace, BranchStatus::Existing) => "  switch to existing branch".yellow(),
        (LaunchMode::InPlace, BranchStatus::CheckedOut(path)) => {
            format!("  checked out at {}, use Worktree mode", path.display()).red()
        }
        (LaunchMode::Worktree, BranchStatus::New) => new_from(),
        (LaunchMode::Worktree, BranchStatus::Existing) => "  existing branch".yellow(),
        (LaunchMode::Worktree, BranchStatus::CheckedOut(path)) => {
            format!("  reopen worktree {}", path.display()).yellow()
        }
        (_, BranchStatus::Empty) => "  choose a branch".dim(),
    }
}

/// The tag after a branch in the picker list.
fn branch_tag<P: Probe>(app: &App<P>, branch: &str) -> Span<'static> {
    let Some(repo) = &app.repo else {
        return Span::default();
    };
    if repo.is_current(branch) {
        return "  (current)".dim();
    }
    match repo.branch_status(branch) {
        BranchStatus::CheckedOut(_) => "  ⎇ worktree".yellow(),
        BranchStatus::New => "  + new".green(),
        _ => Span::default(),
    }
}
