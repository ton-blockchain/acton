//! Validated TON blocks and canonical indexing batches.

use std::collections::HashSet;

use thiserror::Error;
use tycho_types::{
    boc::Boc,
    cell::{Cell, HashBytes, Lazy},
    models::{Block, BlockInfo, PrevBlockRef, Transaction, block::BlockId as TychoBlockId},
};

use crate::{BlockId, Error, Hash256, Result};

/// Errors produced while decoding or inspecting TON blocks.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// The payload is not a valid single-root `BoC`.
    #[error("invalid block BoC: {0}")]
    InvalidBoc(String),
    /// The root cell does not contain the expected TL-B model.
    #[error("invalid TON model: {0}")]
    InvalidModel(String),
    /// A masterchain block did not contain the masterchain extension.
    #[error("masterchain block has no McBlockExtra")]
    MissingMasterchainExtra,
    /// The `BoC` hash does not match the id returned by the transport.
    #[error("{kind} hash mismatch for block {block_id}: expected {expected}, got {actual}")]
    HashMismatch {
        /// Name of the mismatching hash.
        kind: &'static str,
        /// Block being decoded.
        block_id: Box<BlockId>,
        /// Hash carried by the block id.
        expected: Hash256,
        /// Hash calculated from the payload.
        actual: Hash256,
    },
    /// Decoded block metadata does not match the requested block id.
    #[error("block id mismatch: requested {requested}, decoded {decoded}")]
    IdMismatch {
        /// Id returned by the transport.
        requested: Box<BlockId>,
        /// Id reconstructed from the TL-B model and calculated hashes.
        decoded: Box<BlockId>,
    },
    /// A split or merge flag is inconsistent with the shard prefix.
    #[error("invalid shard topology for block {0}")]
    InvalidTopology(BlockId),
    /// A masterchain-only operation was requested for a shard block.
    #[error("block {0} is not a masterchain block")]
    NotMasterchain(BlockId),
}

/// A validated, full-fidelity TON block backed by owned `tycho-types` cells.
#[derive(Clone, Debug)]
pub struct BlockData {
    id: BlockId,
    root: Cell,
    block: Block,
    info: BlockInfo,
    transactions: Vec<Lazy<Transaction>>,
}

impl BlockData {
    /// Decodes and validates a block `BoC` against its transport-provided id.
    ///
    /// Nested transaction dictionaries are traversed once and retained as
    /// full [`Lazy<Transaction>`] values for all downstream consumers.
    ///
    /// # Errors
    ///
    /// Returns an error if the file hash, root hash, block identity, `BoC`,
    /// block model, or transaction dictionaries are invalid.
    pub fn decode(id: BlockId, boc: &[u8]) -> std::result::Result<Self, DecodeError> {
        let actual_file_hash = from_hash(Boc::file_hash(boc));
        if actual_file_hash != id.file_hash {
            return Err(DecodeError::HashMismatch {
                kind: "file",
                block_id: Box::new(id),
                expected: id.file_hash,
                actual: actual_file_hash,
            });
        }

        let root = Boc::decode(boc).map_err(|error| DecodeError::InvalidBoc(error.to_string()))?;
        let actual_root_hash = from_hash(*root.repr_hash());
        if actual_root_hash != id.root_hash {
            return Err(DecodeError::HashMismatch {
                kind: "root",
                block_id: Box::new(id),
                expected: id.root_hash,
                actual: actual_root_hash,
            });
        }

        let block = root.parse::<Block>().map_err(invalid_model("block root"))?;
        let info = block.load_info().map_err(invalid_model("block info"))?;
        let decoded_id = BlockId {
            workchain: info.shard.workchain(),
            shard: info.shard.prefix(),
            seqno: info.seqno,
            root_hash: actual_root_hash,
            file_hash: actual_file_hash,
        };
        if decoded_id != id {
            return Err(DecodeError::IdMismatch {
                requested: Box::new(id),
                decoded: Box::new(decoded_id),
            });
        }

        let transactions = collect_transactions(&block)?;
        Ok(Self {
            id,
            root,
            block,
            info,
            transactions,
        })
    }

