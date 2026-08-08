use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use fs2::FileExt;
use tokio::process::Command;
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_128;

use crate::contract_registry::VerifiedSourceRegistration;
use crate::{ContractSourceArtifactError, ContractSourceArtifactStore};

const EXCLUDED_PROJECT_DIRECTORIES: &[&str] = &[
    ".acton",
    ".git",
    ".studio",
    "build",
    "node_modules",
    "target",
];

pub(crate) struct ProjectArtifactSynchronizer {
    acton_executable: PathBuf,
    workspace_root: PathBuf,
    current_dir: PathBuf,
    staging_dir: PathBuf,
    artifact_store: ContractSourceArtifactStore,
    publish_lock_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectFingerprint(Vec<ProjectFileFingerprint>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProjectFileFingerprint {
    path: PathBuf,
    content_hash: u128,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ArtifactSyncError {
    #[error("Failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Acton build exited with {status}")]
    BuildFailed { status: ExitStatus },
    #[error("Artifact filesystem worker failed: {message}")]
    WorkerFailed { message: String },
    #[error(transparent)]
    Store(#[from] ContractSourceArtifactError),
}

impl ProjectArtifactSynchronizer {
    pub(crate) fn new(
        acton_executable: impl Into<PathBuf>,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        let acton_executable = acton_executable.into();
        let workspace_root = workspace_root.into();
        let artifacts_root = workspace_root
            .join(".studio")
            .join("artifacts")
            .join("contracts");
        Self {
            acton_executable,
            workspace_root,
            current_dir: artifacts_root.join("current"),
            staging_dir: artifacts_root.join(format!(".staging-{}", Uuid::new_v4())),
            artifact_store: ContractSourceArtifactStore::from_history_root(
                artifacts_root.join("by-bundle"),
            ),
            publish_lock_path: artifacts_root.join(".publish.lock"),
        }
    }

    pub(crate) async fn build_and_store(
        &self,
    ) -> Result<Vec<VerifiedSourceRegistration>, ArtifactSyncError> {
        let staging_dir = self.staging_dir.clone();
        tokio::task::spawn_blocking(move || prepare_staging_source_dir(&staging_dir))
            .await
            .map_err(|error| ArtifactSyncError::WorkerFailed {
                message: error.to_string(),
            })??;

        let status = Command::new(&self.acton_executable)
            .arg("--project-root")
            .arg(&self.workspace_root)
            .arg("build")
            .arg("--output-sources")
            .arg(&self.staging_dir)
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|source| ArtifactSyncError::Io {
                operation: "start Acton build with",
                path: self.acton_executable.clone(),
                source,
            })?;
        if !status.success() {
            return Err(ArtifactSyncError::BuildFailed { status });
        }

        let staging_dir = self.staging_dir.clone();
        let artifact_store = self.artifact_store.clone();
        let registrations = tokio::task::spawn_blocking(move || {
            load_and_persist_source_artifacts(&staging_dir, &artifact_store)
        })
        .await
        .map_err(|error| ArtifactSyncError::WorkerFailed {
            message: error.to_string(),
        })??;
        let staging_dir = self.staging_dir.clone();
        let current_dir = self.current_dir.clone();
        let publish_lock_path = self.publish_lock_path.clone();
        tokio::task::spawn_blocking(move || {
            replace_current_source_dir(&staging_dir, &current_dir, &publish_lock_path)
        })
        .await
        .map_err(|error| ArtifactSyncError::WorkerFailed {
            message: error.to_string(),
        })??;
        Ok(registrations)
    }

    pub(crate) async fn load_history(
        &self,
    ) -> Result<Vec<VerifiedSourceRegistration>, ArtifactSyncError> {
        let artifact_store = self.artifact_store.clone();
        tokio::task::spawn_blocking(move || {
            artifact_store
                .load_all()
                .map(|artifacts| {
                    artifacts
                        .into_iter()
                        .map(|artifact| VerifiedSourceRegistration {
                            code_hash: artifact.code_hash,
                            source: artifact.source,
                        })
                        .collect()
                })
                .map_err(ArtifactSyncError::from)
        })
        .await
        .map_err(|error| ArtifactSyncError::WorkerFailed {
            message: error.to_string(),
        })?
    }

    pub(crate) async fn fingerprint(&self) -> Result<ProjectFingerprint, ArtifactSyncError> {
        let workspace_root = self.workspace_root.clone();
        tokio::task::spawn_blocking(move || project_fingerprint(&workspace_root))
            .await
            .map_err(|error| ArtifactSyncError::WorkerFailed {
                message: error.to_string(),
            })?
    }
}

fn prepare_staging_source_dir(staging_dir: &Path) -> Result<(), ArtifactSyncError> {
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir).map_err(|source| ArtifactSyncError::Io {
            operation: "clear source artifact staging directory",
            path: staging_dir.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(staging_dir).map_err(|source| ArtifactSyncError::Io {
        operation: "create source artifact staging directory",
        path: staging_dir.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn replace_current_source_dir(
    staging_dir: &Path,
    current_dir: &Path,
    publish_lock_path: &Path,
) -> Result<(), ArtifactSyncError> {
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(publish_lock_path)
        .map_err(|source| ArtifactSyncError::Io {
            operation: "open source artifact publish lock",
            path: publish_lock_path.to_path_buf(),
            source,
        })?;
    lock_file
        .lock_exclusive()
        .map_err(|source| ArtifactSyncError::Io {
            operation: "lock source artifact publication",
            path: publish_lock_path.to_path_buf(),
            source,
        })?;

    let previous_dir = current_dir.with_file_name(".previous");
    if !current_dir.exists() && previous_dir.exists() {
        fs::rename(&previous_dir, current_dir).map_err(|source| ArtifactSyncError::Io {
            operation: "recover current source artifact directory",
            path: current_dir.to_path_buf(),
            source,
        })?;
    } else if previous_dir.exists() {
        fs::remove_dir_all(&previous_dir).map_err(|source| ArtifactSyncError::Io {
            operation: "remove previous source artifact directory",
            path: previous_dir.clone(),
            source,
        })?;
    }

    let had_current = current_dir.exists();
    if had_current {
        fs::rename(current_dir, &previous_dir).map_err(|source| ArtifactSyncError::Io {
            operation: "move current source artifact directory",
            path: current_dir.to_path_buf(),
            source,
        })?;
    }
    if let Err(source) = fs::rename(staging_dir, current_dir) {
        if had_current {
            let _ = fs::rename(&previous_dir, current_dir);
        }
        return Err(ArtifactSyncError::Io {
            operation: "activate generated source artifact directory",
            path: current_dir.to_path_buf(),
            source,
        });
    }
    if had_current && let Err(error) = fs::remove_dir_all(&previous_dir) {
        tracing::warn!(
            path = %previous_dir.display(),
            %error,
            "Failed to clean previous source artifact directory after publication"
        );
    }
    Ok(())
}

fn load_and_persist_source_artifacts(
    current_dir: &Path,
    artifact_store: &ContractSourceArtifactStore,
) -> Result<Vec<VerifiedSourceRegistration>, ArtifactSyncError> {
    let mut registrations = Vec::new();
    for path in source_artifact_paths(current_dir)? {
        let bytes = fs::read(&path).map_err(|source| ArtifactSyncError::Io {
            operation: "read source artifact",
            path: path.clone(),
            source,
        })?;
        let artifact = artifact_store.publish(&bytes)?;
        registrations.push(VerifiedSourceRegistration {
            code_hash: artifact.code_hash,
            source: artifact.source,
        });
    }
    Ok(registrations)
}

fn source_artifact_paths(root: &Path) -> Result<Vec<PathBuf>, ArtifactSyncError> {
    let mut result = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory).map_err(|source| ArtifactSyncError::Io {
            operation: "read source artifact directory",
            path: directory.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ArtifactSyncError::Io {
                operation: "read source artifact directory entry",
                path: directory.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| ArtifactSyncError::Io {
                operation: "inspect source artifact path",
                path: entry.path(),
                source,
            })?;
            if file_type.is_dir() {
                directories.push(entry.path());
            } else if file_type.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".source.json")
            {
                result.push(entry.path());
            }
        }
    }
    result.sort();
    Ok(result)
}

fn project_fingerprint(workspace_root: &Path) -> Result<ProjectFingerprint, ArtifactSyncError> {
    let mut files = Vec::new();
    let mut directories = vec![workspace_root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory).map_err(|source| ArtifactSyncError::Io {
            operation: "read project directory",
            path: directory.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ArtifactSyncError::Io {
                operation: "read project directory entry",
                path: directory.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| ArtifactSyncError::Io {
                operation: "inspect project path",
                path: entry.path(),
                source,
            })?;
            if file_type.is_dir() {
                if !EXCLUDED_PROJECT_DIRECTORIES
                    .iter()
                    .any(|excluded| entry.file_name() == *excluded)
                {
                    directories.push(entry.path());
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            let is_manifest = path == workspace_root.join("Acton.toml");
            let is_tolk_source = path
                .extension()
                .is_some_and(|extension| extension == "tolk");
            if !is_manifest && !is_tolk_source {
                continue;
            }
            let content = fs::read(&path).map_err(|source| ArtifactSyncError::Io {
                operation: "read project file",
                path: path.clone(),
                source,
            })?;
            files.push(ProjectFileFingerprint {
                path: path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&path)
                    .to_path_buf(),
                content_hash: xxh3_128(&content),
            });
        }
    }
    files.sort();
    Ok(ProjectFingerprint(files))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        load_and_persist_source_artifacts, project_fingerprint, replace_current_source_dir,
    };
    use crate::{ContractSourceArtifactError, ContractSourceArtifactStore};

    #[test]
    fn source_artifacts_are_stored_by_bundle_hash_without_overwriting_history() {
        let temp = tempdir().expect("temp directory");
        let current = temp.path().join("current");
        let artifact_store =
            ContractSourceArtifactStore::from_history_root(temp.path().join("by-bundle"));
        fs::create_dir_all(&current).expect("current directory");
        let bundle_hash = "ab".repeat(32);
        let artifact = json!({
            "code_hash": "contract-code-hash",
            "verified": true,
            "bundle": {
                "source_bundle_hash": bundle_hash,
                "entrypoint": "contracts/counter.tolk"
            }
        });
        let artifact_path = current.join("counter.source.json");
        let artifact_bytes = serde_json::to_vec(&artifact).expect("serialize artifact");
        fs::write(&artifact_path, &artifact_bytes).expect("write artifact");

        let registrations =
            load_and_persist_source_artifacts(&current, &artifact_store).expect("persist artifact");
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].code_hash, "contract-code-hash");
        let immutable_path = artifact_store
            .root()
            .join(format!("{bundle_hash}.source.json"));
        assert_eq!(
            fs::read(&immutable_path).expect("read immutable artifact"),
            artifact_bytes
        );

        let changed_artifact = json!({
            "code_hash": "different-code-hash",
            "verified": true,
            "bundle": {
                "source_bundle_hash": bundle_hash
            }
        });
        fs::write(
            &artifact_path,
            serde_json::to_vec(&changed_artifact).expect("serialize changed artifact"),
        )
        .expect("write changed artifact");
        assert!(matches!(
            load_and_persist_source_artifacts(&current, &artifact_store),
            Err(super::ArtifactSyncError::Store(
                ContractSourceArtifactError::ImmutableArtifactCollision { .. }
            ))
        ));
    }

    #[test]
    fn project_fingerprint_tracks_manifest_and_tolk_sources_only() {
        let temp = tempdir().expect("temp directory");
        fs::write(temp.path().join("Acton.toml"), "[contracts]\n").expect("write manifest");
        let contracts = temp.path().join("contracts");
        fs::create_dir(&contracts).expect("contracts directory");
        fs::write(contracts.join("counter.tolk"), "fun main() {}\n").expect("write source");
        fs::write(temp.path().join("README.md"), "ignored\n").expect("write ignored file");
        let studio = temp.path().join(".studio");
        fs::create_dir(&studio).expect("studio directory");
        fs::write(studio.join("generated.tolk"), "ignored\n").expect("write generated source");

        let initial = project_fingerprint(temp.path()).expect("initial fingerprint");
        fs::write(studio.join("generated.tolk"), "still ignored\n")
            .expect("update generated source");
        assert_eq!(
            project_fingerprint(temp.path()).expect("ignored fingerprint"),
            initial
        );

        fs::write(contracts.join("counter.tolk"), "fun noop() {}\n").expect("update source");
        assert_ne!(
            project_fingerprint(temp.path()).expect("changed fingerprint"),
            initial
        );
    }

    #[test]
    fn replacing_current_artifacts_drops_removed_contracts() {
        let temp = tempdir().expect("temp directory");
        let current = temp.path().join("current");
        let staging = temp.path().join(".staging");
        let publish_lock = temp.path().join(".publish.lock");
        fs::create_dir(&current).expect("current directory");
        fs::create_dir(&staging).expect("staging directory");
        fs::write(current.join("removed.source.json"), "{}").expect("old artifact");
        fs::write(staging.join("current.source.json"), "{}").expect("new artifact");

        replace_current_source_dir(&staging, &current, &publish_lock)
            .expect("replace current artifacts");

        assert!(!current.join("removed.source.json").exists());
        assert!(current.join("current.source.json").exists());
        assert!(!staging.exists());
    }
}
