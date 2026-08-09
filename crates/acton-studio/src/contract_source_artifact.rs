use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use uuid::Uuid;

pub const CONTRACT_SOURCE_HISTORY_PATH: &str = ".studio/artifacts/contracts/by-bundle";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractSourceArtifact {
    pub code_hash: String,
    pub bundle_hash: String,
    pub source: Value,
}

#[derive(Clone, Debug)]
pub struct ContractSourceArtifactStore {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractSourceArtifactError {
    #[error("Failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse source artifact {path}: {source}")]
    InvalidJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Source artifact {path} does not contain a valid {field}")]
    InvalidArtifact { path: PathBuf, field: &'static str },
    #[error("Source bundle {bundle_hash} already exists with different content at {path}")]
    ImmutableArtifactCollision { bundle_hash: String, path: PathBuf },
}

impl ContractSourceArtifactStore {
    #[must_use]
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        Self {
            root: project_root.as_ref().join(CONTRACT_SOURCE_HISTORY_PATH),
        }
    }

    #[must_use]
    pub fn from_history_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn publish(
        &self,
        bytes: &[u8],
    ) -> Result<ContractSourceArtifact, ContractSourceArtifactError> {
        fs::create_dir_all(&self.root).map_err(|source| ContractSourceArtifactError::Io {
            operation: "create immutable source artifact directory",
            path: self.root.clone(),
            source,
        })?;

        let artifact = parse_source_artifact(
            bytes,
            &self.root.join("<incoming-contract-source-artifact>"),
        )?;
        let immutable_path = self
            .root
            .join(format!("{}.source.json", artifact.bundle_hash));
        if immutable_path.exists() {
            verify_immutable_source_artifact(&immutable_path, &artifact.bundle_hash, bytes)?;
            return Ok(artifact);
        }

        let temporary_path =
            self.root
                .join(format!(".{}.{}.tmp", artifact.bundle_hash, Uuid::new_v4()));
        let mut temporary_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|source| ContractSourceArtifactError::Io {
                operation: "create immutable source artifact temporary file",
                path: temporary_path.clone(),
                source,
            })?;
        if let Err(source) = temporary_file
            .write_all(bytes)
            .and_then(|()| temporary_file.sync_all())
        {
            drop(temporary_file);
            let _ = fs::remove_file(&temporary_path);
            return Err(ContractSourceArtifactError::Io {
                operation: "write immutable source artifact temporary file",
                path: temporary_path,
                source,
            });
        }
        drop(temporary_file);

        match fs::hard_link(&temporary_path, &immutable_path) {
            Ok(()) => {
                if let Err(error) = fs::remove_file(&temporary_path) {
                    tracing::warn!(
                        path = %temporary_path.display(),
                        %error,
                        "Failed to clean immutable source artifact temporary file"
                    );
                }
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary_path);
                verify_immutable_source_artifact(&immutable_path, &artifact.bundle_hash, bytes)?;
            }
            Err(source) => {
                let _ = fs::remove_file(&temporary_path);
                return Err(ContractSourceArtifactError::Io {
                    operation: "publish immutable source artifact",
                    path: immutable_path,
                    source,
                });
            }
        }

        Ok(artifact)
    }

    pub fn load_all(&self) -> Result<Vec<ContractSourceArtifact>, ContractSourceArtifactError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut paths = fs::read_dir(&self.root)
            .map_err(|source| ContractSourceArtifactError::Io {
                operation: "read immutable source artifact directory",
                path: self.root.clone(),
                source,
            })?
            .filter_map(|entry| match entry {
                Ok(entry)
                    if entry.file_type().is_ok_and(|file_type| file_type.is_file())
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".source.json") =>
                {
                    Some(Ok(entry.path()))
                }
                Ok(_) => None,
                Err(source) => Some(Err(ContractSourceArtifactError::Io {
                    operation: "read immutable source artifact directory entry",
                    path: self.root.clone(),
                    source,
                })),
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();

        paths
            .into_iter()
            .map(|path| {
                let bytes = fs::read(&path).map_err(|source| ContractSourceArtifactError::Io {
                    operation: "read immutable source artifact",
                    path: path.clone(),
                    source,
                })?;
                parse_source_artifact(&bytes, &path)
            })
            .collect()
    }
}

