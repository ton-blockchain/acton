//! Resolves and validates the official TON binary distribution.
//!
//! [`TonBinaries::resolve`] selects an explicit directory, the directory stored
//! in the network manifest, or an automatically installed pinned release.
//! Resolved installations must contain every executable and resource directory
//! required to create and run the local network.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::storage::{Layout, Manifest};

mod install;
mod release;

pub(super) const REQUIRED_BINARIES: &[&str] = &[
    "create-state",
    "dht-server",
    "fift",
    "generate-random-id",
    "lite-client",
    "validator-engine",
    "validator-engine-console",
];

#[derive(Debug, Clone)]
pub struct TonBinaries {
    pub root: PathBuf,
}

impl TonBinaries {
    pub async fn resolve(layout: &Layout, override_dir: Option<PathBuf>) -> Result<Self> {
        let root = if let Some(path) = override_dir {
            dunce::canonicalize(&path).with_context(|| {
                format!("TON binary directory {} does not exist", path.display())
            })?
        } else if layout.manifest.is_file() {
            if let Some(path) = Manifest::load(&layout.manifest)?.ton_bin_dir {
                dunce::canonicalize(&path).with_context(|| {
                    format!(
                        "persisted TON binary directory {} does not exist; pass --ton-bin-dir",
                        path.display()
                    )
                })?
            } else {
                install::install_pinned_release(layout).await?
            }
        } else {
            install::install_pinned_release(layout).await?
        };
        let binaries = Self { root };
        binaries.validate()?;
        Ok(binaries)
    }

    pub fn command(&self, name: &str) -> PathBuf {
        #[cfg(windows)]
        let name = format!("{name}.exe");
        self.root.join(name)
    }

    pub fn optional_command(&self, name: &str) -> Option<PathBuf> {
        let path = self.command(name);
        path.is_file().then_some(path)
    }

    pub fn lib_dir(&self) -> PathBuf {
        self.root.join("lib")
    }

    pub fn smartcont_dir(&self) -> PathBuf {
        self.root.join("smartcont")
    }

    pub fn validate(&self) -> Result<()> {
        for name in REQUIRED_BINARIES {
            let path = self.command(name);
            if !path.is_file() {
                bail!("required TON binary is missing: {}", path.display());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let metadata = fs::metadata(&path)?;
                let mode = metadata.permissions().mode();
                if mode & 0o111 == 0 {
                    fs::set_permissions(&path, fs::Permissions::from_mode(mode | 0o111))
                        .with_context(|| {
                            format!("failed to make TON binary executable: {}", path.display())
                        })?;
                }
            }
        }
        for path in [self.lib_dir(), self.smartcont_dir()] {
            if !path.is_dir() {
                bail!("required TON resources are missing: {}", path.display());
            }
        }
        Ok(())
    }
}
