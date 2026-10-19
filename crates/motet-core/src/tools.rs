//! Desktop child-tool search paths without changing the process environment or CLI behavior.
use std::{ffi::OsString, process::Command, sync::OnceLock};

static DESKTOP_PATH: OnceLock<OsString> = OnceLock::new();

/// Finder launches omit Homebrew. Extend only desktop child environments, never global PATH.
pub fn configure_desktop() {
    #[cfg(target_os = "macos")]
    DESKTOP_PATH.get_or_init(|| search_path(std::env::var_os("PATH").unwrap_or_default()));
}

#[cfg(any(target_os = "macos", test))]
fn search_path(existing: OsString) -> OsString {
    use std::path::PathBuf;
    let mut paths: Vec<_> = std::env::split_paths(&existing).collect();
    for directory in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        let directory = PathBuf::from(directory);
        if !paths.contains(&directory) {
            paths.push(directory);
        }
    }
    std::env::join_paths(paths).expect("existing PATH components and fixed directories are valid")
}

pub fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    if let Some(path) = DESKTOP_PATH.get() {
        command.env("PATH", path);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires installed whisper-cli, whisper-server and Ollama; run with Finder PATH"]
    fn desktop_tools_start_with_finder_path() {
        configure_desktop();
        for program in ["whisper-cli", "whisper-server", "ollama"] {
            let output = command(program)
                .arg("--help")
                .output()
                .unwrap_or_else(|error| panic!("{program}: {error}"));
            assert!(output.status.success(), "{program}: {}", output.status);
        }
    }
    #[test]
    fn finder_path_gets_local_tools_without_replacing_existing_order() {
        let result = search_path(OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"));
        let paths: Vec<_> = std::env::split_paths(&result).collect();
        assert_eq!(paths[0], PathBuf::from("/usr/bin"));
        assert!(paths.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(paths.contains(&PathBuf::from("/usr/local/bin")));
        assert_eq!(search_path(result.clone()), result);
    }
}
