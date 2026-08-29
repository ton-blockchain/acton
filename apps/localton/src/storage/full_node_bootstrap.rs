use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{Layout, Manifest, TON_RELEASE, write_json_atomic};

pub const FULL_NODE_BOOTSTRAP_SCHEMA_VERSION: u32 = 1;

/// Public data required to initialize an independent full node
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FullNodeBootstrap {
    /// Version of this bootstrap document
    pub schema_version: u32,
    /// Official TON release used by the network
    pub ton_release: String,
    /// Global config containing the zerostate and DHT entry points
    pub global_config: Value,
    /// Zerostate files required before the node can synchronize over ADNL
    pub static_states: Vec<StaticStateFile>,
}

/// One public zerostate file encoded for JSON transport
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaticStateFile {
    /// Hash-derived filename expected by validator-engine
    pub name: String,
    /// Base64-encoded BoC contents
    pub content_base64: String,
}

impl FullNodeBootstrap {
    pub fn from_layout(layout: &Layout) -> Result<Self> {
        let manifest = Manifest::load(&layout.manifest)?;
        let global_config = serde_json::from_slice(
            &fs::read(&layout.global_config)
                .with_context(|| format!("failed to read {}", layout.global_config.display()))?,
        )
        .context("global config is invalid JSON")?;
        let static_dir = layout.validator_db.join("static");
        let mut paths = fs::read_dir(&static_dir)
            .with_context(|| format!("failed to read {}", static_dir.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();
        let static_states = paths
            .into_iter()
            .filter(|path| path.is_file())
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("static state filename is not valid UTF-8")?
                    .to_owned();
                let content = fs::read(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                Ok(StaticStateFile {
                    name,
                    content_base64: BASE64.encode(content),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let bootstrap = Self {
            schema_version: FULL_NODE_BOOTSTRAP_SCHEMA_VERSION,
            ton_release: manifest.ton_release,
            global_config,
            static_states,
        };
        bootstrap.validate()?;
        Ok(bootstrap)
    }

    pub fn install(&self, layout: &Layout) -> Result<()> {
        self.validate()?;
        layout.create_dirs()?;
        write_json_atomic(&layout.global_config, &self.global_config)?;
        let static_dir = layout.validator_db.join("static");
        fs::create_dir_all(&static_dir)
            .with_context(|| format!("failed to create {}", static_dir.display()))?;
        for state in &self.static_states {
            let path = static_dir.join(&state.name);
            let content = BASE64
                .decode(&state.content_base64)
                .with_context(|| format!("static state {} is not valid base64", state.name))?;
            write_atomic(&path, &content)?;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == FULL_NODE_BOOTSTRAP_SCHEMA_VERSION,
            "unsupported full-node bootstrap schema {}",
            self.schema_version
        );
        ensure!(
            self.ton_release == TON_RELEASE,
            "bootstrap uses TON {}, expected {}",
            self.ton_release,
            TON_RELEASE
        );
        ensure!(
            self.global_config
                .pointer("/validator/zero_state")
                .is_some(),
            "bootstrap global config has no validator zerostate"
        );
        ensure!(
            !self.static_states.is_empty(),
            "bootstrap contains no static states"
        );
        let mut names = BTreeSet::new();
        for state in &self.static_states {
            ensure!(
                safe_filename(&state.name),
                "invalid static state filename `{}`",
                state.name
            );
            ensure!(
                names.insert(&state.name),
                "duplicate static state `{}`",
                state.name
            );
            BASE64
                .decode(&state.content_base64)
                .with_context(|| format!("static state {} is not valid base64", state.name))?;
        }
        Ok(())
    }
}

fn safe_filename(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .file_name()
            .is_some_and(|filename| filename == name)
        && Path::new(name).components().count() == 1
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = path.with_extension("bootstrap.tmp");
    fs::write(&temporary, contents)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("failed to replace {}", path.display()))
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use serde_json::json;

    use super::*;
    use crate::storage::SCHEMA_VERSION;

    #[test]
    fn rejects_unsafe_or_incompatible_bootstrap_documents() {
        let document = FullNodeBootstrap {
            schema_version: FULL_NODE_BOOTSTRAP_SCHEMA_VERSION,
            ton_release: TON_RELEASE.to_owned(),
            global_config: json!({"validator": {"zero_state": {}}}),
            static_states: vec![StaticStateFile {
                name: "../keyring/secret".to_owned(),
                content_base64: BASE64.encode(b"state"),
            }],
        };
        let mut wrong_release = document.clone();
        wrong_release.ton_release = "v0".to_owned();
        let actual = format!(
            "unsafe filename: {}\nwrong release: {}",
            document.validate().unwrap_err(),
            wrong_release.validate().unwrap_err()
        );

        expect![[
            r#"unsafe filename: invalid static state filename `../keyring/secret`
wrong release: bootstrap uses TON v0, expected v2026.06"#
        ]]
        .assert_eq(&actual);
    }

    #[test]
    fn installs_only_public_network_bootstrap_data() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(root.path().join("follower"));
        let document = FullNodeBootstrap {
            schema_version: FULL_NODE_BOOTSTRAP_SCHEMA_VERSION,
            ton_release: TON_RELEASE.to_owned(),
            global_config: json!({"validator": {"zero_state": {"file_hash": "hash"}}}),
            static_states: vec![StaticStateFile {
                name: "state-hash".to_owned(),
                content_base64: BASE64.encode(b"public zerostate"),
            }],
        };

        document.install(&layout).unwrap();

        let actual = json!({
            "global_config": serde_json::from_slice::<Value>(&fs::read(&layout.global_config).unwrap()).unwrap(),
            "static_state": String::from_utf8(fs::read(layout.validator_db.join("static/state-hash")).unwrap()).unwrap(),
            "keyring_created": layout.validator_keyring.exists(),
        });
        expect![[r#"
            {
              "global_config": {
                "validator": {
                  "zero_state": {
                    "file_hash": "hash"
                  }
                }
              },
              "keyring_created": true,
              "static_state": "public zerostate"
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());
    }

    #[test]
    fn export_excludes_primary_private_state() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(root.path().join("primary"));
        layout.create_dirs().unwrap();
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            ton_release: TON_RELEASE.to_owned(),
            ton_bin_dir: None,
            validator_id_hex: "validator-public-id".to_owned(),
            validator_id_base64: "validator-public-id-base64".to_owned(),
            liteserver_public_key: "liteserver-public-key".to_owned(),
            global_config: layout.global_config.clone(),
            imported_accounts: Vec::new(),
        };
        manifest.save_atomic(&layout.manifest).unwrap();
        write_json_atomic(
            &layout.global_config,
            &json!({"validator": {"zero_state": {"file_hash": "network"}}}),
        )
        .unwrap();
        let static_dir = layout.validator_db.join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::write(static_dir.join("state-hash"), b"public zerostate").unwrap();
        fs::write(layout.validator_keyring.join("private-key"), b"secret").unwrap();

        let document = FullNodeBootstrap::from_layout(&layout).unwrap();
        let serialized = serde_json::to_string_pretty(&document).unwrap();
        let actual = json!({
            "top_level_fields": serde_json::from_str::<Value>(&serialized)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            "static_state_names": document
                .static_states
                .iter()
                .map(|state| &state.name)
                .collect::<Vec<_>>(),
            "contains_private_key": serialized.contains("private-key") || serialized.contains("secret"),
        });
        expect![[r#"
            {
              "contains_private_key": false,
              "static_state_names": [
                "state-hash"
              ],
              "top_level_fields": [
                "global_config",
                "schema_version",
                "static_states",
                "ton_release"
              ]
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());
    }
}
