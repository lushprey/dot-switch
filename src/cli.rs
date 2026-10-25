//! clap command definitions. clap auto-provides `-h` / `--help` on the root
//! command and on every subcommand.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "dot-switch",
    version,
    about = "Swap ~/.config files between named profiles via symlinks.\nBuilt for Hyprland/Quickshell, generic for everything.",
    long_about = "dot-switch manages a central repository of config profiles and swaps them into\n$XDG_CONFIG_HOME using symlinks. Each profile declares which files it owns\nvia a dot-switch.toml manifest. Reload behaviour is opt-in via the global\nconfig — by default, dot-switch only swaps files and prints what changed.",
    disable_help_flag = false,
    disable_help_subcommand = false
)]
pub struct Cli {
    /// Increase verbosity (-v = debug, default = warn).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize dot-switch (write default config.toml, create profiles dir).
    Init,

    /// Switch to a named profile.
    #[command(alias = "switch")]
    Use {
        /// Profile name (must exist in the profiles repo).
        name: String,

        /// Only apply a single file from the profile (matches `source` or `target`).
        #[arg(long, value_name = "PATH")]
        only: Option<String>,

        /// Snapshot current targets before switching.
        #[arg(long)]
        backup: bool,

        /// Print actions without performing them.
        #[arg(long)]
        dry_run: bool,

        /// Skip reload commands even if a preset has `enabled = true`.
        #[arg(long, global = true)]
        no_reload: bool,

        /// Force reload even for presets with `enabled = false`.
        #[arg(long, conflicts_with = "no_reload", global = true)]
        reload: bool,
    },

    /// Apply all profiles in the repo (resets everything to canonical state).
    Apply {
        /// Print actions without performing them.
        #[arg(long)]
        dry_run: bool,
    },

    /// List known profiles.
    #[command(alias = "ls")]
    List,

    /// Show what `use <profile>` would change without touching the filesystem.
    Diff {
        /// Profile name.
        name: String,
        /// Only diff a single file (matches `source` or `target`).
        #[arg(long, value_name = "PATH")]
        only: Option<String>,
    },

    /// Print the currently-active profile.
    Current,

    /// Revert to the previous profile (from history).
    Revert {
        #[arg(long)]
        dry_run: bool,
    },

    /// Take a manual snapshot of current live files (no switch).
    Snapshot {
        /// Paths inside $XDG_CONFIG_HOME to snapshot (e.g. `hypr/hyprland.conf`).
        #[arg(required = true)]
        paths: Vec<String>,
    },

    /// Create a new profile from current live ~/.config files.
    New {
        /// New profile name.
        name: String,
        /// Live files to copy into the profile, as `target` paths inside $XDG_CONFIG_HOME.
        #[arg(long, value_name = "PATH", num_args = 1.., required = true)]
        from_live: Vec<String>,
    },

    /// Edit the global config in $EDITOR.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Print all the paths dot-switch uses (config, state, backups, profiles).
    Paths,

    /// Generate shell completions.
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// Edit config.toml in $EDITOR (or $VISUAL, or vi).
    Edit,
    /// Show the path to config.toml.
    Path,
    /// Print the current config.toml to stdout.
    Show,
}

/// Helper: resolve $EDITOR / $VISUAL / vi.
pub fn resolve_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into())
}

/// Helper: locate a profile repo directory. Used by `new`.
pub fn profile_repo_dir(cfg_profiles_repo: &str) -> PathBuf {
    PathBuf::from(cfg_profiles_repo)
}
