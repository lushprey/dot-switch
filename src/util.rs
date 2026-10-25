//! Global helpers: path expansion, symlink operations, logging setup.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

/// Expand `~` and `$ENV_VAR` references in a path string, then normalize `.`/`..`.
pub fn expand_path(input: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(input)
        .map_err(|e| anyhow!("path expansion failed for `{input}`: {e}"))?
        .into_owned();
    let path = PathBuf::from(expanded);
    Ok(normalize(&path))
}

/// Normalize a path lexically (does NOT touch the filesystem).
/// Strips `.` components and resolves `..` where possible.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                // Only pop if last is a normal component
                if !out.as_os_str().is_empty()
                    && out.components().next_back() != Some(Component::RootDir)
                {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Ensure a directory exists, creating it (and parents) if necessary.
pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create directory {}", path.display()))?;
    }
    Ok(())
}

/// Read a symlink target. Returns Ok(None) if path is not a symlink.
pub fn read_symlink_target(path: &Path) -> Result<Option<PathBuf>> {
    let meta = std::fs::symlink_metadata(path);
    match meta {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("stat {}", path.display())),
        Ok(m) if m.file_type().is_symlink() => {
            let target = std::fs::read_link(path)
                .with_context(|| format!("read_link {}", path.display()))?;
            Ok(Some(normalize(&target)))
        }
        Ok(_) => Ok(None),
    }
}

/// Create a symlink, removing an existing file/symlink at the target first.
///
/// - If target is a directory, this is an error (we refuse to overwrite dirs).
/// - If target is a symlink or regular file, it is removed first.
/// - If target's parent dir doesn't exist, it is created with 0o755.
pub fn atomic_symlink(source: &Path, target: &Path, dry_run: bool) -> Result<SymlinkOutcome> {
    if target.exists() && target.is_dir() && !target.is_symlink() {
        bail!(
            "refusing to overwrite directory {} with a symlink",
            target.display()
        );
    }

    // Ensure parent dir
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            if dry_run {
                return Ok(SymlinkOutcome::WouldCreateParent);
            }
            std::fs::create_dir_all(parent).with_context(|| {
                format!("create parent dir {}", parent.display())
            })?;
        }
    }

    // Remove existing file/symlink at target
    let mut removed_existing = false;
    if target.exists() || target.is_symlink() {
        if dry_run {
            return Ok(SymlinkOutcome::WouldReplace);
        }
        std::fs::remove_file(target)
            .with_context(|| format!("remove existing {}", target.display()))?;
        removed_existing = true;
    }

    if dry_run {
        return Ok(SymlinkOutcome::WouldCreate);
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(source, target).with_context(|| {
        format!(
            "symlink {} -> {}",
            source.display(),
            target.display()
        )
    })?;
    #[cfg(not(unix))]
    bail!("symlinks are only supported on unix");

    Ok(if removed_existing {
        SymlinkOutcome::Replaced
    } else {
        SymlinkOutcome::Created
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkOutcome {
    Created,
    Replaced,
    WouldCreate,
    WouldReplace,
    WouldCreateParent,
}

/// Initialize tracing-subscriber. Called once at startup.
pub fn init_logging(verbose: bool) {
    let filter = if verbose { "debug" } else { "warn" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .try_init();
}

/// Recursively copy a directory tree.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(&name);
        let meta = entry.file_type()?;
        if meta.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if meta.is_symlink() {
            let link = std::fs::read_link(&path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// Walk a directory and collect all file paths (relative to `root`).
pub fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_inner(root, root, &mut out)?;
    Ok(out)
}

fn walk_inner(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.file_type()?;
        if meta.is_dir() {
            walk_inner(root, &path, out)?;
        } else {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    Ok(())
}
