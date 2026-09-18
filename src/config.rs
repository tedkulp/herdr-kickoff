use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// A coding-agent CLI the Launcher can start in the new workspace.
/// An empty `command` is the plain Shell harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub command: String,
}

impl Harness {
    fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.to_string(),
            command: command.to_string(),
        }
    }

    /// The executable that must be on PATH for this harness to be available.
    pub fn program(&self) -> Option<&str> {
        self.command.split_whitespace().next()
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub default_harness: Option<String>,
    pub harnesses: Vec<Harness>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    default_harness: Option<String>,
    #[serde(default)]
    harnesses: Vec<FileHarness>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileHarness {
    name: String,
    #[serde(default)]
    command: String,
}

pub fn builtin_harnesses() -> Vec<Harness> {
    vec![
        Harness::new("Opencode", "opencode"),
        Harness::new("Codex", "codex"),
        Harness::new("Claude", "claude"),
        Harness::new("pi", "pi"),
        Harness::new("omp", "omp"),
        Harness::new("Shell", ""),
    ]
}

impl Config {
    pub fn load(config_dir: Option<&Path>) -> Result<Self> {
        let Some(path) = config_dir.map(|dir| dir.join("config.toml")) else {
            return Self::from_toml("");
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                Self::from_toml(&text).with_context(|| format!("invalid {}", path.display()))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::from_toml(""),
            Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Configured harnesses replace a built-in of the same name (case-insensitive)
    /// or are added before Shell, which always stays last.
    pub fn from_toml(text: &str) -> Result<Self> {
        let file: FileConfig = toml::from_str(text)?;
        let mut harnesses = builtin_harnesses();
        for entry in file.harnesses {
            let harness = Harness::new(entry.name.trim(), entry.command.trim());
            match harnesses
                .iter_mut()
                .find(|h| h.name.eq_ignore_ascii_case(&harness.name))
            {
                Some(existing) => *existing = harness,
                None => {
                    let shell = harnesses.iter().position(|h| h.name == "Shell");
                    harnesses.insert(shell.unwrap_or(harnesses.len()), harness);
                }
            }
        }
        Ok(Self {
            default_harness: file.default_harness,
            harnesses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(config: &Config) -> Vec<&str> {
        config.harnesses.iter().map(|h| h.name.as_str()).collect()
    }

    #[test]
    fn empty_config_uses_builtins() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(
            names(&config),
            ["Opencode", "Codex", "Claude", "pi", "omp", "Shell"]
        );
        assert_eq!(config.default_harness, None);
    }

    #[test]
    fn override_replaces_by_name_and_new_ones_go_before_shell() {
        let config = Config::from_toml(
            r#"
            default_harness = "Claude"
            [[harnesses]]
            name = "claude"
            command = "claude --continue"
            [[harnesses]]
            name = "Aider"
            command = "aider"
            "#,
        )
        .unwrap();
        assert_eq!(
            names(&config),
            ["Opencode", "Codex", "claude", "pi", "omp", "Aider", "Shell"]
        );
        assert_eq!(config.harnesses[2].command, "claude --continue");
        assert_eq!(config.default_harness.as_deref(), Some("Claude"));
    }

    #[test]
    fn program_is_first_word() {
        assert_eq!(
            Harness::new("x", "claude --continue").program(),
            Some("claude")
        );
        assert_eq!(Harness::new("Shell", "").program(), None);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::from_toml("colour = 1").is_err());
    }
}
