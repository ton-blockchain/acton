//! Publishes verified hardfork states for stock TON initial synchronization.
//!
//! TON refuses an init block older than the latest hardfork. Its ordinary state
//! serializer can delay publication for hours, so administrative edits stage the
//! complete, authenticated states during verification and install them while the
//! engine is stopped. ArchiveManager discovers these native files on restart.

use super::*;
use sha2::{Digest, Sha256};
use ton_hardfork::HardforkSources;
use tycho_types::boc::Boc;

/// Stages every state needed by a fresh node, including an unchanged basechain.
/// Only called after the live verification has authenticated the complete states.
pub(super) fn stage(layout: &Layout, sources: &HardforkSources) -> Result<()> {
    let directory = staging_dir(&layout.node).join("bootstrap-states");
    fs::create_dir_all(&directory)?;
    let masterchain = BlockId {
        shard: tycho_types::models::ShardIdent::MASTERCHAIN,
        seqno: sources.masterchain_prev.seqno,
        root_hash: sources.masterchain_prev.root_hash,
        file_hash: sources.masterchain_prev.file_hash,
    };
    fs::write(
        directory.join(filename(&masterchain, &masterchain)),
        Boc::encode(&sources.masterchain_state),
    )?;

    if let Some(shard) = &sources.basechain
        && shard.prev.seqno != 0
    {
        let block = BlockId {
            shard: shard.shard,
            seqno: shard.prev.seqno,
            root_hash: shard.prev.root_hash,
            file_hash: shard.prev.file_hash,
        };
        fs::write(
            directory.join(filename(&block, &masterchain)),
            Boc::encode(&shard.state),
        )?;
    }
    Ok(())
}

/// Installs staged states before advertising their block in either global config.
/// The caller owns the state lock and must keep validator-engine stopped so its
/// archive index can discover the new files on the next startup.
pub(super) fn publish(layout: &Layout) -> Result<()> {
    let started = std::time::Instant::now();
    let staging = staging_dir(&layout.node);
    let plan: HardforkPlan = serde_json::from_slice(&fs::read(staging.join("plan.json"))?)?;
    let masterchain = BlockId {
        shard: tycho_types::models::ShardIdent::MASTERCHAIN,
        seqno: plan.masterchain.seqno,
        root_hash: HashBytes(*plan.masterchain.root_hash.as_bytes()),
        file_hash: HashBytes(*plan.masterchain.file_hash.as_bytes()),
    };
    let source = staging.join("bootstrap-states");
    ensure!(
        source.join(filename(&masterchain, &masterchain)).is_file(),
        "Verify the hardfork states before publishing its bootstrap block"
    );
    let target = layout.node.db.join("archive/states");
    fs::create_dir_all(&target)?;

    // Copy through a sibling directory: a partial file must never be mistaken
    // for a downloadable state, even if finish is interrupted before restart.
    for entry in fs::read_dir(&source)? {
        let entry = entry?;
        let temporary = staging.join("bootstrap-state.tmp");
        fs::copy(entry.path(), &temporary)?;
        fs::File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, target.join(entry.file_name()))?;
    }

    for path in [&layout.global_config, &layout.node.global_config] {
        let mut config = GlobalConfig::load(path)?;
        config.use_hardfork_for_bootstrap(masterchain.seqno)?;
        config.save_atomic(path)?;
    }
    info!(
        operation = "publish_hardfork_bootstrap",
        node = %layout.node.root.display(),
        target = %target.display(),
        seqno = masterchain.seqno,
        duration_ms = started.elapsed().as_millis(),
        outcome = "published"
    );
    Ok(())
}

// Native validator/db/fileref.cpp names states by SHA-256 of the boxed TL key:
// db.filedb.key.persistentStateFile block_id:tonNode.blockIdExt
// masterchain_block_id:tonNode.blockIdExt = db.filedb.Key
// The nested block ids are bare TL values, so they have no constructor prefixes.
// TON reconstructs the path with an uppercase hash on case-sensitive filesystems.
fn filename(block: &BlockId, masterchain: &BlockId) -> String {
    const KEY: u32 = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC).checksum(
        b"db.filedb.key.persistentStateFile block_id:tonNode.blockIdExt masterchain_block_id:tonNode.blockIdExt = db.filedb.Key",
    );
    let mut hash = Sha256::new();
    hash.update(KEY.to_le_bytes());
    for id in [block, masterchain] {
        hash.update(id.shard.workchain().to_le_bytes());
        hash.update(id.shard.prefix().to_le_bytes());
        hash.update(id.seqno.to_le_bytes());
        hash.update(id.root_hash.as_slice());
        hash.update(id.file_hash.as_slice());
    }
    format!(
        "state_{}_{}_{:x}_{}",
        masterchain.seqno,
        block.shard.workchain(),
        block.shard.prefix(),
        hex::encode_upper(hash.finalize())
    )
}
