//! Durable identity and initialization boundary for one validator-engine node.

use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::ton::tools::types::{KeyId, TonPublicKey};

use super::{NodeRuntime, write_json_atomic};

const NODE_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Persistent identities created as one validator-engine node is initialized.
///
/// The manifest is written after the database, service keys, and full-node ADNL
/// identity are complete. Its presence is therefore the only node-level commit
/// marker; intermediate engine files must never be used to infer completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct NodeManifest {
    schema_version: u32,
    node_name: String,
    console_public_key: TonPublicKey,
    liteserver_public_key: Option<TonPublicKey>,
    full_node_adnl: KeyId,
    validator_public_key: Option<TonPublicKey>,
    validator_adnl: Option<KeyId>,
}

impl NodeManifest {
    /// Builds the commit record from identities whose private material is already
    /// installed in the node-owned keyring.
    pub(crate) fn new(
        node_name: impl Into<String>,
        console_public_key: TonPublicKey,
        liteserver_public_key: Option<TonPublicKey>,
        full_node_adnl: KeyId,
        validator_public_key: Option<TonPublicKey>,
        validator_adnl: Option<KeyId>,
    ) -> Self {
        Self {
            schema_version: NODE_MANIFEST_SCHEMA_VERSION,
            node_name: node_name.into(),
            console_public_key,
            liteserver_public_key,
            full_node_adnl,
            validator_public_key,
            validator_adnl,
        }
    }

    /// Loads a completed node and rejects a marker belonging to another configured name.
    pub(crate) fn load(path: &Path, expected_node: &str) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read node manifest {}", path.display()))?;
        let manifest: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid node manifest {}", path.display()))?;

        ensure!(
            manifest.schema_version == NODE_MANIFEST_SCHEMA_VERSION,
            "unsupported node manifest schema {} for `{expected_node}`",
            manifest.schema_version
        );
        ensure!(
            manifest.node_name == expected_node,
            "node manifest {} belongs to `{}`, expected `{expected_node}`",
            path.display(),
            manifest.node_name
        );

        Ok(manifest)
    }

    /// Atomically publishes the node only after all initialization work succeeds.
    pub(crate) fn save_atomic(&self, path: &Path) -> Result<()> {
        write_json_atomic(path, self)
            .with_context(|| format!("failed to save node manifest {}", path.display()))
    }

    /// Returns the durable liteserver identity advertised by this node.
    ///
    /// Nodes may omit the identity when liteserver mode is disabled, so callers
    /// that publish a connection endpoint must handle the absent key explicitly.
    pub(crate) fn liteserver_public_key(&self) -> Option<TonPublicKey> {
        self.liteserver_public_key
    }

    /// Applies durable identity to operational state from an earlier invocation.
    ///
    /// Election history and synchronization observations are not node initialization
    /// artifacts, so they survive a restart. Manifest-owned service and full-node
    /// identities always replace runtime copies, while the genesis validator identity
    /// is retained as history without overwriting a newer election key.
    pub(crate) fn runtime(&self, mut runtime: NodeRuntime) -> NodeRuntime {
        runtime.initialized = true;
        runtime.running = false;
        runtime.pid = None;
        runtime.status = "initialized".to_owned();
        runtime.last_error = None;
        runtime.console_public_key = Some(self.console_public_key);
        runtime.liteserver_public_key = self.liteserver_public_key;
        runtime.full_node_adnl = Some(self.full_node_adnl);

        if let Some(public_key) = self.validator_public_key {
            runtime.remember_validator_public_key(public_key);
            runtime.validator_public_key.get_or_insert(public_key);
        }
        if runtime.validator_adnl.is_none() {
            runtime.validator_adnl = self.validator_adnl;
        }

        runtime
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    #[test]
    fn committed_manifest_reconstructs_durable_runtime_identity() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let path = directory.path().join("node-manifest.json");
        NodeManifest::new(
            "node2",
            TonPublicKey::from_bytes([1; 32]),
            Some(TonPublicKey::from_bytes([2; 32])),
            KeyId::from_bytes([3; 32]),
            Some(TonPublicKey::from_bytes([4; 32])),
            Some(KeyId::from_bytes([5; 32])),
        )
        .save_atomic(&path)
        .unwrap();

        let runtime = NodeManifest::load(&path, "node2")
            .unwrap()
            .runtime(NodeRuntime::default());
        let actual = serde_json::json!({
            "initialized": runtime.initialized,
            "status": runtime.status,
            "console_public_key": runtime.console_public_key,
            "liteserver_public_key": runtime.liteserver_public_key,
            "full_node_adnl": runtime.full_node_adnl,
            "validator_public_key": runtime.validator_public_key,
            "validator_public_keys": runtime.validator_public_keys,
            "validator_adnl": runtime.validator_adnl,
        });

        expect![[r#"
            {
              "console_public_key": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
              "full_node_adnl": "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
              "initialized": true,
              "liteserver_public_key": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=",
              "status": "initialized",
              "validator_adnl": "BQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQU=",
              "validator_public_key": "BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ=",
              "validator_public_keys": [
                "BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ="
              ]
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());
    }
}
