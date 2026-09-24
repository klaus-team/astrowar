//! Platform data directory for AstroWar local files.

use std::path::PathBuf;

const APP_DIR: &str = "astrowar";

/// Conventional per-OS data directory for AstroWar files.
///
/// - Linux: `~/.local/share/astrowar`
/// - macOS: `~/Library/Application Support/astrowar`
/// - Windows: `%APPDATA%\astrowar` (Roaming)
pub fn data_dir() -> Option<PathBuf> {
    let mut dir = dirs::data_dir()?;
    dir.push(APP_DIR);
    Some(dir)
}

pub fn data_file(name: &str) -> Option<PathBuf> {
    Some(data_dir()?.join(name))
}
