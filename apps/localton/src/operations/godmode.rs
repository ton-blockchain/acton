//! Installs administrator-built hardfork blocks into one node's state directory.
//!
//! Changing the state of a running TON network without patching the node means
//! grafting a hardfork block. The masterchain block is registered in the global
//! config and read from `db/static`; a basechain block cannot go there, because a
//! non-masterchain block read from `db/static` is always run through the fork
//! accept path, which refuses it. Shard blocks are instead served to the node
//! over the full-node master protocol, which this state directory hosts locally.
//!
//! Blocks themselves are built elsewhere — the builder needs the chain's full
//! states. This module owns only what has to happen inside the node's own
//! directory: pausing production so the graft point can be observed, publishing
//! the blocks, and putting the node back into its normal networking mode.

use std::{
    fs,
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::task::JoinHandle;
use ton_fullnode_master::{BlockSource, ServedBlock};
use ton_liteapi::adnl::crypto::{KeyPair, SecretKey};
use tracing::{info, warn};
use tycho_types::models::block::{BlockId, ShardIdent};
use tycho_types::prelude::HashBytes;

use crate::{
    cli::GodmodeCommand,
    storage::{BLOCK_SOURCE_PORT, Layout, NodeLayout, write_json_atomic},
    ton::{
        global_config::GlobalConfig,
        tools::{
            types::{TonBlockHash, TonPublicKey},
            validator_engine_config::{EngineValidator, ValidatorEngineConfig},
        },
    },
};

#[path = "godmode_workflow.rs"]
mod workflow;
pub(crate) use workflow::recover_install;
use workflow::{install, live_command};

/// Masterchain workchain id, the only chain a hardfork may be registered for.
const MASTERCHAIN_ID: i32 = -1;

/// Runs one step of an administrative graft against a state directory.
pub(crate) async fn execute(command: GodmodeCommand) -> Result<()> {
    match &command {
        GodmodeCommand::Observe(state)
        | GodmodeCommand::Prepare(state)
        | GodmodeCommand::Verify(state) => {
            return live_command(&Layout::new(state.state_dir.clone()), &command).await;
        }
        _ => {}
    }
    let state = match &command {
        GodmodeCommand::Suspend(s) | GodmodeCommand::Resume(s) | GodmodeCommand::Finish(s) => s,
        GodmodeCommand::Install { state, .. } => state,
        _ => unreachable!(),
    };
    let layout = Layout::new(state.state_dir.clone());
    let _lock = crate::bootstrap::acquire_lock(&layout.lock)
        .context("Stop this Localton instance before changing its engine configuration")?;
    recover_install(&layout)?;
    match command {
        GodmodeCommand::Observe(_) | GodmodeCommand::Prepare(_) | GodmodeCommand::Verify(_) => {
            unreachable!()
        }
        GodmodeCommand::Suspend(state) => {
            let layout = Layout::new(state.state_dir);
            let suspended = suspend_validation(&layout.node)?;
            println!("{}", json!({ "suspended": suspended }));
        }
        GodmodeCommand::Resume(state) => {
            let layout = Layout::new(state.state_dir);
            let resumed = resume_validation(&layout.node)?;
            println!("{}", json!({ "resumed": resumed }));
        }
        GodmodeCommand::Install { state, plan } => {
            let layout = Layout::new(state.state_dir);
            let plan: HardforkPlan =
                serde_json::from_slice(&if plan == std::path::Path::new("-") {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    std::io::stdin()
                        .take(64 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)?;
                    ensure!(
                        bytes.len() <= 64 * 1024 * 1024,
                        "Hardfork plan is too large"
                    );
                    bytes
                } else {
                    fs::read(&plan).with_context(|| format!("failed to read {}", plan.display()))?
                })
                .context("invalid hardfork plan")?;
            let (_, key) = source_identity(&layout.node)?;
            install(
                &layout,
                &plan,
                BlockSourceEndpoint {
                    port: BLOCK_SOURCE_PORT,
                    key,
                },
            )?;
            println!(
                "{}",
                json!({
                    "hardforks": GlobalConfig::load(&layout.global_config)?.hardfork_seqnos(),
                    "shard_blocks": plan.shard_blocks.len(),
                    "block_source": "automatic",
                })
            );
        }
        GodmodeCommand::Finish(state) => {
            let layout = Layout::new(state.state_dir);
            let detached = finish(&layout.node)?;
            println!("{}", json!({ "detached": detached }));
        }
    }
    Ok(())
}

/// One block an administrator built for this network.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct PlannedBlock {
    pub(crate) workchain: i32,
    pub(crate) shard: u64,
    pub(crate) seqno: u32,
    pub(crate) root_hash: TonBlockHash,
    pub(crate) file_hash: TonBlockHash,
    /// Base64 of the serialized block `BoC`.
    pub(crate) block: String,
    /// Base64 of the serialized `BlockProof` link, required for shard blocks.
    #[serde(default)]
    pub(crate) proof: Option<String>,
}