    /// Returns the canonical block identity.
    #[must_use]
    pub const fn id(&self) -> BlockId {
        self.id
    }

    /// Returns the original root cell decoded from the block `BoC`.
    #[must_use]
    pub const fn root(&self) -> &Cell {
        &self.root
    }

    /// Returns the full lazy TON block model.
    #[must_use]
    pub const fn block(&self) -> &Block {
        &self.block
    }

    /// Returns eagerly decoded block metadata.
    #[must_use]
    pub const fn info(&self) -> &BlockInfo {
        &self.info
    }

    /// Returns all full transaction cells in account/dictionary order.
    #[must_use]
    pub fn transactions(&self) -> &[Lazy<Transaction>] {
        &self.transactions
    }

    /// Returns the shard frontier committed by this masterchain block.
    ///
    /// # Errors
    ///
    /// Returns an error for a shard block, a missing masterchain extension,
    /// or malformed shard hashes.
    pub fn shard_frontier(&self) -> std::result::Result<Vec<BlockId>, DecodeError> {
        if !self.id.is_masterchain() {
            return Err(DecodeError::NotMasterchain(self.id));
        }

        let custom = self
            .block
            .load_extra()
            .map_err(invalid_model("block extra"))?
            .load_custom()
            .map_err(invalid_model("masterchain extra"))?
            .ok_or(DecodeError::MissingMasterchainExtra)?;

        custom
            .shards
            .latest_blocks()
            .map(|item| {
                item.map(from_tycho_block_id)
                    .map_err(invalid_model("shard hashes"))
            })
            .collect()
    }

    /// Reconstructs the full ids of this block's direct predecessors.
    ///
    /// # Errors
    ///
    /// Returns an error when predecessor references or shard topology are
    /// malformed.
    pub fn predecessors(&self) -> std::result::Result<Vec<BlockId>, DecodeError> {
        let previous = self
            .info
            .load_prev_ref()
            .map_err(invalid_model("previous block reference"))?;

        match previous {
            PrevBlockRef::Single(previous) => {
                let shard = if self.info.after_split {
                    self.info
                        .shard
                        .merge()
                        .ok_or(DecodeError::InvalidTopology(self.id))?
                } else {
                    self.info.shard
                };
                Ok(vec![from_tycho_block_id(previous.as_block_id(shard))])
            }
            PrevBlockRef::AfterMerge { left, right } => {
                let (left_shard, right_shard) = self
                    .info
                    .shard
                    .split()
                    .ok_or(DecodeError::InvalidTopology(self.id))?;
                Ok(vec![
                    from_tycho_block_id(left.as_block_id(left_shard)),
                    from_tycho_block_id(right.as_block_id(right_shard)),
                ])
            }
        }
    }
}

/// One canonical masterchain step with all new shard blocks.
#[derive(Clone, Debug)]
pub struct Batch {
    masterchain: BlockData,
    shards: Vec<BlockData>,
}

impl Batch {
    /// Creates a canonical batch after checking its structural invariants.
    ///
    /// # Errors
    ///
    /// Returns an invariant error if the anchor is not a masterchain block,
    /// or if the shard delta contains masterchain or duplicate blocks.
    pub fn try_new(masterchain: BlockData, shards: Vec<BlockData>) -> Result<Self> {
        if !masterchain.id().is_masterchain() {
            return Err(Error::Invariant(format!(
                "batch anchor {} is not a masterchain block",
                masterchain.id()
            )));
        }

        let mut ids = HashSet::with_capacity(shards.len());
        for block in &shards {
            if block.id().is_masterchain() {
                return Err(Error::Invariant(format!(
                    "masterchain block {} found in shard delta",
                    block.id()
                )));
            }
            if !ids.insert(block.id()) {
                return Err(Error::Invariant(format!(
                    "duplicate shard block {} in batch",
                    block.id()
                )));
            }
        }

        Ok(Self {
            masterchain,
            shards,
        })
    }

