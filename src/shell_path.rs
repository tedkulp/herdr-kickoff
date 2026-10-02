use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Wraps the PATH so rc-file chatter on stdout can't be mistaken for it.
const MARKER: &str = "__HERDR_KICKOFF_PATH__";

/// The PATH an interactive login `shell` ends up with: what a harness will
/// see once `herdr pane run` types it into a pane. The herdr server, and so
/// this plugin, may have been started with a bare system PATH. `None` if the
/// shell fails, prints nothing usable, or takes longer than `timeout`.
pub fn login_shell_path(shell: &OsStr, timeout: Duration) -> Option<OsString> {
    let mut command = Command::new(shell);
    command.args([
        "-l",
        "-i",
        "-c",
        &format!("printf '\\n{MARKER}%s{MARKER}\\n' \"$PATH\""),
    ]);
    parse(&output_within(command, timeout)?)
}

/// stdout of `command`, killed and discarded if it outlives `timeout`.
fn output_within(mut command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on a thread so a chatty shell can't block on a full pipe.
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => return reader.join().ok()?.ok(),
            Some(_) => return None,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn parse(stdout: &[u8]) -> Option<OsString> {
    let marker = MARKER.as_bytes();
    let start = find(stdout, marker)? + marker.len();
    let len = find(&stdout[start..], marker)?;
    let path = &stdout[start..start + len];
    (!path.is_empty()).then(|| OsStr::from_bytes(path).to_os_string())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(path: &str) -> Vec<u8> {
        format!("{MARKER}{path}{MARKER}").into_bytes()
    }

    #[test]
    fn parses_path_between_markers_ignoring_chatter() {
        let mut out = b"welcome!\n".to_vec();
        out.extend(marked("/a:/b"));
        out.extend(b"\nbye\n");
        assert_eq!(parse(&out), Some("/a:/b".into()));
    }

    #[test]
    fn rejects_missing_or_empty_path() {
        assert_eq!(parse(b"no markers here"), None);
        assert_eq!(parse(&marked("")), None);
    }

    #[test]
    fn reads_path_from_a_real_shell() {
        let path = login_shell_path(OsStr::new("sh"), Duration::from_secs(5));
        assert!(path.is_some_and(|p| !p.is_empty()));
    }

    #[test]
    fn gives_up_on_a_slow_command() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5"]);
        let started = Instant::now();
        assert_eq!(output_within(command, Duration::from_millis(100)), None);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn missing_shell_gives_none() {
        let shell = OsStr::new("definitely-not-a-shell-xyz");
        assert_eq!(login_shell_path(shell, Duration::from_secs(1)), None);
    }
}
