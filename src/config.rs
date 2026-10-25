//! Load and validate `dot-switch.toml` (global tool config).

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::util::expand_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the central profiles repository (expanded at load time).
    #[serde(default)]
    pub profiles_repo: String,

    /// How long to keep auto-backups before pruning (days). 0 = never prune.
    #[serde(default = "default_backup_keep_days")]
    pub default_backup_keep_days: u32,

    /// Per-app preset overrides. Keys are arbitrary names; built-ins are
    /// `hyprland` and `quickshell`. User-defined presets can also live here.
    #[serde(default)]
    pub presets: HashMap<String, PresetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Command run after switching a profile that touches files matching
    /// `reload_on_files`. Default empty.
    #[serde(default)]
    pub reload_cmd: String,
    /// Glob patterns (relative to $XDG_CONFIG_HOME) that trigger a reload.
    #[serde(default)]
    pub reload_on_files: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut presets = HashMap::new();
        presets.insert(
            "hyprland".into(),
            PresetConfig {
                enabled: false,
                reload_cmd: "hyprctl reload".into(),
                reload_on_files: vec!["hypr/*.conf".into()],
            },
        );
        presets.insert(
            "quickshell".into(),
            PresetConfig {
                enabled: false,
                reload_cmd: "pkill -SIGUSR1 quickshell".into(),
                reload_on_files: vec!["quickshell/**/*.qml".into()],
            },
        );
        Self {
            profiles_repo: default_profiles_repo(),
            default_backup_keep_days: default_backup_keep_days(),
            presets,
        }
    }
}

fn default_backup_keep_days() -> u32 {
    7
}

fn default_profiles_repo() -> String {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok().filter(|s| !s.is_empty());
    let base = match xdg {
        Some(p) => PathBuf::from(p),
        None => dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config")),
    };
    base.join("dot-switch")
        .join("profiles")
        .to_string_lossy()
        .into_owned()
}

impl Config {
    /// Load config from the XDG config path, writing a default if missing.
    pub fn load_or_init() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        // Expand paths
        cfg.profiles_repo = expand_path(&cfg.profiles_repo)?
            .to_string_lossy()
            .into_owned();
        Ok(cfg)
    }

    /// Save config to the XDG config path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)
            .context("serialize config")?;
        std::fs::write(&path, raw)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Path to the config file: `$XDG_CONFIG_HOME/dot-switch/config.toml`.
    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("no XDG config dir"))?;
        Ok(base.join("dot-switch").join("config.toml"))
    }

    /// Path to the state file: `$XDG_STATE_HOME/dot-switch/state.json`.
    pub fn state_path() -> Result<PathBuf> {
        let base = dirs::state_dir()
            .or_else(|| dirs::data_local_dir())
            .ok_or_else(|| anyhow::anyhow!("no XDG state dir"))?;
        Ok(base.join("dot-switch").join("state.json"))
    }

    /// Path to the backups dir: `$XDG_STATE_HOME/dot-switch/backups/`.
    pub fn backups_dir() -> Result<PathBuf> {
        let base = dirs::state_dir()
            .or_else(|| dirs::data_local_dir())
            .ok_or_else(|| anyhow::anyhow!("no XDG state dir"))?;
        Ok(base.join("dot-switch").join("backups"))
    }

    /// Path to the log file: `$XDG_STATE_HOME/dot-switch/dot-switch.log`.
    pub fn log_path() -> Result<PathBuf> {
        let base = dirs::state_dir()
            .or_else(|| dirs::data_local_dir())
            .ok_or_else(|| anyhow::anyhow!("no XDG state dir"))?;
        Ok(base.join("dot-switch").join("dot-switch.log"))
    }

    /// Validate: profiles_repo must be an absolute path or expandable.
    pub fn validate(&self) -> Result<()> {
        let p = PathBuf::from(&self.profiles_repo);
        if !p.is_absolute() {
            bail!(
                "profiles_repo must be an absolute path after expansion, got: {}",
                self.profiles_repo
            );
        }
        Ok(())
    }

    /// Look up a preset by name.
    pub fn preset(&self, name: &str) -> Option<&PresetConfig> {
        self.presets.get(name)
    }
}
