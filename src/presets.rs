//! Built-in presets for Hyprland and Quickshell.
//!
//! These are *recipes* — defaults that get loaded into `config.toml` on first
//! `init`. Users can override, disable, or add their own. Auto-reload is
//! OFF by default (the distro is assumed to handle it).

use std::collections::HashMap;

use crate::config::PresetConfig;

/// Default presets that ship with dot-switch.
pub fn defaults() -> HashMap<String, PresetConfig> {
    let mut m = HashMap::new();
    m.insert(
        "hyprland".into(),
        PresetConfig {
            enabled: false,
            reload_cmd: "hyprctl reload".into(),
            reload_on_files: vec!["hypr/*.conf".into()],
        },
    );
    m.insert(
        "quickshell".into(),
        PresetConfig {
            enabled: false,
            reload_cmd: "pkill -SIGUSR1 quickshell".into(),
            reload_on_files: vec!["quickshell/**/*.qml".into()],
        },
    );
    m
}
