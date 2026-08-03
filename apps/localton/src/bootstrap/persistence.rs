//! Validation and exclusive access for persistent local-network state.
//!
//! A state directory may be reused across launcher runs. Before starting any
//! process, the launcher locks that directory and verifies that its manifest,
//! global config, databases, and control keys form a complete network state.

use std::{fs::File, path::Path};

use anyhow::{Context, Result, ensure};
use fs2::FileExt;

use crate::{
    storage::{Layout, Manifest},
    ton::accounts::ImportedAccount,
};

/// Locks a state directory for the lifetime of one launcher process.
///
/// Two launchers sharing databases and fixed ports would corrupt runtime state
/// and compete for the same sockets. The returned open file owns the advisory
/// lock; dropping it releases the directory for the next invocation.
pub(super) fn acquire_lock(path: &Path) -> Result<File> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open state lock {}", path.display()))?;
    file.try_lock_exclusive().with_context(|| {
        format!(
            "another localton process is already using {}",
            path.parent().unwrap_or(path).display()
        )
    })?;
    Ok(file)
}

/// Checks the minimum artifact set needed to restart an existing network.
///
/// The manifest alone is insufficient: validator and DHT databases, global
/// config, and both console credentials must exist. The global-config path is
/// also checked against this state directory to prevent mixing two networks.
pub(super) fn validate_persisted_state(layout: &Layout, manifest: &Manifest) -> Result<()> {
    for path in [
        &layout.global_config,
        &layout.validator_db.join("config.json"),
        &layout.dht_db.join("config.json"),
        &layout.certs.join("client"),
        &layout.certs.join("server.pub"),
    ] {
        ensure!(
            path.exists(),
            "persistent state is incomplete: {} is missing",
            path.display()
        );
    }
    ensure!(
        manifest.global_config == layout.global_config,
        "manifest points to an unexpected global config"
    );
    Ok(())
}

/// Prevents CLI input from implying a change to an immutable zerostate.
///
/// Imported accounts can only be inserted while creating the network. On reuse,
/// supplied descriptors must exactly match the manifest; omitting the option is
/// valid because the persisted manifest remains authoritative.
pub(super) fn validate_requested_imported_accounts(
    manifest: &Manifest,
    requested: &[ImportedAccount],
) -> Result<()> {
    if requested.is_empty() {
        return Ok(());
    }
    let requested = requested
        .iter()
        .map(|account| account.descriptor.clone())
        .collect::<Vec<_>>();
    ensure!(
        manifest.imported_accounts == requested,
        "--add-account values do not match this persistent zerostate; \
         omit them to reuse the existing network or select a new --state-dir"
    );
    Ok(())
}
