//! Block cache and masterchain download checkpoint.
//!
//! Both BOCs are flushed before the checkpoint advances. The directory lock
//! prevents two sources from writing to the same cache.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use tycho_types::models::{BlockId, ShardIdent};

use crate::config::NetworkConfig;

#[derive(Deserialize, Serialize)]
struct Checkpoint {
    version: u32,
    zero_state: BlockId,
    anchor: BlockId,
    head: BlockId,
    previous: Option<BlockId>,
    verification: Verification,
}

/// Records which checks were applied to the cached blocks.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Verification {
    HashesAndLinks,
}

pub(crate) struct Storage {
    directory: PathBuf,
    blocks: PathBuf,
    checkpoint: Checkpoint,
    has_checkpoint: bool,
    _lock: File,
}

impl Storage {
    pub(crate) const fn anchor(&self) -> BlockId {
        self.checkpoint.anchor
    }

    /// Lists committed masterchain IDs. Files left beyond the checkpoint after
    /// an interrupted write are excluded until downloaded and committed again.
    pub(crate) fn masterchain_ids(&self) -> Result<BTreeMap<u32, BlockId>> {
        let mut ids = BTreeMap::new();

        for entry in fs::read_dir(&self.blocks)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str().and_then(|name| name.strip_suffix(".boc")) else {
                continue;
            };
            if name.ends_with(".proof") {
                continue;
            }

            let parts = name.split('-').collect::<Vec<_>>();
            if parts.len() != 3 {
                continue;
            }
            let seqno = parts[0].parse::<u32>()?;
            if seqno < self.anchor().seqno || seqno > self.head().seqno {
                continue;
            }

            let id = BlockId {
                shard: ShardIdent::MASTERCHAIN,
                seqno,
                root_hash: parts[1].parse()?,
                file_hash: parts[2].parse()?,
            };
            ensure!(
                ids.insert(seqno, id).is_none(),
                "conflicting stored masterchain IDs at {seqno}"
            );
        }

        ids.insert(self.anchor().seqno, self.anchor());
        ids.insert(self.head().seqno, self.head());
        Ok(ids)
    }

    pub(crate) fn block_path(&self, id: &BlockId) -> PathBuf {
        self.blocks.join(format!("{}.boc", block_name(id)))
    }

    pub(crate) fn open(directory: &Path, config: &NetworkConfig) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("sync.lock"))?;
        lock.try_lock()
            .with_context(|| format!("cannot lock P2P data directory {}", directory.display()))?;

        let path = directory.join("checkpoint.json");
        let (checkpoint, has_checkpoint) = match fs::read(&path) {
            Ok(bytes) => {
                let checkpoint: Checkpoint = serde_json::from_slice(&bytes)
                    .with_context(|| format!("invalid checkpoint {}", path.display()))?;
                ensure!(
                    checkpoint.version == 1,
                    "unsupported P2P checkpoint version"
                );
                ensure!(
                    checkpoint.zero_state == config.zero_state(),
                    "P2P data directory belongs to another network"
                );
                ensure!(
                    checkpoint.anchor.shard.is_masterchain()
                        && checkpoint.head.shard.is_masterchain(),
                    "checkpoint is not a masterchain download"
                );
                ensure!(
                    checkpoint.head.seqno >= checkpoint.anchor.seqno,
                    "checkpoint precedes its anchor"
                );
                (checkpoint, true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
                Checkpoint {
                    version: 1,
                    zero_state: config.zero_state(),
                    anchor: config.initial_block(),
                    head: config.initial_block(),
                    previous: None,
                    verification: Verification::HashesAndLinks,
                },
                false,
            ),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot read checkpoint {}", path.display()));
            }
        };

        let blocks = directory.join("masterchain");
        fs::create_dir_all(&blocks)?;

        Ok(Self {
            directory: directory.to_owned(),
            blocks,
            checkpoint,
            has_checkpoint,
            _lock: lock,
        })
    }

    pub(crate) const fn head(&self) -> BlockId {
        self.checkpoint.head
    }

    pub(crate) const fn needs_anchor(&self) -> bool {
        !self.has_checkpoint && self.head().seqno > 0
    }

    /// The anchor itself must be downloaded before following its successors.
    pub(crate) fn next_seqno(&self) -> Result<u32> {
        if self.needs_anchor() {
            Ok(self.head().seqno)
        } else {
            self.head()
                .seqno
                .checked_add(1)
                .context("masterchain sequence overflow")
        }
    }

    /// Rechecks the last committed files before resuming. Missing or damaged
    /// data fails explicitly instead of advancing from a metadata-only position.
    pub(crate) fn restore(&self) -> Result<()> {
        if !self.has_checkpoint {
            return Ok(());
        }

        let id = self.head();
        let name = block_name(&id);
        let block_path = self.blocks.join(format!("{name}.boc"));
        let proof_path = self.blocks.join(format!("{name}.proof.boc"));
        let block = fs::read(&block_path)
            .with_context(|| format!("cannot read {}", block_path.display()))?;
        let proof = fs::read(&proof_path)
            .with_context(|| format!("cannot read {}", proof_path.display()))?;

        ensure!(
            self.checkpoint.previous.is_some() || id == self.checkpoint.anchor,
            "checkpoint has no predecessor"
        );
        crate::download::validate_download(&id, self.checkpoint.previous.as_ref(), &block, &proof)
            .with_context(|| format!("invalid downloaded block in {}", block_path.display()))?;

        Ok(())
    }

    /// Commits complete files before advancing progress. Repeating a download
    /// after a crash safely replaces orphan files for the same exact block ID.
    pub(crate) fn commit(&mut self, id: BlockId, block: &[u8], proof: &[u8]) -> Result<()> {
        ensure!(
            id.seqno == self.next_seqno()?,
            "cannot skip masterchain blocks at commit"
        );
        ensure!(
            !self.needs_anchor() || id == self.head(),
            "downloaded anchor differs from checkpoint"
        );

        let name = block_name(&id);
        persist_file(&self.blocks, &format!("{name}.boc"), block)?;
        persist_file(&self.blocks, &format!("{name}.proof.boc"), proof)?;

        // Both renames must be durable before the checkpoint can reference
        // them. One directory flush covers the pair; each file is already synced.
        #[cfg(unix)]
        File::open(&self.blocks)?.sync_all()?;

        let checkpoint = Checkpoint {
            version: 1,
            zero_state: self.checkpoint.zero_state,
            anchor: self.checkpoint.anchor,
            head: id,
            previous: if self.needs_anchor() {
                None
            } else {
                Some(self.head())
            },
            verification: Verification::HashesAndLinks,
        };
        atomic_write(
            &self.directory,
            "checkpoint.json",
            &serde_json::to_vec_pretty(&checkpoint)?,
        )?;
        self.checkpoint = checkpoint;
        self.has_checkpoint = true;

        Ok(())
    }
}

fn block_name(id: &BlockId) -> String {
    format!("{}-{}-{}", id.seqno, id.root_hash, id.file_hash)
}

/// Flushes content before rename, then flushes the containing directory on Unix.
pub(crate) fn atomic_write(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    persist_file(directory, name, bytes)?;

    #[cfg(unix)]
    File::open(directory)?.sync_all()?;

    Ok(())
}

/// Flushes and renames a file. The caller must flush the directory before
/// publishing a checkpoint that depends on this name surviving a crash.
fn persist_file(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = directory.join(name);
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(&path)
        .with_context(|| format!("cannot persist {}", path.display()))?;

    Ok(())
}
