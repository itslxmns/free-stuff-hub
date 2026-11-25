//! Persisted app state (settings, seen/claimed offers, activity log) in the app data dir.

use crate::free_games::{FreeGame, OfferStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub check_interval_hours: u32,
    pub notify: bool,
    pub close_to_tray: bool,
    /// Start with Windows (hidden in the tray).
    pub launch_at_startup: bool,
    pub epic: bool,
    pub gog: bool,
    pub prime: bool,
    pub steam: bool,
    /// Add newly found 100%-off Steam games automatically (off by default).
    pub steam_auto_add: bool,
    /// Max automatic Steam adds per 24 h.
    pub steam_daily_cap: u32,
    /// Card idling: max games idled at once while building the first 2 h of playtime.
    pub idle_max_games: u32,
    /// Card idling: idle games under 2 h together first.
    pub idle_multi: bool,
    /// User has read the ToS / account-risk notice.
    pub acknowledged_risk: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            check_interval_hours: 6,
            notify: true,
            close_to_tray: true,
            launch_at_startup: false,
            epic: true,
            gog: true,
            prime: true,
            steam: true,
            steam_auto_add: false,
            steam_daily_cap: 10,
            idle_max_games: 5,
            idle_multi: true,
            acknowledged_risk: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: DateTime<Utc>,
    pub module: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Persisted {
    pub settings: Settings,
    pub seen: BTreeSet<String>,
    pub claimed: BTreeSet<String>,
    pub games: Vec<FreeGame>,
    pub last_check: Option<DateTime<Utc>>,
    /// Times of successful automatic/manual Steam adds (for the daily cap).
    pub steam_adds: Vec<DateTime<Utc>>,
    pub next_check: Option<DateTime<Utc>>,
    pub log: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    #[serde(flatten)]
    pub game: FreeGame,
    pub claimed: bool,
    pub is_new: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub games: Vec<GameView>,
    pub settings: Settings,
    pub last_check: Option<DateTime<Utc>>,
    pub next_check: Option<DateTime<Utc>>,
    pub log: Vec<LogEntry>,
    pub claimed_total: usize,
}

pub struct Store {
    path: PathBuf,
    pub data: Persisted,
    /// IDs that appeared in the most recent check and were never seen before.
    pub fresh: BTreeSet<String>,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, data, fresh: BTreeSet::new() }
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.data) {
            let tmp = self.path.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn log(&mut self, module: &str, level: &str, message: impl Into<String>) {
        self.data.log.push(LogEntry {
            at: Utc::now(),
            module: module.into(),
            level: level.into(),
            message: message.into(),
        });
        let n = self.data.log.len();
        if n > 300 {
            self.data.log.drain(0..n - 300);
        }
    }

    /// Replace the offer list; returns newly seen claimable offers (for notifications).
    pub fn apply_games(&mut self, games: Vec<FreeGame>) -> Vec<FreeGame> {
        let mut new_active = Vec::new();
        self.fresh.clear();
        for g in &games {
            if self.data.seen.insert(g.id.clone()) {
                self.fresh.insert(g.id.clone());
                if g.status != OfferStatus::Upcoming && !self.data.claimed.contains(&g.id) {
                    new_active.push(g.clone());
                }
            }
        }
        self.data.games = games;
        self.data.last_check = Some(Utc::now());
        new_active
    }

    /// Like apply_games, but keeps the current "new" markers and last-check time (phase-2 merges).
    pub fn merge_games(&mut self, games: Vec<FreeGame>) -> Vec<FreeGame> {
        let mut new_active = Vec::new();
        for g in &games {
            if self.data.seen.insert(g.id.clone()) {
                self.fresh.insert(g.id.clone());
                if g.status != OfferStatus::Upcoming && !self.data.claimed.contains(&g.id) {
                    new_active.push(g.clone());
                }
            }
        }
        self.data.games = games;
        new_active
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut games: Vec<GameView> = self.data.games.iter().map(|g| GameView {
            claimed: self.data.claimed.contains(&g.id),
            is_new: self.fresh.contains(&g.id),
            game: g.clone(),
        }).collect();
        // Active first, then reminders, then upcoming; unclaimed before claimed.
        games.sort_by_key(|g| {
            let s = match g.game.status { OfferStatus::Active => 0, OfferStatus::Reminder => 1, OfferStatus::Upcoming => 2 };
            (g.claimed, s, g.game.ends_at)
        });
        Snapshot {
            games,
            settings: self.data.settings.clone(),
            last_check: self.data.last_check,
            next_check: self.data.next_check,
            log: self.data.log.iter().rev().take(100).cloned().collect(),
            claimed_total: self.data.claimed.len(),
        }
    }
}
