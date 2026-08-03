use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{EnvironmentConfig, EnvironmentRuntimeError};

const ENVIRONMENT_DIRECTORY_PREFIX: &str = "environment-";
const ENVIRONMENT_FILE_NAME: &str = "environment.json";
const ENVIRONMENT_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) struct StoredEnvironment {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) config: EnvironmentConfig,
    pub(crate) resume_on_startup: bool,
}

#[derive(Debug)]
pub(crate) struct LoadedEnvironments {
    pub(crate) records: Vec<StoredEnvironment>,
    pub(crate) next_id: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EnvironmentFile {
    format_version: u32,
    id: String,
    name: String,
    config: EnvironmentConfig,
    resume_on_startup: bool,
}

pub(crate) async fn load_environments(
    workspace_root: &Path,
) -> Result<LoadedEnvironments, EnvironmentRuntimeError> {
    let root = environments_root(workspace_root);
    let mut entries = match fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(LoadedEnvironments {
                records: Vec::new(),
                next_id: 1,
            });
        }
        Err(error) => return Err(read_error("list", &root, error)),
    };

    let mut records = Vec::new();
    let mut highest_id = 0;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| read_error("list", &root, error))?
    {
        let Some(number) = environment_directory_number(&entry.file_name())? else {
            continue;
        };
        highest_id = highest_id.max(number);

        let file_type = entry
            .file_type()
            .await
            .map_err(|error| read_error("inspect", &entry.path(), error))?;
        if !file_type.is_dir() {
            return Err(invalid_directory(format!(
                "{} must be a directory",
                entry.path().display()
            )));
        }

        let metadata_path = entry.path().join(ENVIRONMENT_FILE_NAME);
        let bytes = match fs::read(&metadata_path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                tracing::warn!(
                    "Ignoring incomplete Studio environment without metadata at {}",
                    metadata_path.display()
                );
                continue;
            }
            Err(error) => return Err(read_error("read", &metadata_path, error)),
        };
        let stored: EnvironmentFile = serde_json::from_slice(&bytes).map_err(|error| {
            invalid_metadata(format!(
                "Failed to parse {}: {error}",
                metadata_path.display()
            ))
        })?;
        let expected_id = environment_id(number);
        if stored.id != expected_id {
            return Err(invalid_metadata(format!(
                "{} belongs to {expected_id}, but contains environment id {}",
                metadata_path.display(),
                stored.id
            )));
        }
        if stored.format_version != ENVIRONMENT_FORMAT_VERSION {
            return Err(EnvironmentRuntimeError::Internal {
                code: "environment_store_unsupported_version",
                message: format!(
                    "{} uses environment format version {}; supported version is {}",
                    metadata_path.display(),
                    stored.format_version,
                    ENVIRONMENT_FORMAT_VERSION
                ),
            });
        }

        records.push((
            number,
            StoredEnvironment {
                id: stored.id,
                name: stored.name,
                config: stored.config,
                resume_on_startup: stored.resume_on_startup,
            },
        ));
    }

    records.sort_by_key(|(number, _)| *number);
    let next_id = highest_id
        .checked_add(1)
        .ok_or_else(|| EnvironmentRuntimeError::Internal {
            code: "environment_store_id_exhausted",
            message: "Studio cannot allocate another environment id".to_owned(),
        })?;

    Ok(LoadedEnvironments {
        records: records.into_iter().map(|(_, record)| record).collect(),
        next_id,
    })
}

pub(crate) async fn persist_environment(
    workspace_root: &Path,
    record: &StoredEnvironment,
) -> Result<(), EnvironmentRuntimeError> {
    let number = parse_environment_id(&record.id).map_err(|message| {
        EnvironmentRuntimeError::InvalidRequest {
            code: "invalid_environment_id",
            message,
        }
    })?;
    let directory = environments_root(workspace_root).join(environment_id(number));
    fs::create_dir_all(&directory)
        .await
        .map_err(|error| write_error("create", &directory, error))?;

    let metadata = EnvironmentFile {
        format_version: ENVIRONMENT_FORMAT_VERSION,
        id: record.id.clone(),
        name: record.name.clone(),
        config: record.config.clone(),
        resume_on_startup: record.resume_on_startup,
    };
    let mut bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
        EnvironmentRuntimeError::Internal {
            code: "environment_store_serialization_failed",
            message: format!(
                "Failed to serialize metadata for environment {}: {error}",
                record.id
            ),
        }
    })?;
    bytes.push(b'\n');

    let metadata_path = directory.join(ENVIRONMENT_FILE_NAME);
    let temporary_path = directory.join(format!(".{ENVIRONMENT_FILE_NAME}.{}", Uuid::new_v4()));
    let write_result = async {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
            .await
            .map_err(|error| write_error("create", &temporary_path, error))?;
        file.write_all(&bytes)
            .await
            .map_err(|error| write_error("write", &temporary_path, error))?;
        file.sync_all()
            .await
            .map_err(|error| write_error("sync", &temporary_path, error))?;
        drop(file);
        fs::rename(&temporary_path, &metadata_path)
            .await
            .map_err(|error| write_error("replace", &metadata_path, error))
    }
    .await;

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path).await;
    }
    write_result
}