    /// Returns the masterchain anchor.
    #[must_use]
    pub const fn masterchain(&self) -> &BlockData {
        &self.masterchain
    }

    /// Returns new shard blocks in predecessor-first order.
    #[must_use]
    pub fn shards(&self) -> &[BlockData] {
        &self.shards
    }

    /// Iterates over the masterchain block followed by all shard blocks.
    pub fn blocks(&self) -> impl Iterator<Item = &BlockData> {
        std::iter::once(&self.masterchain).chain(self.shards.iter())
    }

    /// Returns the durable position created by this batch.
    #[must_use]
    pub const fn checkpoint(&self) -> BlockId {
        self.masterchain.id()
    }
}

fn collect_transactions(block: &Block) -> std::result::Result<Vec<Lazy<Transaction>>, DecodeError> {
    let extra = block.load_extra().map_err(invalid_model("block extra"))?;
    let account_blocks = extra
        .account_blocks
        .load()
        .map_err(invalid_model("account blocks"))?;

    let mut transactions = Vec::new();
    for entry in account_blocks.iter() {
        let (_, _, account_block) = entry.map_err(invalid_model("account block dictionary"))?;
        for entry in account_block.transactions.iter() {
            let (_, _, transaction) = entry.map_err(invalid_model("transaction dictionary"))?;
            transactions.push(transaction);
        }
    }
    Ok(transactions)
}

const fn from_tycho_block_id(id: TychoBlockId) -> BlockId {
    BlockId {
        workchain: id.shard.workchain(),
        shard: id.shard.prefix(),
        seqno: id.seqno,
        root_hash: from_hash(id.root_hash),
        file_hash: from_hash(id.file_hash),
    }
}

const fn from_hash(hash: HashBytes) -> Hash256 {
    Hash256::new(hash.0)
}

fn invalid_model(context: &'static str) -> impl FnOnce(tycho_types::error::Error) -> DecodeError {
    move |error| DecodeError::InvalidModel(format!("{context}: {error}"))
}

#[cfg(test)]
pub(crate) fn test_batch(seqno: u32) -> Batch {
    use tycho_types::{
        cell::{CellBuilder, Lazy},
        merkle::MerkleUpdate,
        models::{BlockExtra, ShardIdent, ValueFlow},
    };

    let marker = u8::try_from(seqno).unwrap();
    let info = BlockInfo {
        seqno,
        shard: ShardIdent::MASTERCHAIN,
        ..Default::default()
    };

    let block = Block {
        global_id: 0,
        info: Lazy::new(&info).unwrap(),
        value_flow: Lazy::new(&ValueFlow::default()).unwrap(),
        state_update: Lazy::new(&MerkleUpdate::default()).unwrap(),
        out_msg_queue_updates: None,
        extra: Lazy::new(&BlockExtra::default()).unwrap(),
    };
    let root = CellBuilder::build_from(&block).unwrap();
    let id = BlockId {
        workchain: BlockId::MASTERCHAIN_WORKCHAIN,
        shard: BlockId::FULL_SHARD,
        seqno,
        root_hash: Hash256::new([marker; 32]),
        file_hash: Hash256::new([marker; 32]),
    };

    Batch::try_new(
        BlockData {
            id,
            root,
            block,
            info,
            transactions: Vec::new(),
        },
        Vec::new(),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_payload_before_tlb_decode_when_file_hash_is_wrong() {
        let id = BlockId {
            workchain: BlockId::MASTERCHAIN_WORKCHAIN,
            shard: BlockId::FULL_SHARD,
            seqno: 1,
            root_hash: Hash256::ZERO,
            file_hash: Hash256::ZERO,
        };
        assert!(matches!(
            BlockData::decode(id, &[1, 2, 3]),
            Err(DecodeError::HashMismatch { kind: "file", .. })
        ));
    }

    #[test]
    fn rejects_duplicate_shard_blocks() {
        let mut shard = test_batch(1).masterchain;
        shard.id.workchain = 0;
        let masterchain = test_batch(1).masterchain;

        assert!(Batch::try_new(masterchain, vec![shard.clone(), shard]).is_err());
    }
}
