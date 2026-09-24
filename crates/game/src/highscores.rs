//! Persist the top local high scores on this machine.

use crate::storage::data_file;
use bevy::prelude::Resource;
use std::fs;

const MAX_ENTRIES: usize = 3;
const FILE_NAME: &str = "high_scores";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighScoreEntry {
    pub nickname: String,
    pub score: i32,
}

#[derive(Resource, Clone)]
pub struct HighScores {
    pub entries: Vec<HighScoreEntry>,
    recorded_this_match: bool,
}

impl Default for HighScores {
    fn default() -> Self {
        Self {
            entries: load_high_scores(),
            recorded_this_match: false,
        }
    }
}

impl HighScores {
    pub fn reset_match_flag(&mut self) {
        self.recorded_this_match = false;
    }

    /// Record the local player's end-of-match score once per match.
    /// Returns true if the leaderboard changed.
    pub fn consider_score(&mut self, nickname: &str, score: i32) -> bool {
        if self.recorded_this_match {
            return false;
        }
        self.recorded_this_match = true;
        if score <= 0 {
            return false;
        }
        let nick = nickname.trim();
        if nick.is_empty() {
            return false;
        }
        let mut next = self.entries.clone();
        next.push(HighScoreEntry {
            nickname: nick.chars().take(24).collect(),
            score,
        });
        next.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.nickname.cmp(&b.nickname)));
        next.truncate(MAX_ENTRIES);
        if next == self.entries {
            return false;
        }
        self.entries = next;
        save_high_scores(&self.entries);
        true
    }

    pub fn menu_block(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let mut lines = String::from("\n\nHigh scores");
        for (i, entry) in self.entries.iter().enumerate() {
            lines.push_str(&format!(
                "\n{}. {}  {}",
                i + 1,
                entry.nickname,
                entry.score
            ));
        }
        lines
    }
}

fn load_high_scores() -> Vec<HighScoreEntry> {
    let Some(path) = data_file(FILE_NAME) else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, score_str)) = line.rsplit_once('\t') else {
            continue;
        };
        let Ok(score) = score_str.parse::<i32>() else {
            continue;
        };
        let nickname = name.trim();
        if nickname.is_empty() || score <= 0 {
            continue;
        }
        entries.push(HighScoreEntry {
            nickname: nickname.chars().take(24).collect(),
            score,
        });
    }
    entries.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.nickname.cmp(&b.nickname)));
    entries.truncate(MAX_ENTRIES);
    entries
}

fn save_high_scores(entries: &[HighScoreEntry]) {
    let Some(path) = data_file(FILE_NAME) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = entries
        .iter()
        .map(|e| format!("{}\t{}", e.nickname, e.score))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(path, body);
}