fn parse_source_artifact(
    bytes: &[u8],
    path: &Path,
) -> Result<ContractSourceArtifact, ContractSourceArtifactError> {
    let source: Value = serde_json::from_slice(bytes).map_err(|source| {
        ContractSourceArtifactError::InvalidJson {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let code_hash = source
        .get("code_hash")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ContractSourceArtifactError::InvalidArtifact {
            path: path.to_path_buf(),
            field: "code_hash",
        })?
        .to_owned();
    let bundle_hash = source
        .pointer("/bundle/source_bundle_hash")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| ContractSourceArtifactError::InvalidArtifact {
            path: path.to_path_buf(),
            field: "bundle.source_bundle_hash",
        })?
        .to_ascii_lowercase();

    Ok(ContractSourceArtifact {
        code_hash,
        bundle_hash,
        source,
    })
}

fn verify_immutable_source_artifact(
    immutable_path: &Path,
    bundle_hash: &str,
    expected: &[u8],
) -> Result<(), ContractSourceArtifactError> {
    let existing = fs::read(immutable_path).map_err(|source| ContractSourceArtifactError::Io {
        operation: "read immutable source artifact",
        path: immutable_path.to_path_buf(),
        source,
    })?;
    if existing != expected {
        return Err(ContractSourceArtifactError::ImmutableArtifactCollision {
            bundle_hash: bundle_hash.to_owned(),
            path: immutable_path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{ContractSourceArtifactError, ContractSourceArtifactStore};

    #[test]
    fn history_survives_new_store_instances_and_keeps_every_bundle() {
        let temp = tempdir().expect("temp directory");
        let first_store = ContractSourceArtifactStore::for_project(temp.path());
        let first = json!({
            "code_hash": "first-code",
            "bundle": {"source_bundle_hash": "ab".repeat(32)}
        });
        let second = json!({
            "code_hash": "second-code",
            "bundle": {"source_bundle_hash": "cd".repeat(32)}
        });

        first_store
            .publish(&serde_json::to_vec(&first).expect("first artifact"))
            .expect("publish first artifact");
        first_store
            .publish(&serde_json::to_vec(&second).expect("second artifact"))
            .expect("publish second artifact");

        let restored = ContractSourceArtifactStore::for_project(temp.path())
            .load_all()
            .expect("restore artifact history");
        assert_eq!(
            restored
                .iter()
                .map(|artifact| artifact.code_hash.as_str())
                .collect::<Vec<_>>(),
            vec!["first-code", "second-code"]
        );
    }

    #[test]
    fn immutable_artifact_publication_is_atomic_across_publishers() {
        let temp = tempdir().expect("temp directory");
        let store = Arc::new(ContractSourceArtifactStore::for_project(temp.path()));
        let bundle_hash = "ef".repeat(32);
        let bytes = Arc::new(
            serde_json::to_vec(&json!({
                "code_hash": "contract-code",
                "bundle": {"source_bundle_hash": bundle_hash}
            }))
            .expect("artifact"),
        );
        let barrier = Arc::new(Barrier::new(2));

        let publishers = (0..2)
            .map(|_| {
                let store = Arc::clone(&store);
                let bytes = Arc::clone(&bytes);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.publish(&bytes)
                })
            })
            .collect::<Vec<_>>();

        for publisher in publishers {
            publisher
                .join()
                .expect("publisher thread")
                .expect("publish artifact");
        }
        assert_eq!(
            fs::read(store.root().join(format!("{bundle_hash}.source.json")))
                .expect("published artifact"),
            *bytes
        );
    }

    #[test]
    fn bundle_hash_collision_is_rejected() {
        let temp = tempdir().expect("temp directory");
        let store = ContractSourceArtifactStore::for_project(temp.path());
        let bundle_hash = "01".repeat(32);
        let first = serde_json::to_vec(&json!({
            "code_hash": "first-code",
            "bundle": {"source_bundle_hash": bundle_hash}
        }))
        .expect("first artifact");
        let second = serde_json::to_vec(&json!({
            "code_hash": "second-code",
            "bundle": {"source_bundle_hash": bundle_hash}
        }))
        .expect("second artifact");

        store.publish(&first).expect("publish first artifact");
        assert!(matches!(
            store.publish(&second),
            Err(ContractSourceArtifactError::ImmutableArtifactCollision { .. })
        ));
    }
}
