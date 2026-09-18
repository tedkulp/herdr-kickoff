use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::Harness;

/// Whether the harness can be launched: its program is on `path` (a
/// PATH-style list). Shell, with no program, is always available.
pub fn is_available(harness: &Harness, path: Option<&OsStr>) -> bool {
    match harness.program() {
        None => true,
        Some(program) if program.contains('/') => is_executable(Path::new(program)),
        Some(program) => path.is_some_and(|path| {
            std::env::split_paths(path).any(|dir| is_executable(&dir.join(program)))
        }),
    }
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn harness(command: &str) -> Harness {
        Harness {
            name: "t".into(),
            command: command.into(),
        }
    }

    #[test]
    fn finds_executables_on_path_only() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("agent");
        fs::write(&exe, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.path().join("plain"), "").unwrap();

        let path = dir.path().as_os_str();
        assert!(is_available(&harness("agent --flag"), Some(path)));
        assert!(!is_available(&harness("plain"), Some(path)));
        assert!(!is_available(&harness("missing"), Some(path)));
        assert!(!is_available(&harness("agent"), None));
        assert!(is_available(&harness(""), None));
        assert!(is_available(&harness(exe.to_str().unwrap()), None));
    }
}
