mod app;
mod config;
mod fuzzy;
mod git;
mod harness;
mod herdr;
mod ui;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::app::{App, HarnessChoice, Outcome, Probe};
use crate::config::Config;
use crate::git::RepoInfo;
use crate::herdr::Herdr;

const PLUGIN_ID: &str = "workspace-launcher";
const LAST_HARNESS_FILE: &str = "last_harness";
const AVAILABILITY_REFRESH: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("action") => open_popup(),
        Some("launcher") => run_launcher(),
        _ => {
            eprintln!("usage: workspace-launcher <action|launcher>");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("workspace-launcher: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn open_popup() -> Result<()> {
    let plugin_id = std::env::var("HERDR_PLUGIN_ID").unwrap_or_else(|_| PLUGIN_ID.to_string());
    Herdr::from_env().open_popup(&plugin_id)
}

struct SystemProbe;

impl Probe for SystemProbe {
    fn inspect(&self, dir: &Path) -> Option<RepoInfo> {
        git::inspect(dir)
    }
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn is_available(harness: &config::Harness) -> bool {
    harness::is_available(harness, std::env::var_os("PATH").as_deref())
}

fn zoxide_dirs() -> Result<Vec<String>> {
    let output = Command::new("zoxide")
        .args(["query", "--list"])
        .output()
        .context("running zoxide")?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

fn run_launcher() -> Result<()> {
    let config_dir = env_dir("HERDR_PLUGIN_CONFIG_DIR");
    let state_dir = env_dir("HERDR_PLUGIN_STATE_DIR");
    let last_harness_path = state_dir.map(|dir| dir.join(LAST_HARNESS_FILE));

    let mut startup_errors = Vec::new();
    let config = Config::load(config_dir.as_deref()).unwrap_or_else(|err| {
        startup_errors.push(format!("{err:#}; using built-in harnesses"));
        Config::from_toml("").expect("empty config parses")
    });
    let zoxide = zoxide_dirs().unwrap_or_else(|err| {
        startup_errors.push(format!("{err:#}; type a /path instead"));
        Vec::new()
    });
    let last_harness = last_harness_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string());

    let harnesses = config
        .harnesses
        .into_iter()
        .map(|harness| HarnessChoice {
            available: is_available(&harness),
            harness,
        })
        .collect();
    let preferred: Vec<&str> = [last_harness.as_deref(), config.default_harness.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    let mut app = App::new(SystemProbe, zoxide, harnesses, &preferred);
    if !startup_errors.is_empty() {
        app.error = Some(startup_errors.join("; "));
    }

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, last_harness_path.as_deref());
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App<SystemProbe>,
    last_harness_path: Option<&Path>,
) -> Result<()> {
    let herdr = Herdr::from_env();
    let mut last_refresh = Instant::now();
    loop {
        if last_refresh.elapsed() >= AVAILABILITY_REFRESH {
            app.refresh_availability(is_available);
            last_refresh = Instant::now();
        }
        terminal.draw(|frame| ui::draw(frame, app, false))?;
        if !event::poll(AVAILABILITY_REFRESH)? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.handle_key(key) {
            Outcome::Continue => {}
            Outcome::Cancel => return Ok(()),
            Outcome::Submit(request) => {
                terminal.draw(|frame| ui::draw(frame, app, true))?;
                match herdr.launch(&request) {
                    Ok(()) => {
                        if let Some(path) = last_harness_path {
                            // Remembering the harness is a convenience; never fail the launch over it.
                            let _ = std::fs::write(path, &request.harness.name);
                        }
                        return Ok(());
                    }
                    Err(err) => app.error = Some(format!("{err:#}")),
                }
            }
        }
    }
}