fn environments_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".studio").join("environments")
}

fn environment_directory_number(file_name: &OsStr) -> Result<Option<u64>, EnvironmentRuntimeError> {
    let name = file_name.to_string_lossy();
    if !name.starts_with(ENVIRONMENT_DIRECTORY_PREFIX) {
        return Ok(None);
    }
    parse_environment_id(&name)
        .map(Some)
        .map_err(invalid_directory)
}

fn parse_environment_id(id: &str) -> Result<u64, String> {
    let Some(number) = id.strip_prefix(ENVIRONMENT_DIRECTORY_PREFIX) else {
        return Err(format!(
            "Environment id {id:?} must use the {ENVIRONMENT_DIRECTORY_PREFIX}<number> format"
        ));
    };
    let number = number
        .parse::<u64>()
        .map_err(|_| format!("Environment id {id:?} has an invalid numeric suffix"))?;
    if number == 0 || environment_id(number) != id {
        return Err(format!(
            "Environment id {id:?} is not in canonical {ENVIRONMENT_DIRECTORY_PREFIX}<number> format"
        ));
    }
    Ok(number)
}

fn environment_id(number: u64) -> String {
    format!("{ENVIRONMENT_DIRECTORY_PREFIX}{number}")
}

const fn invalid_directory(message: String) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "environment_store_invalid_directory",
        message,
    }
}

const fn invalid_metadata(message: String) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "environment_store_invalid_metadata",
        message,
    }
}

fn read_error(action: &str, path: &Path, error: std::io::Error) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "environment_store_read_failed",
        message: format!(
            "Failed to {action} persisted environments at {}: {error}",
            path.display()
        ),
    }
}

