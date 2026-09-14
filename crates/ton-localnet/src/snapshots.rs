//! Durable snapshots shared by the node control API and Studio's stopped environments.
//!
//! Each immutable JSON file contains its metadata and state. Publishing one file atomically
//! keeps interrupted saves out of the inventory without a separate manifest or memory cache.

use crate::node::{Node, StateSource};
use crate::node_snapshot::NodeStateSnapshot;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const FORMAT_VERSION: u32 = 1;

/// Inventory data for a saved state; the ID, rather than its display name, identifies the file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Snapshot {
    pub id: String,
    pub name: Option<String>,
    pub created_at: u64,
    pub block_seqno: u32,
    pub size_bytes: u64,
}

#[derive(Deserialize, Serialize)]
struct SavedSnapshot {
    format_version: u32,
    id: String,
    name: Option<String>,
    created_at: u64,
    fork_network: Option<String>,
    state: NodeStateSnapshot,
}

/// Owns the snapshot directory independently of the node's lifetime and current state.
///
/// Mutating live state remains the node actor's responsibility; inventory operations also
/// work while the environment is stopped. Restoring never changes the saved files.
#[derive(Clone, Debug)]
pub struct SnapshotStore {
    directory: PathBuf,
}

impl SnapshotStore {
    /// Selects the inventory location without creating files until the first save or import.
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    /// Keeps a persistent node's snapshots next to its database, outside the database itself.
    #[must_use]
    pub fn for_database(database: &Path) -> Self {
        Self::new(database.with_extension("snapshots"))
    }

    /// Captures a stopped environment's persisted state without starting mining or HTTP services.
    /// The caller must own the database lifecycle and ensure its node process is stopped.
    pub fn create_from_database(
        &self,
        database: &Path,
        name: Option<String>,
        fork_network: Option<String>,
        fork_block_number: Option<u64>,
    ) -> anyhow::Result<Snapshot> {
        anyhow::ensure!(
            database.is_file(),
            "The environment has no saved database yet"
        );

        let source = match fork_network {
            Some(network) => StateSource::Remote(crate::remote::RemoteProvider {
                network: network.parse()?,
                fork_block_number,
                fork_snapshot: None,
            }),
            None => StateSource::Local,
        };
        let node = Node::with_db_path(
            Box::new(crate::executor::TvmEmulatorAdapter::new()?),
            crate::types::BocBytes::from_base64(ton_executor::DEFAULT_CONFIG)?,
            source,
            Some(database),
        )?;

        self.create(&node, name)
    }

    /// Reads committed snapshot files; unfinished temporary files are excluded from the inventory.
    pub fn list(&self) -> anyhow::Result<Vec<Snapshot>> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Cannot list snapshots in {}", self.directory.display())
                });
            }
        };

        let mut snapshots = Vec::new();

        for entry in entries {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }

            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .context("Invalid snapshot filename")?;
            let saved = self.read(id)?;
            snapshots.push(Self::metadata(&saved, fs::metadata(&path)?.len()));
        }

        snapshots.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(snapshots)
    }

    /// Deletes only the selected saved file, without touching the running state or other snapshots.
    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let path = self.path(id)?;
        fs::remove_file(&path).with_context(|| format!("Cannot delete snapshot {}", path.display()))
    }

    /// Returns a saved file verbatim, preserving the state captured at creation.
    pub fn export(&self, id: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.path(id)?;
        fs::read(&path).with_context(|| format!("Cannot read snapshot {}", path.display()))
    }

    /// Validates a file before publishing it under a new ID. Import never restores state and
    /// cannot overwrite an existing snapshot, including when importing an earlier export.
    pub fn import(&self, json: &[u8], name: Option<String>) -> anyhow::Result<Snapshot> {
        let mut saved: SavedSnapshot =
            serde_json::from_slice(json).context("Invalid snapshot JSON")?;
        Self::validate(&saved)?;

        saved.id = Uuid::new_v4().to_string();
        saved.name = normalize_name(name.or(saved.name))?;
        saved.created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        self.write(&saved)
    }

    pub(crate) fn create(&self, node: &Node, name: Option<String>) -> anyhow::Result<Snapshot> {
        let saved = SavedSnapshot {
            format_version: FORMAT_VERSION,
            id: Uuid::new_v4().to_string(),
            name: normalize_name(name)?,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            fork_network: match &node.state_source {
                StateSource::Local => None,
                StateSource::Remote(provider) => Some(provider.network.to_string()),
            },
            state: node.build_snapshot()?,
        };

        self.write(&saved)
    }

    pub(crate) fn restore(&self, node: &mut Node, id: &str) -> anyhow::Result<Snapshot> {
        let saved = self.read(id)?;
        let network = match &node.state_source {
            StateSource::Local => None,
            StateSource::Remote(provider) => Some(provider.network.to_string()),
        };

        anyhow::ensure!(
            network == saved.fork_network,
            "Snapshot belongs to a different fork network"
        );
        Self::validate(&saved)?;

        let metadata = Self::metadata(&saved, fs::metadata(self.path(id)?)?.len());
        node.apply_snapshot(saved.state)?;

        Ok(metadata)
    }

    fn path(&self, id: &str) -> anyhow::Result<PathBuf> {
        let parsed = Uuid::parse_str(id).context("Invalid snapshot ID")?;
        anyhow::ensure!(parsed.to_string() == id, "Invalid snapshot ID");

        Ok(self.directory.join(format!("{id}.json")))
    }

    fn read(&self, id: &str) -> anyhow::Result<SavedSnapshot> {
        let path = self.path(id)?;
        let saved: SavedSnapshot = serde_json::from_reader(BufReader::new(File::open(&path)?))
            .with_context(|| format!("Cannot read snapshot {}", path.display()))?;

        anyhow::ensure!(
            saved.format_version == FORMAT_VERSION,
            "Unsupported snapshot format version {}",
            saved.format_version
        );
        anyhow::ensure!(saved.id == id, "Snapshot ID does not match its filename");

        Ok(saved)
    }

    fn validate(saved: &SavedSnapshot) -> anyhow::Result<()> {
        anyhow::ensure!(
            saved.format_version == FORMAT_VERSION,
            "Unsupported snapshot format version {}",
            saved.format_version
        );
        Node::validate_snapshot(&saved.state)?;

        Ok(())
    }

    fn write(&self, saved: &SavedSnapshot) -> anyhow::Result<Snapshot> {
        fs::create_dir_all(&self.directory)?;
        let path = self.path(&saved.id)?;

        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        serde_json::to_writer(file.as_file_mut(), saved)?;
        file.as_file_mut().flush()?;
        file.as_file().sync_all()?;
        let size = file.as_file().metadata()?.len();

        file.persist_noclobber(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("Cannot save snapshot {}", path.display()))?;

        Ok(Self::metadata(saved, size))
    }

    fn metadata(saved: &SavedSnapshot, size_bytes: u64) -> Snapshot {
        Snapshot {
            id: saved.id.clone(),
            name: saved.name.clone(),
            created_at: saved.created_at,
            block_seqno: saved.state.globals.head_seqno,
            size_bytes,
        }
    }
}

fn normalize_name(name: Option<String>) -> anyhow::Result<Option<String>> {
    let name = name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());

    anyhow::ensure!(
        name.as_ref()
            .is_none_or(|name| name.len() <= 128 && !name.chars().any(char::is_control)),
        "Snapshot names must be at most 128 bytes without control characters"
    );

    Ok(name)
}
