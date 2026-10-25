//! Core switch engine. Knows nothing about CLI / TUI — just applies manifests.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glob::Pattern;
use tracing::{debug, info, warn};

use crate::backup::Backup;
use crate::config::Config;
use crate::manifest::Manifest;
use crate::state::State;
use crate::util::{SymlinkOutcome, atomic_symlink, expand_path};

/// Result of a single file mapping operation during a switch.
#[derive(Debug, Clone)]
pub struct FileResult {
    pub source: String,
    pub target: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Created a fresh symlink.
    Created,
    /// Replaced an existing file/symlink with a new symlink.
    Replaced,
    /// Target was already a symlink pointing at our source — skipped.
    AlreadyLinked,
    /// Dry-run: would have created.
    WouldCreate,
    /// Dry-run: would have replaced.
    WouldReplace,
}

#[derive(Debug, Clone, Default)]
pub struct SwitchReport {
    pub profile: String,
    pub files: Vec<FileResult>,
    pub backed_up: Vec<String>,
    pub reloaded_presets: Vec<String>,
}

impl SwitchReport {
    pub fn summary_line(&self) -> String {
        let created = self.files.iter().filter(|f| f.outcome == Outcome::Created).count();
        let replaced = self.files.iter().filter(|f| f.outcome == Outcome::Replaced).count();
        let skipped = self.files.iter().filter(|f| f.outcome == Outcome::AlreadyLinked).count();
        let would_create = self.files.iter().filter(|f| f.outcome == Outcome::WouldCreate).count();
        let would_replace = self.files.iter().filter(|f| f.outcome == Outcome::WouldReplace).count();

        if would_create + would_replace > 0 {
            format!(
                "[dry-run] profile `{}`: would create {}, would replace {}, already linked {}",
                self.profile, would_create, would_replace, skipped
            )
        } else {
            format!(
                "profile `{}`: created {}, replaced {}, already linked {}{}{}",
                self.profile,
                created,
                replaced,
                skipped,
                if self.backed_up.is_empty() {
                    String::new()
                } else {
                    format!(", backed up {}", self.backed_up.len())
                },
                if self.reloaded_presets.is_empty() {
                    String::new()
                } else {
                    format!(", reloaded [{}]", self.reloaded_presets.join(", "))
                },
            )
        }
    }
}

/// Options passed to the switch engine.
#[derive(Debug, Clone, Default)]
pub struct SwitchOptions {
    pub only: Option<String>,
    pub backup: bool,
    pub dry_run: bool,
    pub force_reload: bool,
    pub no_reload: bool,
}

/// Discover all profile names by listing directories inside `profiles_repo`
/// that contain a `dot-switch.toml` manifest.
pub fn list_profiles(profiles_repo: &Path) -> Result<Vec<String>> {
    if !profiles_repo.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(profiles_repo)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("dot-switch.toml").exists() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Load a profile manifest by name.
pub fn load_profile(profiles_repo: &Path, name: &str) -> Result<(PathBuf, Manifest)> {
    let dir = profiles_repo.join(name);
    if !dir.exists() {
        bail!("profile `{}` not found in {}", name, profiles_repo.display());
    }
    let manifest = Manifest::load(&dir)?;
    Ok((dir, manifest))
}

/// Compute the XDG_CONFIG_HOME path (fallback ~/.config).
pub fn xdg_config_home() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        if !p.is_empty() {
            return expand_path(&p);
        }
    }
    dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("no XDG config dir"))
}

/// Apply a profile: symlink every mapping in its manifest into $XDG_CONFIG_HOME.
pub fn apply_profile(
    cfg: &Config,
    profile_name: &str,
    opts: &SwitchOptions,
) -> Result<SwitchReport> {
    let profiles_repo = PathBuf::from(&cfg.profiles_repo);
    let (profile_dir, manifest) = load_profile(&profiles_repo, profile_name)?;
    let xdg = xdg_config_home()?;

    let mappings = manifest.filter_only(opts.only.as_deref());
    if mappings.is_empty() {
        bail!(
            "no file mappings matched `--only {:?}` in profile `{}`",
            opts.only,
            profile_name
        );
    }

    // Backup phase
    let mut backed_up = Vec::new();
    if opts.backup && !opts.dry_run {
        let backups_root = Config::backups_dir()?;
        let backup = Backup::create(&backups_root)?;
        for m in &mappings {
            let target_rel = PathBuf::from(&m.target);
            backup.snapshot_file(&xdg, &target_rel)?;
            backed_up.push(m.target.clone());
        }
        info!("backed up {} files to {}", backed_up.len(), backup.dir.display());
    } else if opts.backup && opts.dry_run {
        for m in &mappings {
            backed_up.push(format!("(would back up) {}", m.target));
        }
    }

    // Switch phase
    let mut files = Vec::new();
    for m in &mappings {
        let source = manifest.source_path(&profile_dir, m);
        let target = manifest.target_path(&xdg, m);

        if !source.exists() {
            bail!(
                "profile `{}`: source file `{}` does not exist at {}",
                profile_name,
                m.source,
                source.display()
            );
        }

        // Idempotency check
        if let Some(existing_target) = crate::util::read_symlink_target(&target)? {
            let normalized_source = crate::util::normalize(&source);
            if existing_target == normalized_source {
                files.push(FileResult {
                    source: m.source.clone(),
                    target: m.target.clone(),
                    outcome: Outcome::AlreadyLinked,
                });
                debug!("already linked: {}", target.display());
                continue;
            }
        }

        let outcome = atomic_symlink(&source, &target, opts.dry_run)?;
        let mapped = match outcome {
            SymlinkOutcome::Created => Outcome::Created,
            SymlinkOutcome::Replaced => Outcome::Replaced,
            SymlinkOutcome::WouldCreate | SymlinkOutcome::WouldCreateParent => Outcome::WouldCreate,
            SymlinkOutcome::WouldReplace => Outcome::WouldReplace,
        };
        files.push(FileResult {
            source: m.source.clone(),
            target: m.target.clone(),
            outcome: mapped,
        });
    }

    // Reload phase
    let reloaded_presets = if opts.no_reload {
        Vec::new()
    } else {
        run_reloads(cfg, &files, opts.force_reload, opts.dry_run)?
    };

    // State update
    if !opts.dry_run {
        let mut state = State::load()?;
        state.record_switch(profile_name);
        state.save()?;
    }

    Ok(SwitchReport {
        profile: profile_name.to_string(),
        files,
        backed_up,
        reloaded_presets,
    })
}

