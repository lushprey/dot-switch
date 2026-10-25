//! Parse profile manifests (`dot-switch.toml` inside each profile dir).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// A profile's manifest, declaring which files it owns and where they
/// should be symlinked inside `$XDG_CONFIG_HOME`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub files: Vec<FileMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMapping {
    /// Source path inside the profile dir.
    pub source: String,
    /// Target path inside `$XDG_CONFIG_HOME` (relative).
    pub target: String,
}

impl Manifest {
    /// Load a manifest from `<profile_dir>/dot-switch.toml`.
    pub fn load(profile_dir: &Path) -> Result<Self> {
        let path = profile_dir.join("dot-switch.toml");
        if !path.exists() {
            bail!(
                "no `dot-switch.toml` manifest found in {}",
                profile_dir.display()
            );
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let manifest: Manifest = toml::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        // Sanity: no target should escape $XDG_CONFIG_HOME
        for f in &manifest.files {
            if f.target.starts_with('/') || f.target.contains("..") {
                bail!(
                    "manifest `{}`: target `{}` must be a relative path inside $XDG_CONFIG_HOME",
                    manifest.name,
                    f.target
                );
            }
            if f.source.starts_with('/') || f.source.contains("..") {
                bail!(
                    "manifest `{}`: source `{}` must be a relative path inside the profile dir",
                    manifest.name,
                    f.source
                );
            }
        }
        if manifest.name.trim().is_empty() {
            bail!("manifest has empty `name` field");
        }
        Ok(manifest)
    }

    /// Returns the absolute source path for a mapping (inside the profile dir).
    pub fn source_path(&self, profile_dir: &Path, mapping: &FileMapping) -> PathBuf {
        profile_dir.join(&mapping.source)
    }

    /// Returns the absolute target path (inside $XDG_CONFIG_HOME).
    pub fn target_path(&self, xdg_config: &Path, mapping: &FileMapping) -> PathBuf {
        xdg_config.join(&mapping.target)
    }

    /// Filter mappings by `--only` selector. The selector can match either
    /// `source` or `target` (whichever the user typed).
    pub fn filter_only<'a>(
        &'a self,
        only: Option<&str>,
    ) -> Vec<&'a FileMapping> {
        match only {
            None => self.files.iter().collect(),
            Some(sel) => self
                .files
                .iter()
                .filter(|f| f.source == sel || f.target == sel)
                .collect(),
        }
    }
}
