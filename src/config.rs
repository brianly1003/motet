//! Config: meetnote's own settings from `./.env` and `~/.config/meetnote/.env`, then the shell.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The only keys a `.env` file may set. Anything else (PATH, DYLD_*, OLLAMA_HOST…) is ignored,
/// so a `.env` in a folder you happen to run meetnote from can't redirect tools or send data off
/// the Mac. The process environment is never changed; child tools see the shell's environment.
const KEYS: [&str; 4] = [
    "WHISPER_MODEL",
    "WHISPER_VAD_MODEL",
    "OLLAMA_MODEL",
    "FIX_MODEL",
];

/// A setting: `./.env` wins over `~/.config/meetnote/.env`, and both win over the shell, so a
/// stale export in a long-running terminal can't pick the wrong model.
pub(crate) fn var(key: &str) -> Option<String> {
    static FILES: OnceLock<HashMap<String, String>> = OnceLock::new();
    FILES
        .get_or_init(|| {
            let global = std::env::home_dir().map(|h| h.join(".config/meetnote/.env"));
            let mut vars = HashMap::new();
            // Later files overwrite earlier ones, so ./.env (current folder only) comes last.
            for path in global.into_iter().chain([PathBuf::from(".env")]) {
                let Ok(lines) = dotenvy::from_path_iter(&path) else {
                    continue;
                };
                for (k, v) in lines.flatten() {
                    if KEYS.contains(&k.as_str()) {
                        vars.insert(k, v);
                    }
                }
            }
            vars
        })
        .get(key)
        .cloned()
        .or_else(|| std::env::var(key).ok())
}
