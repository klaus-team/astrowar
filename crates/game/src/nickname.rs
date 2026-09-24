//! Persist the last confirmed nickname on the local machine.

use crate::storage::data_file;
use std::fs;

const DEFAULT_NICKNAME: &str = "Player";
const FILE_NAME: &str = "last_nickname";

pub fn resolve_or_default(nickname: &str) -> String {
    let trimmed = nickname.trim();
    if trimmed.is_empty() {
        DEFAULT_NICKNAME.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn load_last_nickname() -> Option<String> {
    let path = data_file(FILE_NAME)?;
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
    let Some(path) = data_file(FILE_NAME) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, trimmed);
}