fn write_error(action: &str, path: &Path, error: std::io::Error) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "environment_store_write_failed",
        message: format!(
            "Failed to {action} environment metadata at {}: {error}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    #[tokio::test]
    async fn environment_metadata_round_trips() {
        let workspace = tempfile::tempdir_in("/tmp").unwrap();
        let record = localnet_record("environment-2", "Forked network", false);

        persist_environment(workspace.path(), &record)
            .await
            .unwrap();
        let file = fs::read_to_string(
            workspace
                .path()
                .join(".studio/environments/environment-2/environment.json"),
        )
        .await
        .unwrap();
        let loaded = load_environments(workspace.path()).await.unwrap();

        expect![[r#"FILE
{
  "formatVersion": 1,
  "id": "environment-2",
  "name": "Forked network",
  "config": {
    "kind": "actonLocalnet",
    "port": 5401,
    "forkNetwork": "testnet",
    "forkBlockNumber": 12345,
    "accounts": [
      "deployer"
    ],
    "rateLimit": 120,
    "responseDelayMs": 15,
    "blockIntervalMs": 1000,
    "noMining": false,
    "mineEmptyBlocks": true
  },
  "resumeOnStartup": false
}
LOADED
environment-2 | Forked network | resume=false | {"kind":"actonLocalnet","port":5401,"forkNetwork":"testnet","forkBlockNumber":12345,"accounts":["deployer"],"rateLimit":120,"responseDelayMs":15,"blockIntervalMs":1000,"noMining":false,"mineEmptyBlocks":true}
next_id=3"#]]
        .assert_eq(&format!(
            "FILE\n{}LOADED\n{}\nnext_id={}",
            file,
            describe_records(&loaded.records),
            loaded.next_id
        ));
    }

    #[tokio::test]
    async fn updated_name_and_restart_flag_persist() {
        let workspace = tempfile::tempdir_in("/tmp").unwrap();
        persist_environment(
            workspace.path(),
            &localnet_record("environment-1", "Original name", false),
        )
        .await
        .unwrap();
        persist_environment(
            workspace.path(),
            &localnet_record("environment-1", "Persistent network", true),
        )
        .await
        .unwrap();

        let loaded = load_environments(workspace.path()).await.unwrap();
        let mut directory_entries =
            fs::read_dir(workspace.path().join(".studio/environments/environment-1"))
                .await
                .unwrap();
        let mut names = Vec::new();
        while let Some(entry) = directory_entries.next_entry().await.unwrap() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();

        expect![[r#"environment-1 | Persistent network | resume=true | {"kind":"actonLocalnet","port":5401,"forkNetwork":"testnet","forkBlockNumber":12345,"accounts":["deployer"],"rateLimit":120,"responseDelayMs":15,"blockIntervalMs":1000,"noMining":false,"mineEmptyBlocks":true}
files=environment.json"#]]
        .assert_eq(&format!(
            "{}\nfiles={}",
            describe_records(&loaded.records),
            names.join(",")
        ));
    }

    #[tokio::test]
    async fn environment_directory_without_metadata_is_ignored_and_reserves_its_id() {
        let workspace = tempfile::tempdir_in("/tmp").unwrap();
        fs::create_dir_all(workspace.path().join(".studio/environments/environment-7"))
            .await
            .unwrap();

        let loaded = load_environments(workspace.path()).await.unwrap();

        expect![[r"records=0
next_id=8
incomplete_directory_preserved=true"]]
        .assert_eq(&format!(
            "records={}\nnext_id={}\nincomplete_directory_preserved={}",
            loaded.records.len(),
            loaded.next_id,
            workspace
                .path()
                .join(".studio/environments/environment-7")
                .is_dir()
        ));
    }

    #[tokio::test]
    async fn invalid_directories_ids_and_versions_are_rejected() {
        let invalid_directory_workspace = tempfile::tempdir_in("/tmp").unwrap();
        fs::create_dir_all(
            invalid_directory_workspace
                .path()
                .join(".studio/environments/environment-01"),
        )
        .await
        .unwrap();
        let invalid_directory = load_environments(invalid_directory_workspace.path())
            .await
            .unwrap_err();

        let mismatched_id_workspace = tempfile::tempdir_in("/tmp").unwrap();
        write_metadata(
            mismatched_id_workspace.path(),
            "environment-2",
            1,
            "environment-3",
        )
        .await;
        let mismatched_id = load_environments(mismatched_id_workspace.path())
            .await
            .unwrap_err();

        let unsupported_version_workspace = tempfile::tempdir_in("/tmp").unwrap();
        write_metadata(
            unsupported_version_workspace.path(),
            "environment-4",
            2,
            "environment-4",
        )
        .await;
        let unsupported_version = load_environments(unsupported_version_workspace.path())
            .await
            .unwrap_err();

        expect![[r#"environment_store_invalid_directory: Environment id "environment-01" is not in canonical environment-<number> format
environment_store_invalid_metadata: <workspace>/.studio/environments/environment-2/environment.json belongs to environment-2, but contains environment id environment-3
environment_store_unsupported_version: <workspace>/.studio/environments/environment-4/environment.json uses environment format version 2; supported version is 1"#]]
        .assert_eq(&format!(
            "{}\n{}\n{}",
            describe_error(invalid_directory, invalid_directory_workspace.path()),
            describe_error(mismatched_id, mismatched_id_workspace.path()),
            describe_error(unsupported_version, unsupported_version_workspace.path())
        ));
    }

    fn localnet_record(id: &str, name: &str, resume_on_startup: bool) -> StoredEnvironment {
        StoredEnvironment {
            id: id.to_owned(),
            name: name.to_owned(),
            config: EnvironmentConfig::ActonLocalnet {
                port: 5401,
                fork_network: Some("testnet".to_owned()),
                fork_block_number: Some(12_345),
                accounts: vec!["deployer".to_owned()],
                rate_limit: Some(120),
                response_delay_ms: Some(15),
                block_interval_ms: Some(1_000),
                no_mining: false,
                mine_empty_blocks: true,
            },
            resume_on_startup,
        }
    }

    fn describe_records(records: &[StoredEnvironment]) -> String {
        records
            .iter()
            .map(|record| {
                format!(
                    "{} | {} | resume={} | {}",
                    record.id,
                    record.name,
                    record.resume_on_startup,
                    serde_json::to_string(&record.config).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn describe_error(error: EnvironmentRuntimeError, workspace_root: &Path) -> String {
        let (code, message) = match error {
            EnvironmentRuntimeError::InvalidRequest { code, message }
            | EnvironmentRuntimeError::Conflict { code, message }
            | EnvironmentRuntimeError::Internal { code, message } => (code, message),
            EnvironmentRuntimeError::NotFound { environment_id } => {
                ("environment_not_found", environment_id)
            }
        };
        format!(
            "{code}: {}",
            message.replace(&workspace_root.display().to_string(), "<workspace>")
        )
    }

    async fn write_metadata(
        workspace_root: &Path,
        directory_id: &str,
        format_version: u32,
        stored_id: &str,
    ) {
        let directory = environments_root(workspace_root).join(directory_id);
        fs::create_dir_all(&directory).await.unwrap();
        let metadata = EnvironmentFile {
            format_version,
            id: stored_id.to_owned(),
            name: "Stored".to_owned(),
            config: EnvironmentConfig::FullTonNetwork {
                api_v2_port: 8081,
                api_v3_port: 8082,
                admin_port: 8083,
                validators: 1,
            },
            resume_on_startup: false,
        };
        fs::write(
            directory.join(ENVIRONMENT_FILE_NAME),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .await
        .unwrap();
    }
}