/// Every block of one administrative change.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct HardforkPlan {
    /// Masterchain key block; the one the node is told about through its config.
    pub(crate) masterchain: PlannedBlock,
    /// Shard blocks the masterchain block publishes, served over the network.
    #[serde(default)]
    pub(crate) shard_blocks: Vec<PlannedBlock>,
}

/// Local endpoint a node downloads administrator-built shard blocks from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockSourceEndpoint {
    pub(crate) port: u16,
    pub(crate) key: TonPublicKey,
}

/// Stops block production without touching the engine keyring.
///
/// A hardfork block is only accepted directly on top of the node's current top
/// block, so the graft point has to be read while nothing is producing. Returns
/// whether anything was suspended; the removed keys are kept next to the engine
/// config so an interrupted graft can still be undone.
pub(crate) fn suspend_validation(node: &NodeLayout) -> Result<bool> {
    let config_path = engine_config_path(node);
    let mut config = ValidatorEngineConfig::load(&config_path)?;
    if suspended_keys_path(node).exists() {
        anyhow::ensure!(
            !config.validates(),
            "Suspended keys already exist but validation is enabled"
        );
        return Ok(false);
    }
    let suspended = config.suspend_validation();
    write_json_atomic(&suspended_keys_path(node), &suspended)
        .context("failed to store suspended validator keys")?;
    config.save(&config_path)?;
    info!(operation = "suspend_validation", outcome = "suspended");
    Ok(true)
}

/// Restores block production suspended by [`suspend_validation`].
pub(crate) fn resume_validation(node: &NodeLayout) -> Result<bool> {
    let path = suspended_keys_path(node);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let suspended: Vec<EngineValidator> =
        serde_json::from_slice(&bytes).context("invalid suspended validator keys")?;

    let config_path = engine_config_path(node);
    let mut config = ValidatorEngineConfig::load(&config_path)?;
    invalidate_observation(node)?;
    config.resume_validation(suspended);
    config.save(&config_path)?;
    fs::remove_file(&path)?;
    info!(operation = "resume_validation", outcome = "resumed");
    Ok(true)
}

