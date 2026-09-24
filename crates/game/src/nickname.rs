//! Persist the last confirmed nickname on the local machine.

use std::fs;
use std::path::PathBuf;

const DEFAULT_NICKNAME: &str = "Player";

pub fn resolve_or_default(nickname: &str) -> String {
    let trimmed = nickname.trim();
    if trimmed.is_empty() {
        DEFAULT_NICKNAME.to_string()
    } else {
        trimmed.to_string()
    }
}

fn storage_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let mut dir = PathBuf::from(home);
    dir.push(".local");
    dir.push("share");
    dir.push("astrowar");
    Some(dir.join("last_nickname"))
}

pub fn load_last_nickname() -> Option<String> {
    let path = storage_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 24 {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn save_last_nickname(nickname: &str) {
    let trimmed = nickname.trim();
    if trimmed.is_empty() || trimmed.len() > 24 {
        return;
    }
    let Some(path) = storage_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, trimmed);
}
