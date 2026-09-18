//! Calls into the herdr CLI to turn a submitted Request into a focused workspace.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow, bail};
use serde_json::Value;

use crate::app::{Launch, Request};

pub struct Herdr {
    bin: OsString,
}

impl Herdr {
    pub fn from_env() -> Self {
        Self {
            bin: std::env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into()),
        }
    }

    /// Open the Launcher popup for this plugin.
    pub fn open_popup(&self, plugin_id: &str) -> Result<()> {
        self.call(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            plugin_id,
            "--entrypoint",
            "launcher",
            "--focus",
        ])?;
        Ok(())
    }

    /// Create (or reopen) the workspace and start the harness in its first pane.
    pub fn launch(&self, request: &Request) -> Result<()> {
        let title = request.title.as_str();
        let response = match &request.launch {
            // Any branch switch has already been done by the caller.
            Launch::InPlace { .. } => self.call(&[
                "workspace",
                "create",
                "--cwd",
                path_arg(&request.directory)?,
                "--label",
                title,
                "--focus",
            ])?,
            Launch::CreateWorktree {
                repo_root,
                branch,
                base,
            } => {
                let mut args = vec![
                    "worktree",
                    "create",
                    "--cwd",
                    path_arg(repo_root)?,
                    "--branch",
                    branch.as_str(),
                    "--label",
                    title,
                    "--focus",
                ];
                if let Some(base) = base {
                    args.extend(["--base", base.as_str()]);
                }
                self.call(&args)?
            }
            Launch::OpenWorktree { repo_root, branch } => self.call(&[
                "worktree",
                "open",
                "--cwd",
                path_arg(repo_root)?,
                "--branch",
                branch,
                "--label",
                title,
                "--focus",
            ])?,
        };

        let result = &response["result"];
        // A worktree that was already open keeps whatever is running in it.
        if result["already_open"].as_bool() == Some(true) {
            return Ok(());
        }
        let command = request.harness.command.trim();
        if command.is_empty() {
            return Ok(());
        }
        let pane = match result["root_pane"]["pane_id"].as_str() {
            Some(pane) => pane.to_string(),
            None => self.first_pane(result)?,
        };
        self.call(&["pane", "run", &pane, command])?;
        Ok(())
    }

    fn first_pane(&self, result: &Value) -> Result<String> {
        let workspace = result["workspace"]["workspace_id"]
            .as_str()
            .ok_or_else(|| anyhow!("herdr did not report the new workspace"))?;
        let panes = self.call(&["pane", "list", "--workspace", workspace])?;
        panes["result"]["panes"][0]["pane_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("workspace {workspace} has no pane to start the harness in"))
    }

    fn call(&self, args: &[&str]) -> Result<Value> {
        let output = Command::new(&self.bin).args(args).output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            bail!(
                "{}",
                error_message(&stderr)
                    .or_else(|| error_message(&stdout))
                    .unwrap_or_else(|| format!("herdr {} failed", args[..2].join(" ")))
            );
        }
        Ok(serde_json::from_str(&stdout).unwrap_or(Value::Null))
    }
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("{} is not valid UTF-8", path.display()))
}

/// herdr reports failures as JSON `{"error":{"message":…}}`; fall back to raw text.
fn error_message(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let from_json = serde_json::from_str::<Value>(text).ok().and_then(|v| {
        let error = &v["error"];
        error["message"]
            .as_str()
            .or_else(|| error.as_str())
            .map(str::to_string)
    });
    Some(from_json.unwrap_or_else(|| text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_prefers_json_message() {
        assert_eq!(
            error_message(r#"{"id":"x","error":{"code":"bad","message":"branch exists"}}"#),
            Some("branch exists".into())
        );
        assert_eq!(
            error_message("plain failure\n"),
            Some("plain failure".into())
        );
        assert_eq!(error_message("  "), None);
    }
}