/// Publishes one administrative change to this state directory.
///
/// The masterchain block is written where the engine looks for hardfork data and
/// registered in the network config. Shard blocks are staged for the local block
/// source, and the node is pointed at it so it downloads them instead of asking
/// the public overlays, which have no copy.
fn install_unchecked(
    layout: &Layout,
    plan: &HardforkPlan,
    source: BlockSourceEndpoint,
) -> Result<()> {
    if plan.masterchain.workchain != MASTERCHAIN_ID {
        bail!(
            "a hardfork must be registered for a masterchain block, got workchain {}",
            plan.masterchain.workchain
        );
    }

    let static_dir = layout.node.db.join("static");
    fs::create_dir_all(&static_dir)
        .with_context(|| format!("failed to create {}", static_dir.display()))?;
    let block = decode(&plan.masterchain.block, "masterchain block")?;
    let name = plan.masterchain.file_hash.to_static_state_filename();
    fs::write(static_dir.join(&name), &block)
        .with_context(|| format!("failed to write static block {name}"))?;

    let staging = staging_dir(&layout.node);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)
        .with_context(|| format!("failed to create {}", staging.display()))?;
    for shard_block in &plan.shard_blocks {
        if shard_block.workchain == MASTERCHAIN_ID {
            bail!("masterchain blocks are grafted through db/static, not the block source");
        }
        let name = shard_block.file_hash.to_static_state_filename();
        fs::write(
            staging.join(&name),
            decode(&shard_block.block, "shard block")?,
        )?;
        let proof = shard_block
            .proof
            .as_ref()
            .context("a shard block needs a proof link to be downloadable")?;
        fs::write(
            staging.join(format!("{name}.proof")),
            decode(proof, "shard block proof")?,
        )?;
    }
    write_json_atomic(&staging.join("plan.json"), plan)
        .context("failed to store the hardfork plan")?;

    let mut config = GlobalConfig::load(&layout.global_config)?;
    config.push_hardfork(
        plan.masterchain.seqno,
        plan.masterchain.root_hash,
        plan.masterchain.file_hash,
    );
    config.save_atomic(&layout.global_config)?;

    if !plan.shard_blocks.is_empty() {
        let config_path = engine_config_path(&layout.node);
        let mut engine = ValidatorEngineConfig::load(&config_path)?;
        engine.set_full_node_master(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, source.port),
            source.key,
        );
        engine.save(&config_path)?;
    }

    info!(
        operation = "install_hardfork",
        seqno = plan.masterchain.seqno,
        shard_blocks = plan.shard_blocks.len(),
        outcome = "installed"
    );
    Ok(())
}

/// Returns the node to normal overlay networking after a graft.
///
/// While a node downloads through a full-node master it stops broadcasting its
/// own blocks, so the link must not outlive the graft that needed it.
pub(crate) fn finish(node: &NodeLayout) -> Result<bool> {
    let staging = staging_dir(node);
    if !staging.join("plan.json").exists() {
        return Ok(false);
    }
    ensure!(
        staging.join("verified.json").is_file(),
        "Verify the applied hardfork before finishing it"
    );
    let config_path = engine_config_path(node);
    let mut config = ValidatorEngineConfig::load(&config_path)?;
    if let Ok(original) = fs::read(staging.join("original-engine.json")) {
        let original: ValidatorEngineConfig = serde_json::from_slice(&original)?;
        config.restore_full_node_master(&original);
    } else {
        config.clear_full_node_master();
    }
    config.save(&config_path)?;
    let staging = staging_dir(node);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    info!(operation = "finish_hardfork", outcome = "detached");
    Ok(true)
}

/// One staged shard block, decoded and ready to be served.
pub(crate) struct StagedBlock {
    planned: PlannedBlock,
    block: Vec<u8>,
    proof: Vec<u8>,
}

/// Returns the blocks staged for the local block source, if any.
pub(crate) fn staged_blocks(node: &NodeLayout) -> Result<Vec<StagedBlock>> {
    let staging = staging_dir(node);
    let Ok(bytes) = fs::read(staging.join("plan.json")) else {
        return Ok(Vec::new());
    };
    let plan: HardforkPlan =
        serde_json::from_slice(&bytes).context("invalid staged hardfork plan")?;

    let mut staged = Vec::with_capacity(plan.shard_blocks.len());
    for shard_block in plan.shard_blocks {
        let name = shard_block.file_hash.to_static_state_filename();
        let block = fs::read(staging.join(&name))
            .with_context(|| format!("failed to read staged block {name}"))?;
        let proof = fs::read(staging.join(format!("{name}.proof")))
            .with_context(|| format!("failed to read staged proof {name}"))?;
        staged.push(StagedBlock {
            planned: shard_block,
            block,
            proof,
        });
    }
    Ok(staged)
}

