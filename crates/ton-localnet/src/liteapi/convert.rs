use crate::LocalnetError;
use crate::localnet::{LocalnetBlockHeader, LocalnetBlockId, LocalnetMasterchainInfo};
use crate::types::Addr;
use ton_liteapi::tl::common::{AccountId, BlockIdExt, Int256, ZeroStateIdExt};
use ton_liteapi::tl::response::{BlockHeader, MasterchainInfo, MasterchainInfoExt};

use super::{LITEAPI_CAPABILITIES, LITEAPI_VERSION};

/// `LiteServer` reserves workchain `-1` for the masterchain.
///
/// Localnet mines a basechain shard block and a matching masterchain block for
/// each seqno. The masterchain block carries shard/config metadata for `LiteAPI`
/// discovery, while executable transactions remain on the basechain block.
pub(super) const MASTERCHAIN_WORKCHAIN: i32 = -1;

/// Converts localnet's hash wrapper into the `int256` type used by TL.
///
/// `LiteServer` TL objects carry hashes as fixed-width integer byte arrays. The
/// localnet representation already stores canonical 32-byte hashes, so the
/// conversion is a zero-copy shape change at the value level.
pub(super) const fn int256(bytes: [u8; 32]) -> Int256 {
    Int256(bytes)
}

/// Converts a localnet block id into the `LiteServer` `tonNode.blockIdExt`.
///
/// The same shape is used for both real localnet masterchain blocks and the
/// single basechain shard. `LiteAPI` clients treat the root/file hashes in this id
/// as the consistency anchor for follow-up block, transaction, proof, and shard
/// calls.
pub(super) fn block_id_ext(id: &LocalnetBlockId) -> BlockIdExt {
    BlockIdExt {
        workchain: id.workchain,
        shard: id.shard,
        seqno: i32::try_from(id.seqno).unwrap_or(i32::MAX),
        root_hash: int256(id.root_hash.0),
        file_hash: int256(id.file_hash.0),
    }
}

/// Converts a `LiteServer` account id into the localnet address type.
///
/// `LiteServer` splits an account into workchain and 256-bit account id, while
/// localnet uses the same pair as one `Addr` value. This helper is used for
/// account state, transaction, and library-adjacent lookups.
pub(super) const fn addr_from_account_id(id: &AccountId) -> Addr {
    Addr {
        workchain: id.workchain,
        addr: id.id.0,
    }
}

/// Reads a non-negative TL sequence number as the local `u32` seqno type.
///
/// TL schema uses signed `int` for block sequence numbers. Localnet block
/// history is indexed by unsigned sequence numbers, so negative values are
/// rejected before they can accidentally wrap.
pub(super) fn seqno_from_i32(seqno: i32) -> anyhow::Result<u32> {
    u32::try_from(seqno).map_err(|_| {
        LocalnetError::protocol_violation(format!("Invalid negative block seqno {seqno}")).into()
    })
}

/// Builds the `LiteServer` masterchain info response from stored block metadata.
pub(super) fn masterchain_info(info: LocalnetMasterchainInfo) -> MasterchainInfo {
    MasterchainInfo {
        last: block_id_ext(&info.last),
        state_root_hash: int256(info.state_root_hash.0),
        init: masterchain_zero_state_id(&info.init),
    }
}

/// Builds the extended masterchain info response with wall-clock metadata.
///
/// Anton and tonutils-go use this variant for node capability discovery. The
/// local implementation reports only the `LiteServer` capabilities backed by the
/// stored localnet block/state model, rather than mirroring a full validator
/// liteserver.
pub(super) fn masterchain_info_ext(
    info: LocalnetMasterchainInfo,
    header: Option<&LocalnetBlockHeader>,
    now: u32,
) -> anyhow::Result<MasterchainInfoExt> {
    Ok(MasterchainInfoExt {
        mode: (),
        version: LITEAPI_VERSION,
        capabilities: LITEAPI_CAPABILITIES,
        last: block_id_ext(&info.last),
        last_utime: header.map_or(0, |header| header.gen_utime),
        now,
        state_root_hash: int256(info.state_root_hash.0),
        init: masterchain_zero_state_id(&info.init),
    })
}

/// Builds the TL response for `liteServer.getBlockHeader` and lookup results.
///
/// The caller supplies `header_proof` because the proof must be built from the
/// exact block `BoC` stored for the requested workchain. Tonlib virtualizes that
/// proof and reads header fields from the resulting block root.
pub(super) fn block_header(
    header: LocalnetBlockHeader,
    with_state_update: Option<()>,
    with_value_flow: Option<()>,
    with_extra: Option<()>,
    with_shard_hashes: Option<()>,
    with_prev_blk_signatures: Option<()>,
    header_proof: Vec<u8>,
) -> BlockHeader {
    block_header_with_id(
        block_id_ext(&header.id),
        with_state_update,
        with_value_flow,
        with_extra,
        with_shard_hashes,
        with_prev_blk_signatures,
        header_proof,
    )
}

const fn block_header_with_id(
    id: BlockIdExt,
    with_state_update: Option<()>,
    with_value_flow: Option<()>,
    with_extra: Option<()>,
    with_shard_hashes: Option<()>,
    with_prev_blk_signatures: Option<()>,
    header_proof: Vec<u8>,
) -> BlockHeader {
    BlockHeader {
        id,
        mode: (),
        with_state_update,
        with_value_flow,
        with_extra,
        with_shard_hashes,
        with_prev_blk_signatures,
        header_proof,
    }
}

const fn masterchain_zero_state_id(id: &LocalnetBlockId) -> ZeroStateIdExt {
    ZeroStateIdExt {
        workchain: MASTERCHAIN_WORKCHAIN,
        root_hash: Int256(id.root_hash.0),
        file_hash: Int256(id.file_hash.0),
    }
}
