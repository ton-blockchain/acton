//! Resolves and validates the official TON binary distribution.
//!
//! [`TonBinaries::resolve`] selects an explicit directory, the installation
//! persisted for this state directory, or an automatically installed pinned
//! release from the shared per-user cache.
//! Resolved installations must contain every executable and resource directory
//! required to create and run the local network.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::storage::{Layout, Manifest, NodeRole, Settings};

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
    /// Resolves the TON installation owned by a state directory.
    ///
    /// An explicit override has highest priority. Bootstrap state keeps the
    /// selected path in its immutable manifest, while joined state keeps it in
    /// settings because it has no bootstrap manifest. The path is persisted only
    /// after the complete installation passes validation, so retries and other
    /// commands cannot silently switch an initialized database to another build.
    pub async fn resolve(layout: &Layout, override_dir: Option<PathBuf>) -> Result<Self> {
        let mut joined_settings = if layout.settings.is_file() {
            let settings = Settings::load(&layout.settings)?;
            (settings.node.role == NodeRole::Joined).then_some(settings)
        } else {
            None
        };

        let root = if let Some(path) = override_dir {
            dunce::canonicalize(&path).with_context(|| {
                format!("TON binary directory {} does not exist", path.display())
            })?
        } else if let Some(path) = joined_settings
            .as_ref()
            .and_then(|settings| settings.ton_bin_dir.as_ref())
        {
            dunce::canonicalize(path).with_context(|| {
                format!(
                    "persisted TON binary directory {} does not exist; pass --ton-bin-dir",
                    path.display()
                )
            })?
        } else if layout.manifest.is_file() {
            let path = Manifest::load(&layout.manifest)?.ton_bin_dir;
            dunce::canonicalize(&path).with_context(|| {
                format!(
                    "persisted TON binary directory {} does not exist; pass --ton-bin-dir",
                    path.display()
                )
            })?
        } else {
            install::install_pinned_release().await?
        };
        let binaries = Self { root };
        binaries.validate()?;

        if let Some(settings) = &mut joined_settings
            && settings.ton_bin_dir.as_ref() != Some(&binaries.root)
        {
            settings.ton_bin_dir = Some(binaries.root.clone());
            settings.save_atomic(&layout.settings)?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Settings;

    #[tokio::test]
    async fn joined_state_reuses_its_validated_binary_directory() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(directory.path().join("state"));
        layout.create_dirs().unwrap();

        let mut settings = Settings::default();
        settings.node.role = NodeRole::Joined;
        settings.node.name = "node2".to_owned();
        settings.save_atomic(&layout.settings).unwrap();

        let binary_dir = directory.path().join("ton");
        fs::create_dir_all(binary_dir.join("lib")).unwrap();
        fs::create_dir_all(binary_dir.join("smartcont")).unwrap();
        for name in REQUIRED_BINARIES {
            fs::write(binary_dir.join(name), []).unwrap();
        }

        let resolved = TonBinaries::resolve(&layout, Some(binary_dir.clone()))
            .await
            .unwrap();
        let persisted = Settings::load(&layout.settings).unwrap();
        assert_eq!(persisted.ton_bin_dir.as_ref(), Some(&resolved.root));

        let reused = TonBinaries::resolve(&layout, None).await.unwrap();
        assert_eq!(reused.root, resolved.root);
    }
}