/// Diff: print what would change without touching the filesystem.
pub fn diff_profile(cfg: &Config, profile_name: &str, only: Option<&str>) -> Result<SwitchReport> {
    let opts = SwitchOptions {
        only: only.map(|s| s.to_string()),
        backup: false,
        dry_run: true,
        force_reload: false,
        no_reload: true,
    };
    apply_profile(cfg, profile_name, &opts)
}

/// Run a preset's reload command if any changed file matches its
/// `reload_on_files` globs (relative to $XDG_CONFIG_HOME).
fn run_reloads(
    cfg: &Config,
    files: &[FileResult],
    force: bool,
    dry_run: bool,
) -> Result<Vec<String>> {
    let mut fired = Vec::new();
    let changed: Vec<&str> = files
        .iter()
        .filter(|f| f.outcome == Outcome::Created || f.outcome == Outcome::Replaced)
        .map(|f| f.target.as_str())
        .collect();

    if changed.is_empty() {
        return Ok(fired);
    }

    for (name, preset) in &cfg.presets {
        if !preset.enabled && !force {
            continue;
        }
        if preset.reload_cmd.trim().is_empty() {
            continue;
        }
        // Does any changed file match a reload_on_files glob?
        let mut matched = false;
        for pat in &preset.reload_on_files {
            let Ok(p) = Pattern::new(pat) else {
                warn!("invalid glob `{}` in preset `{}`", pat, name);
                continue;
            };
            for c in &changed {
                if p.matches(c) {
                    matched = true;
                    break;
                }
            }
            if matched {
                break;
            }
        }
        if !matched {
            continue;
        }

        if dry_run {
            info!("[dry-run] would run: `{}`", preset.reload_cmd);
            fired.push(format!("{} (dry-run)", name));
            continue;
        }

        info!("running reload: `{}`", preset.reload_cmd);
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&preset.reload_cmd)
            .status()
            .with_context(|| format!("spawn reload cmd: `{}`", preset.reload_cmd));
        match status {
            Ok(s) if s.success() => fired.push(name.clone()),
            Ok(s) => warn!("reload `{}` exited with {:?}", preset.reload_cmd, s.code()),
            Err(e) => warn!("reload `{}` failed: {:#}", preset.reload_cmd, e),
        }
    }
    Ok(fired)
}

/// Apply `--all`: for every profile in the repo, ensure every mapping is
/// symlinked. Useful for "reset everything to canonical state".
pub fn apply_all(cfg: &Config, dry_run: bool) -> Result<Vec<SwitchReport>> {
    let profiles_repo = PathBuf::from(&cfg.profiles_repo);
    let names = list_profiles(&profiles_repo)?;
    if names.is_empty() {
        bail!("no profiles found in {}", profiles_repo.display());
    }
    let mut reports = Vec::new();
    for name in &names {
        let opts = SwitchOptions {
            dry_run,
            no_reload: true,
            ..Default::default()
        };
        // apply_all should not fail the whole run if one profile is missing a
        // source — but we let real errors bubble.
        match apply_profile(cfg, name, &opts) {
            Ok(r) => reports.push(r),
            Err(e) => warn!("apply_all: profile `{}` skipped: {:#}", name, e),
        }
    }
    Ok(reports)
}

/// Revert to the previous profile (from state history).
pub fn revert(cfg: &Config, dry_run: bool) -> Result<Option<SwitchReport>> {
    let mut state = State::load()?;
    let prev = match state.revert() {
        Some(p) => p,
        None => return Ok(None),
    };
    if !dry_run {
        state.save()?;
    }
    let opts = SwitchOptions {
        dry_run,
        no_reload: false,
        ..Default::default()
    };
    let report = apply_profile(cfg, &prev, &opts)?;
    Ok(Some(report))
}

/// Snapshot the *current live state* of `$XDG_CONFIG_HOME/<paths>` into a
/// backup directory without changing anything. Useful as a safety net before
/// manual edits.
pub fn snapshot_paths(paths: &[String]) -> Result<PathBuf> {
    let xdg = xdg_config_home()?;
    let backups_root = Config::backups_dir()?;
    let backup = Backup::create(&backups_root)?;
    for p in paths {
        let rel = Path::new(p);
        backup.snapshot_file(&xdg, rel)?;
    }
    Ok(backup.dir)
}
