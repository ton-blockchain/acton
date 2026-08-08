//! Filesystem operations used while preparing a local network.
//!
//! Bootstrap copies the TON smart-contract templates and the genesis static
//! state into per-node directories. This module keeps those recursive copy and
//! path-normalization operations separate from the network startup sequence.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use std::fs;

/// Recursively copies bootstrap assets while preserving their relative paths.
pub(super) fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
