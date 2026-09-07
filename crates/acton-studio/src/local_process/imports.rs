//! Pins imported cells before hardfork admission and reconciles Contracts after
//! success. Pending manifests survive Studio restarts; recovery never replays an edit.

use super::{
    EnvironmentConfig, EnvironmentRuntimeError, ImportAccountsRequest, LocalEnvironment,
    LocalProcessRuntimeInner, full_localnet, localnet, persist_environment_definition,
    register_imported_contracts,
};
use crate::{AdminOperation, AdminRequest};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::Ordering,
};
use tokio::{fs, io::AsyncWriteExt, time::Instant};
use ton::ton_core::types::TonAddress;
use tracing::log;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub(super) struct PreparedImport {
    request: ImportAccountsRequest,
    edit: AdminRequest,
}

fn directory(runtime: &LocalProcessRuntimeInner, environment_id: &str) -> PathBuf {
    runtime
        .workspace_root
        .join(".studio/environments")
        .join(environment_id)
        .join("imports")
}

fn failure(path: &Path, error: impl std::fmt::Display) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "account_import_store_failed",
        message: format!(
            "Could not persist account import at {}: {error}",
            path.display()
        ),
    }
}

pub(super) async fn load(
    runtime: &LocalProcessRuntimeInner,
    environment_id: &str,
    request: &ImportAccountsRequest,
) -> Result<Option<PreparedImport>, EnvironmentRuntimeError> {
    Uuid::parse_str(&request.id).map_err(|_| EnvironmentRuntimeError::InvalidRequest {
        code: "account_import_invalid",
        message: "Use a UUID for the import operation ID".into(),
    })?;
    let directory = directory(runtime, environment_id);
    for suffix in ["pending.json", "json"] {
        let path = directory.join(format!("{}.{suffix}", request.id));
        let bytes = match fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(failure(&path, error)),
        };
        let record: PreparedImport =
            serde_json::from_slice(&bytes).map_err(|error| failure(&path, error))?;
        // Resolved cells are intentionally excluded from the public request identity.
        let identity = serde_json::to_value(request).map_err(|error| failure(&path, error))?;
        if serde_json::to_value(&record.request).map_err(|error| failure(&path, error))? != identity
        {
            return Err(EnvironmentRuntimeError::Conflict {
                code: "account_import_id_reused",
                message: "This operation ID belongs to a different import".into(),
            });
        }
        return Ok(Some(record));
    }
    Ok(None)
}

// The caller holds the environment lifecycle lock through persistence and admission.
pub(super) async fn start(
    runtime: &LocalProcessRuntimeInner,
    environment: &LocalEnvironment,
    environment_id: &str,
    request: ImportAccountsRequest,
) -> Result<AdminOperation, EnvironmentRuntimeError> {
    // Finish earlier registrations before admitting another import, so two
    // imports of the same address cannot restore their names out of order.
    reconcile(runtime, environment).await?;
    let client = full_localnet(environment)?.client().await?;
    let record = if let Some(record) = load(runtime, environment_id, &request).await? {
        record
    } else {
        let edits = request
            .accounts
            .iter()
            .map(|account| {
                let address = TonAddress::from_str(&account.address).map_err(|error| {
                    EnvironmentRuntimeError::InvalidRequest {
                        code: "full_ton_import_address_invalid",
                        message: error.to_string(),
                    }
                })?;
                let bytes = account
                    .shard_account_boc_hex
                    .as_deref()
                    .and_then(|boc| hex::decode(boc).ok())
                    .ok_or_else(|| EnvironmentRuntimeError::InvalidRequest {
                        code: "account_import_unresolved",
                        message: format!("Source state is missing for {}", account.address),
                    })?;
                Ok(serde_json::json!({
                    "address": address.to_hex(), "type": "replace",
                    "boc": base64::engine::general_purpose::STANDARD.encode(bytes),
                }))
            })
            .collect::<Result<Vec<_>, EnvironmentRuntimeError>>()?;
        let edit: AdminRequest = serde_json::from_value(serde_json::json!({
            "kind": "accounts", "id": request.id, "edits": edits,
        }))
        .map_err(|error| EnvironmentRuntimeError::InvalidRequest {
            code: "account_import_invalid",
            message: error.to_string(),
        })?;
        // Validate the entire batch before taking the network offline, including
        // cell decoding and the embedded account address, not just the user input.
        edit.validate().map_err(localnet::error)?;
        let record = PreparedImport { request, edit };
        let path =
            directory(runtime, environment_id).join(format!("{}.pending.json", record.request.id));
        save(&path, &record).await?;
        record
    };

    let started = Instant::now();
    log::info!(
        "operation=account_import id={} target={environment_id} accounts={} outcome=submitting",
        record.request.id,
        record.request.accounts.len()
    );
    let result = client
        .start_admin(&record.edit)
        .await
        .map_err(localnet::error);
    log::info!(
        "operation=account_import id={} target={environment_id} duration_ms={} outcome={}",
        record.request.id,
        started.elapsed().as_millis(),
        if result.is_ok() {
            "admitted"
        } else {
            "submission_failed"
        }
    );
    drop(client);
    if result
        .as_ref()
        .is_ok_and(|operation| operation.phase == "completed")
    {
        reconcile(runtime, environment).await?;
    }
    result
}

