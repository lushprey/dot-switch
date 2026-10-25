//! Backup logic: snapshot current target files before a switch.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;

use crate::util::{copy_dir_recursive, ensure_dir};

/// A backup directory, named by timestamp.
pub struct Backup {
    pub dir: PathBuf,
    pub timestamp: String,
}

impl Backup {
    /// Create a fresh timestamped backup dir. If the timestamp already
    /// exists (same second), appends a short suffix to disambiguate.
    pub fn create(backups_root: &Path) -> Result<Self> {
        let base_ts = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let mut dir = backups_root.join(&base_ts);
        let mut timestamp = base_ts.clone();
        if dir.exists() {
            // Append a short counter until we find a free slot.
            let mut i = 1;
            loop {
                timestamp = format!("{}-{}", base_ts, i);
                dir = backups_root.join(&timestamp);
                if !dir.exists() {
                    break;
                }
                i += 1;
                if i > 999 {
                    bail!("could not find a free backup dir name in {}", backups_root.display());
                }
            }
        }
        ensure_dir(&dir)?;
        Ok(Backup { dir, timestamp })
    }

    /// Snapshot a single file/symlink into the backup dir, preserving its
    /// relative path (with `/` replaced by `__` to flatten).
    pub fn snapshot_file(&self, xdg_config: &Path, target_rel: &Path) -> Result<()> {
        let live = xdg_config.join(target_rel);
        if !live.exists() && !live.is_symlink() {
            // Nothing to back up.
            return Ok(());
        }
        let flat = target_rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("__");
        let dest = self.dir.join(flat);
        if let Some(parent) = dest.parent() {
            ensure_dir(parent)?;
        }

        let meta = std::fs::symlink_metadata(&live)?;
        if meta.file_type().is_symlink() {
            let link = std::fs::read_link(&live)?;
            // Remove existing dest if present (idempotent re-snapshot)
            if dest.exists() || dest.is_symlink() {
                std::fs::remove_file(&dest).ok();
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &dest)
                .with_context(|| format!("snapshot symlink {}", live.display()))?;
        } else if meta.file_type().is_dir() {
            copy_dir_recursive(&live, &dest)
                .with_context(|| format!("snapshot dir {}", live.display()))?;
        } else {
            std::fs::copy(&live, &dest)
                .with_context(|| format!("snapshot file {}", live.display()))?;
        }
        Ok(())
    }
}

/// Prune backup directories older than `keep_days`. 0 disables pruning.
pub fn prune(backups_root: &Path, keep_days: u32) -> Result<usize> {
    if keep_days == 0 {
        return Ok(0);
    }
    if !backups_root.exists() {
        return Ok(0);
    }
    let now = Utc::now();
    let mut removed = 0;
    let cutoff = chrono::Duration::days(keep_days as i64);
    for entry in std::fs::read_dir(backups_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(ts) = parse_ts(&name) else {
            continue;
        };
        if now.signed_duration_since(ts) > cutoff {
            if std::fs::remove_dir_all(&path).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn parse_ts(name: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(name, "%Y%m%dT%H%M%S")
        .ok()
        .map(|n| chrono::DateTime::<Utc>::from_naive_utc_and_offset(n, Utc))
}