fn decode(value: &str, what: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(value)
        .with_context(|| format!("{what} is not valid base64"))
}

fn engine_config_path(node: &NodeLayout) -> PathBuf {
    node.db.join("config.json")
}

fn suspended_keys_path(node: &NodeLayout) -> PathBuf {
    node.db.join("validators.suspended.json")
}

fn staging_dir(node: &NodeLayout) -> PathBuf {
    node.root.join("godmode")
}

/// Loads, or creates once, the identity of this state directory's block source.
///
/// Nodes address the source by its public key, so the key has to survive
/// restarts; it is generated on first use and kept beside the engine database.
pub(crate) fn source_identity(node: &NodeLayout) -> Result<([u8; 32], TonPublicKey)> {
    let path = node.db.join("block-source.key");
    let secret: [u8; 32] = match fs::read(&path) {
        Ok(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("block source key must contain exactly 32 bytes"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut secret = [0u8; 32];
            getrandom(&mut secret).context("failed to draw a block source key")?;
            fs::create_dir_all(&node.db)?;
            fs::write(&path, secret)
                .with_context(|| format!("failed to write {}", path.display()))?;
            secret
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let keypair = KeyPair::from(&SecretKey::from_bytes(secret));
    Ok((
        secret,
        TonPublicKey::from_bytes(*keypair.public_key.as_bytes()),
    ))
}

/// A running block source that stops when it goes out of scope.
///
/// Nothing outside a graft should be able to download administrator-built
/// blocks, so the server's lifetime is tied to the instance that started it.
pub(crate) struct RunningBlockSource(JoinHandle<()>);

impl Drop for RunningBlockSource {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Serves the blocks staged for this state directory, if there are any.
///
/// The node only reaches the source over loopback, and only while a graft is in
/// flight; [`finish`] removes both the staged blocks and the node's link to it.
pub(crate) async fn serve_staged(
    node: &NodeLayout,
    port: u16,
) -> Result<Option<RunningBlockSource>> {
    let staged = staged_blocks(node)?;
    if staged.is_empty() {
        return Ok(None);
    }
    let (secret, _) = source_identity(node)?;

    let source = BlockSource::new();
    for staged in staged {
        let planned = &staged.planned;
        let shard = ShardIdent::new(planned.workchain, planned.shard).with_context(|| {
            format!("invalid shard {}:{:016x}", planned.workchain, planned.shard)
        })?;
        let block_id = BlockId {
            shard,
            seqno: planned.seqno,
            root_hash: HashBytes(*planned.root_hash.as_bytes()),
            file_hash: HashBytes(*planned.file_hash.as_bytes()),
        };
        info!(operation = "serve_staged_block", block = %block_id, "publishing hardfork shard block");
        source
            .insert(
                &block_id,
                ServedBlock {
                    data: staged.block,
                    proof_link: staged.proof,
                },
            )
            .await;
    }

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .context("Failed to bind the hardfork block source")?;
    let address = listener.local_addr()?;
    let (_, key) = source_identity(node)?;
    let config_path = engine_config_path(node);
    let mut engine = ValidatorEngineConfig::load(&config_path)?;
    engine.set_full_node_master(SocketAddrV4::new(Ipv4Addr::LOCALHOST, address.port()), key);
    engine.save(&config_path)?;
    info!(operation = "start_block_source", %address);
    Ok(Some(RunningBlockSource(tokio::spawn(async move {
        if let Err(error) = source.serve_listener(listener, secret).await {
            warn!(%error, "block source stopped");
        }
    }))))
}

/// Fills a buffer with operating-system randomness.
fn getrandom(buffer: &mut [u8; 32]) -> std::io::Result<()> {
    use std::io::Read;
    fs::File::open("/dev/urandom")?.read_exact(buffer)
}

/// An observation belongs to one suspended engine run, never a later restart.
pub(crate) fn invalidate_observation(node: &NodeLayout) -> Result<()> {
    match fs::remove_file(node.root.join("godmode-head.json")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use tempfile::TempDir;

    use super::*;
    use crate::ton::{global_config::GlobalConfig, tools::types::ZeroStateId};

    fn network() -> (TempDir, Layout) {
        let root = TempDir::new().unwrap();
        let layout = Layout::new(root.path().to_path_buf());
        layout.create_dirs().unwrap();
        GlobalConfig::local(
            ZeroStateId::new(
                TonBlockHash::from_bytes([1; 32]),
                TonBlockHash::from_bytes([2; 32]),
            ),
            Vec::new(),
            Ipv4Addr::LOCALHOST,
            18_004,
            TonPublicKey::from_bytes([3; 32]),
        )
        .save_atomic(&layout.global_config)
        .unwrap();
        (root, layout)
    }

    fn planned(workchain: i32, seqno: u32, with_proof: bool) -> PlannedBlock {
        PlannedBlock {
            workchain,
            shard: 0x8000_0000_0000_0000,
            seqno,
            root_hash: TonBlockHash::from_bytes([4; 32]),
            file_hash: TonBlockHash::from_bytes([5; 32]),
            block: STANDARD.encode(b"block"),
            proof: with_proof.then(|| STANDARD.encode(b"proof")),
        }
    }

    #[test]
    fn masterchain_block_is_written_to_static_storage_and_registered() {
        let (_root, layout) = network();
        let plan = HardforkPlan {
            masterchain: planned(MASTERCHAIN_ID, 12, false),
            shard_blocks: Vec::new(),
        };

        install_unchecked(
            &layout,
            &plan,
            BlockSourceEndpoint {
                port: 4443,
                key: TonPublicKey::from_bytes([6; 32]),
            },
        )
        .unwrap();

        let static_file = layout
            .node
            .db
            .join("static")
            .join(plan.masterchain.file_hash.to_static_state_filename());
        assert_eq!(fs::read(static_file).unwrap(), b"block");
        assert_eq!(
            GlobalConfig::load(&layout.global_config)
                .unwrap()
                .hardfork_seqnos(),
            vec![12]
        );
        // Nothing was staged, so the node keeps talking to the public overlays.
        assert!(staged_blocks(&layout.node).unwrap().is_empty());
    }

    #[test]
    fn a_hardfork_for_a_shard_block_is_refused() {
        let (_root, layout) = network();
        let plan = HardforkPlan {
            masterchain: planned(0, 12, false),
            shard_blocks: Vec::new(),
        };

        let error = install(
            &layout,
            &plan,
            BlockSourceEndpoint {
                port: 4443,
                key: TonPublicKey::from_bytes([6; 32]),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("masterchain block"), "{error}");
    }

    #[test]
    fn a_shard_block_without_a_proof_link_is_refused() {
        let (_root, layout) = network();
        let plan = HardforkPlan {
            masterchain: planned(MASTERCHAIN_ID, 12, false),
            shard_blocks: vec![planned(0, 11, false)],
        };

        let error = install_unchecked(
            &layout,
            &plan,
            BlockSourceEndpoint {
                port: 4443,
                key: TonPublicKey::from_bytes([6; 32]),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("proof link"), "{error}");
    }

    #[test]
    fn block_source_identity_survives_reuse() {
        let (_root, layout) = network();
        let (secret, key) = source_identity(&layout.node).unwrap();
        let (again, same_key) = source_identity(&layout.node).unwrap();

        assert_eq!(secret, again);
        assert_eq!(key.to_base64(), same_key.to_base64());
    }

    #[test]
    fn resuming_without_a_suspended_graft_changes_nothing() {
        let (_root, layout) = network();
        assert!(!resume_validation(&layout.node).unwrap());
    }
}