async fn save(path: &Path, record: &PreparedImport) -> Result<(), EnvironmentRuntimeError> {
    let parent = path
        .parent()
        .ok_or_else(|| failure(path, "Missing directory"))?;
    fs::create_dir_all(parent)
        .await
        .map_err(|error| failure(path, error))?;
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = async {
        let bytes = serde_json::to_vec(record).map_err(|error| failure(path, error))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
            .map_err(|error| failure(path, error))?;
        file.write_all(&bytes)
            .await
            .map_err(|error| failure(path, error))?;
        file.sync_all()
            .await
            .map_err(|error| failure(path, error))?;
        drop(file);
        fs::rename(&temporary, path)
            .await
            .map_err(|error| failure(path, error))
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result
}

// Called by the existing environment monitor and before reporting completion to
// the UI. No browser session is responsible for finishing registry persistence.
pub(super) async fn reconcile(
    runtime: &LocalProcessRuntimeInner,
    environment: &LocalEnvironment,
) -> Result<(), EnvironmentRuntimeError> {
    let id = environment.details.read().await.id.clone();
    let directory = directory(runtime, &id);
    let mut files = match fs::read_dir(&directory).await {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(failure(&directory, error)),
    };
    while let Some(file) = files
        .next_entry()
        .await
        .map_err(|error| failure(&directory, error))?
    {
        let path = file.path();
        if !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".pending.json"))
        {
            continue;
        }
        let bytes = fs::read(&path)
            .await
            .map_err(|error| failure(&path, error))?;
        let record: PreparedImport =
            serde_json::from_slice(&bytes).map_err(|error| failure(&path, error))?;
        let operation = full_localnet(environment)?
            .client()
            .await?
            .admin_operation_by_id(&record.request.id)
            .await
            .map_err(localnet::error)?;
        let Some(operation) = operation.filter(|operation| !operation.is_active()) else {
            continue;
        };

        if operation.phase == "completed" {
            let started = Instant::now();
            register_imported_contracts(&runtime.contract_registry, &id, &record.request.accounts)
                .await?;
            let mut details = environment.details.write().await;
            if let EnvironmentConfig::FullTonNetwork {
                imported_accounts, ..
            } = &mut details.config
            {
                for account in &record.request.accounts {
                    imported_accounts.retain(|previous| previous.address != account.address);
                    imported_accounts.push(account.clone());
                }
            }
            drop(details);
            persist_environment_definition(
                runtime,
                environment,
                environment.resume_on_startup.load(Ordering::Acquire),
            )
            .await?;
            log::info!(
                "operation=account_import id={} target={id} accounts={} duration_ms={} outcome=registered",
                record.request.id,
                record.request.accounts.len(),
                started.elapsed().as_millis()
            );
        }
        fs::rename(&path, directory.join(format!("{}.json", record.request.id)))
            .await
            .map_err(|error| failure(&path, error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
