//! Database records from TON's `ton_api.tl`. Nested block IDs are bare TL values.

use anyhow::{Result, anyhow, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use tl_proto::{TlRead, TlWrite};
use tycho_types::models::{BlockId, ShardIdent};

#[derive(TlRead, TlWrite)]
pub(crate) struct StoredBlockId {
    workchain: i32,
    shard: u64,
    seqno: u32,
    root_hash: [u8; 32],
    file_hash: [u8; 32],
}

impl From<&BlockId> for StoredBlockId {
    fn from(id: &BlockId) -> Self {
        Self {
            workchain: id.shard.workchain(),
            shard: id.shard.prefix(),
            seqno: id.seqno,
            root_hash: id.root_hash.0,
            file_hash: id.file_hash.0,
        }
    }
}

/// CellDB hashes the boxed block ID and encodes it as base64. Block IDs nested
/// inside database values are bare TL, so hashing those bytes gives another key.
pub(crate) fn state_key(id: &BlockId) -> String {
    use sha2::{Digest, Sha256};

    let constructor: u32 = tl_proto::id!("tonNode.blockIdExt", scheme = "db.tl");
    let mut boxed = constructor.to_le_bytes().to_vec();
    boxed.extend(tl_proto::serialize(StoredBlockId::from(id)));

    format!("desc{}", STANDARD.encode(Sha256::digest(boxed)))
}

impl TryFrom<StoredBlockId> for BlockId {
    type Error = anyhow::Error;

    fn try_from(value: StoredBlockId) -> Result<Self> {
        Ok(Self {
            shard: ShardIdent::new(value.workchain, value.shard)
                .ok_or_else(|| anyhow!("invalid stored shard identifier"))?,
            seqno: value.seqno,
            root_hash: value.root_hash.into(),
            file_hash: value.file_hash.into(),
        })
    }
}

#[derive(TlRead)]
#[tl(boxed, id = "db.celldb.value", scheme = "db.tl")]
pub(crate) struct CellState {
    pub block_id: StoredBlockId,
    pub prev: [u8; 32],
    pub next: [u8; 32],
    pub root_hash: [u8; 32],
}

#[derive(TlRead)]
#[tl(boxed, id = "db.state.initBlockId", scheme = "db.tl")]
pub(crate) struct InitBlock {
    pub block: StoredBlockId,
}

#[derive(TlRead)]
#[tl(boxed, id = "db.state.gcBlockId", scheme = "db.tl")]
pub(crate) struct GcBlock {
    pub block: StoredBlockId,
}

#[derive(TlRead)]
#[tl(boxed, id = "db.state.shardClient", scheme = "db.tl")]
pub(crate) struct ShardClient {
    pub block: StoredBlockId,
}

#[derive(TlRead)]
#[tl(boxed, id = "db.state.dbVersion", scheme = "db.tl")]
pub(crate) struct DbVersion {
    pub version: i32,
}

#[derive(TlWrite)]
#[tl(boxed, scheme = "db.tl")]
pub(crate) enum CheckpointKey {
    #[tl(id = "db.state.key.initBlockId")]
    Init,
    #[tl(id = "db.state.key.gcBlockId")]
    Gc,
    #[tl(id = "db.state.key.shardClient")]
    ShardClient,
    #[tl(id = "db.state.key.dbVersion")]
    Version,
}

pub(crate) fn read<'a, T: TlRead<'a>>(mut bytes: &'a [u8]) -> Result<T> {
    let result = T::read_from(&mut bytes)?;
    ensure!(bytes.is_empty(), "trailing bytes in database TL record");

    Ok(result)
}
