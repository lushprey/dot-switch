//! Persistent state: current profile + history ring buffer.
//!
//! Stored as JSON at `$XDG_STATE_HOME/dot-switch/state.json`.

use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    /// Currently-active profile name (None if no profile has been switched to).
    #[serde(default)]
    pub current: Option<String>,
    /// History of previously-active profiles, newest first. Capped at 20.
    #[serde(default)]
    pub history: VecDeque<String>,
}

const HISTORY_LIMIT: usize = 20;

impl State {
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(State::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let state: State = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn path() -> Result<PathBuf> {
        crate::config::Config::state_path()
    }

    /// Record that a profile was just switched to.
    pub fn record_switch(&mut self, profile: &str) {
        if self.current.as_deref() == Some(profile) {
            return;
        }
        if let Some(prev) = self.current.take() {
            self.history.push_front(prev);
            while self.history.len() > HISTORY_LIMIT {
                self.history.pop_back();
            }
        }
        self.current = Some(profile.to_string());
    }

    /// Pop the most recent profile from history and make it current.
    /// Returns the new current profile, or None if history was empty.
    pub fn revert(&mut self) -> Option<String> {
        if let Some(prev) = self.history.pop_front() {
            if let Some(cur) = self.current.take() {
                self.history.push_front(cur);
                while self.history.len() > HISTORY_LIMIT {
                    self.history.pop_back();
                }
            }
            self.current = Some(prev.clone());
            Some(prev)
        } else {
            None
        }
    }
}
