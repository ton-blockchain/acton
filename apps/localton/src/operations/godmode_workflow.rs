//! Validation and recoverable installation of offline administrator plans.
use super::*;
use crate::ton::lite::{BlockRef as ObservedBlock, LocalLiteClient};
use std::io::Read;
use tycho_types::{
    boc::Boc,
    merkle::MerkleProof,
    models::{Block, BlockProof, PrevBlockRef},
};

#[derive(Serialize, Deserialize)]
struct Observation {
    head: ObservedBlock,
    state_hash: String,
}

pub(super) async fn live_command(layout: &Layout, command: &GodmodeCommand) -> Result<()> {
    let _lock = crate::bootstrap::acquire_lock(&layout.node.root.join("godmode-read.lock"))
        .context("Another administrative state query is in progress")?;
    ensure!(
        !ValidatorEngineConfig::load(&engine_config_path(&layout.node))?.validates(),
        "Restart with suspended validator keys before observing or applying changes"
    );
    let mut client = LocalLiteClient::connect(&layout.node.global_config).await?;
    let sources = client
        .hardfork_sources(&layout.node.root.join("godmode-state-cache"))
        .await?;
    let head = client.last().await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    ensure!(
        client.last().await? == head && head.seqno == sources.masterchain_prev.seqno,
        "Block production is still active"
    );
    match command {
        GodmodeCommand::Observe(_) => {
            let observation = Observation {
                head,
                state_hash: sources.masterchain_state.repr_hash().to_string(),
            };
            write_json_atomic(&layout.node.root.join("godmode-head.json"), &observation)?;
            println!("{}", serde_json::to_string(&observation.head)?);
        }
        GodmodeCommand::Prepare(_) => {
            let mut input = Vec::new();
            std::io::stdin()
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut input)?;
            ensure!(
                input.len() <= 16 * 1024 * 1024,
                "Administrative request is too large"
            );
            let edits: Vec<ton_hardfork::request::AccountEdit> = serde_json::from_slice(&input)?;
            let batch = ton_hardfork::request::account_batch(&sources, &edits)?;
            let now = crate::storage::unix_time()
                .try_into()
                .context("Timestamp overflow")?;
            let built = ton_hardfork::build_hardfork(&sources, now, &batch)?;
            let planned = |b: &ton_hardfork::HardforkBlock| PlannedBlock {
                workchain: b.shard.workchain(),
                shard: b.shard.prefix(),
                seqno: b.seqno,
                root_hash: TonBlockHash::from_bytes(b.root_hash.0),
                file_hash: TonBlockHash::from_bytes(b.file_hash.0),
                block: STANDARD.encode(&b.block_boc),
                proof: Some(STANDARD.encode(&b.proof_link)),
            };
            let plan = HardforkPlan {
                masterchain: planned(&built.masterchain),
                shard_blocks: built.basechain.iter().map(planned).collect(),
            };
            write_json_atomic(
                &layout.node.root.join("godmode-head.json"),
                &Observation {
                    head,
                    state_hash: sources.masterchain_state.repr_hash().to_string(),
                },
            )?;
            println!("{}", serde_json::to_string(&plan)?);
        }
        GodmodeCommand::Verify(_) => {
            let dir = staging_dir(&layout.node);
            let plan: HardforkPlan = serde_json::from_slice(&fs::read(dir.join("plan.json"))?)?;
            ensure!(
                head.seqno == plan.masterchain.seqno
                    && head
                        .root_hash
                        .eq_ignore_ascii_case(&hex::encode(plan.masterchain.root_hash.as_bytes())),
                "Masterchain hardfork has not been applied"
            );
            let block = validate_block(&plan.masterchain, false)?;
            ensure!(
                block.state_update.load()?.new_hash == *sources.masterchain_state.repr_hash(),
                "Masterchain state differs from the plan"
            );
            for shard in &plan.shard_blocks {
                let source = sources
                    .basechain
                    .as_ref()
                    .context("Missing applied basechain state")?;
                ensure!(
                    source.prev.root_hash.0 == *shard.root_hash.as_bytes()
                        && source.prev.seqno == shard.seqno,
                    "Shard hardfork has not been applied"
                );
                ensure!(
                    validate_block(shard, true)?.state_update.load()?.new_hash
                        == *source.state.repr_hash(),
                    "Shard state differs from the plan"
                );
            }
            write_json_atomic(&dir.join("verified.json"), &head)?;
            println!(
                "{}",
                serde_json::json!({"verified": true, "seqno": head.seqno})
            );
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_block(planned: &PlannedBlock, shard: bool) -> Result<Block> {
    let data = decode(&planned.block, "block")?;
    let cell = Boc::decode(&data).context("Invalid block BoC")?;
    ensure!(
        cell.repr_hash().0 == *planned.root_hash.as_bytes()
            && Boc::file_hash(&data).0 == *planned.file_hash.as_bytes(),
        "Block hashes do not match its BoC"
    );
    let block = cell.parse::<Block>()?;
    let info = block.info.load()?;
    ensure!(
        info.shard.workchain() == planned.workchain
            && info.shard.prefix() == planned.shard
            && info.seqno == planned.seqno,
        "Block header does not match the plan"
    );
    ensure!(
        info.prev_vert_ref.is_some(),
        "Expected a vertical hardfork increment"
    );
    ensure!(info.key_block != shard, "Invalid key-block flag");
    let PrevBlockRef::Single(previous) = info.load_prev_ref()? else {
        bail!("Expected one previous block");
    };
    let vertical = info
        .prev_vert_ref
        .as_ref()
        .context("Missing previous vertical block")?
        .load()?;
    ensure!(
        (!shard && previous == vertical || shard)
            && previous.seqno.checked_add(1) == Some(info.seqno),
        "Hardfork predecessor references disagree"
    );
    if shard {
        let proof = Boc::decode(decode(
            planned
                .proof
                .as_ref()
                .context("A shard block needs a proof link")?,
            "proof",
        )?)?
        .parse::<BlockProof>()?;
        ensure!(
            proof.proof_for.root_hash == *cell.repr_hash()
                && proof.proof_for.file_hash.0 == *planned.file_hash.as_bytes()
                && proof.proof_for.seqno == planned.seqno
                && proof.proof_for.shard == info.shard,
            "Proof is for another block"
        );
        ensure!(
            *proof
                .root
                .parse_exotic::<MerkleProof>()?
                .cell
                .virtualize()
                .repr_hash()
                == *cell.repr_hash(),
            "Invalid proof root"
        );
    }
    Ok(block)
}

/// The journal exists only while files are being committed, with the node stopped.
/// Bootstrap rolls back an interrupted commit before starting validator-engine.
#[derive(Serialize, Deserialize)]
struct InstallJournal {
    global: serde_json::Value,
    node_global: serde_json::Value,
    engine: serde_json::Value,
}

pub(crate) fn recover_install(layout: &Layout) -> Result<()> {
    let path = layout.node.root.join("godmode-install.json");
    if !path.exists() {
        return Ok(());
    }
    let saved: InstallJournal = serde_json::from_slice(&fs::read(&path)?)?;
    write_json_atomic(&layout.global_config, &saved.global)?;
    write_json_atomic(&layout.node.global_config, &saved.node_global)?;
    write_json_atomic(&engine_config_path(&layout.node), &saved.engine)?;
    let dir = staging_dir(&layout.node);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::remove_file(path)?;
    Ok(())
}

pub(super) fn install(
    layout: &Layout,
    plan: &HardforkPlan,
    source: BlockSourceEndpoint,
) -> Result<()> {
    ensure!(
        plan.masterchain.workchain == MASTERCHAIN_ID,
        "A hardfork requires a masterchain block"
    );
    let mc = validate_block(&plan.masterchain, false)?;
    let mc_info = mc.info.load()?;
    let PrevBlockRef::Single(mc_previous) = mc_info.load_prev_ref()? else {
        bail!("Expected one masterchain predecessor");
    };
    let extra = mc
        .extra
        .load()?
        .custom
        .context("Masterchain block is missing custom data")?
        .load()?;
    for b in &plan.shard_blocks {
        let shard = validate_block(b, true)?;
        let info = shard.info.load()?;
        ensure!(
            info.vert_seqno == mc_info.vert_seqno
                && info
                    .prev_vert_ref
                    .as_ref()
                    .context("Missing shard vertical reference")?
                    .load()?
                    == mc_previous,
            "Shard vertical reference does not match the masterchain predecessor"
        );
        let mut referenced = false;
        for entry in extra.shards.iter() {
            let (ident, descr) = entry?;
            if ident == info.shard {
                referenced = descr.seqno == b.seqno
                    && descr.root_hash.0 == *b.root_hash.as_bytes()
                    && descr.file_hash.0 == *b.file_hash.as_bytes();
            }
        }
        ensure!(
            referenced,
            "Masterchain does not reference the planned shard block"
        );
    }
    ensure!(
        plan.shard_blocks.len() <= 1,
        "Only one unsplit basechain shard is supported"
    );
    ensure!(
        plan.shard_blocks
            .iter()
            .all(|b| b.workchain == 0 && b.shard == ShardIdent::BASECHAIN.prefix()),
        "Unsupported shard"
    );
    let dir = staging_dir(&layout.node);
    if dir.join("plan.json").exists() {
        let previous: HardforkPlan = serde_json::from_slice(&fs::read(dir.join("plan.json"))?)?;
        ensure!(
            serde_json::to_value(previous)? == serde_json::to_value(plan)?,
            "Finish the pending hardfork before installing another one"
        );
        return Ok(());
    }
    let observed: Observation = serde_json::from_slice(
        &fs::read(layout.node.root.join("godmode-head.json"))
            .context("Observe the suspended node before installing a plan")?,
    )?;
    let info = mc.info.load()?;
    let PrevBlockRef::Single(prev) = info.load_prev_ref()? else {
        bail!("Expected one previous masterchain block");
    };
    ensure!(
        prev.seqno == observed.head.seqno
            && prev
                .root_hash
                .to_string()
                .eq_ignore_ascii_case(&observed.head.root_hash)
            && prev
                .file_hash
                .to_string()
                .eq_ignore_ascii_case(&observed.head.file_hash),
        "Hardfork does not extend the observed node head"
    );
    ensure!(
        info.seqno == prev.seqno.checked_add(1).context("Sequence overflow")?
            && mc
                .state_update
                .load()?
                .old_hash
                .to_string()
                .eq_ignore_ascii_case(&observed.state_hash),
        "Hardfork does not extend the observed state"
    );
    ensure!(
        !ValidatorEngineConfig::load(&engine_config_path(&layout.node))?.validates(),
        "Suspend validation before installing a hardfork"
    );
    let global = GlobalConfig::load(&layout.global_config)?;
    ensure!(
        global
            .hardfork_seqnos()
            .last()
            .is_none_or(|n| *n < info.seqno),
        "Hardfork sequence number is already registered"
    );
    let read_json = |p: &std::path::Path| -> Result<serde_json::Value> {
        Ok(serde_json::from_slice(&fs::read(p)?)?)
    };
    let saved = InstallJournal {
        global: read_json(&layout.global_config)?,
        node_global: read_json(&layout.node.global_config)?,
        engine: read_json(&engine_config_path(&layout.node))?,
    };
    let journal = layout.node.root.join("godmode-install.json");
    write_json_atomic(&journal, &saved)?;
    let result = (|| {
        install_unchecked(layout, plan, source)?;
        write_json_atomic(&dir.join("original-engine.json"), &saved.engine)?;
        let mut local = GlobalConfig::load(&layout.node.global_config)?;
        local.push_hardfork(
            plan.masterchain.seqno,
            plan.masterchain.root_hash,
            plan.masterchain.file_hash,
        );
        local.save_atomic(&layout.node.global_config)?;
        fs::remove_file(&journal)?;
        Ok(())
    })();
    if result.is_err() {
        recover_install(layout)
            .context("Installation failed and rollback could not restore the original files")?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ton::tools::types::ZeroStateId;

    fn fixture() -> (tempfile::TempDir, Layout, HardforkPlan) {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path().to_path_buf());
        layout.create_dirs().unwrap();
        let plan: HardforkPlan =
            serde_json::from_str(include_str!("../../tests/fixtures/hardfork-plan.json")).unwrap();
        let block = validate_block(&plan.masterchain, false).unwrap();
        let PrevBlockRef::Single(prev) = block.info.load().unwrap().load_prev_ref().unwrap() else {
            panic!()
        };
        GlobalConfig::local(
            ZeroStateId::new(
                TonBlockHash::from_bytes([1; 32]),
                TonBlockHash::from_bytes([2; 32]),
            ),
            vec![],
            Ipv4Addr::LOCALHOST,
            18004,
            TonPublicKey::from_bytes([3; 32]),
        )
        .save_atomic(&layout.global_config)
        .unwrap();
        fs::copy(&layout.global_config, &layout.node.global_config).unwrap();
        write_json_atomic(&engine_config_path(&layout.node), &serde_json::json!({
            "@type": "engine.validator.config", "out_port": 3272, "addrs": [], "adnl": [], "dht": [], "validators": [],
            "fullnode": STANDARD.encode([1; 32]), "fullnodeslaves": [], "fullnodemasters": [], "liteservers": [], "control": [], "gc": {"@type": "engine.gc", "ids": []}
        })).unwrap();
        write_json_atomic(
            &layout.node.root.join("godmode-head.json"),
            &Observation {
                head: ObservedBlock {
                    workchain: -1,
                    shard: "8000000000000000".into(),
                    seqno: prev.seqno,
                    root_hash: prev.root_hash.to_string(),
                    file_hash: prev.file_hash.to_string(),
                },
                state_hash: block.state_update.load().unwrap().old_hash.to_string(),
            },
        )
        .unwrap();
        (dir, layout, plan)
    }
    fn endpoint() -> BlockSourceEndpoint {
        BlockSourceEndpoint {
            port: 4443,
            key: TonPublicKey::from_bytes([6; 32]),
        }
    }

    #[test]
    fn validated_install_is_idempotent_and_keeps_pending_blocks_on_rejection() {
        let (_dir, layout, plan) = fixture();
        install(&layout, &plan, endpoint()).unwrap();
        let before = fs::read(staging_dir(&layout.node).join("plan.json")).unwrap();
        install(&layout, &plan, endpoint()).unwrap();
        let mut damaged: HardforkPlan = serde_json::from_slice(&before).unwrap();
        damaged.masterchain.root_hash = TonBlockHash::from_bytes([9; 32]);
        assert!(install(&layout, &damaged, endpoint()).is_err());
        assert_eq!(
            before,
            fs::read(staging_dir(&layout.node).join("plan.json")).unwrap()
        );
        assert_eq!(staged_blocks(&layout.node).unwrap().len(), 1);
        assert!(finish(&layout.node).is_err());
    }

    #[test]
    fn installation_requires_the_observed_head_and_valid_shard_proofs() {
        let (_dir, layout, mut plan) = fixture();
        plan.shard_blocks[0].proof = None;
        assert!(
            install(&layout, &plan, endpoint())
                .unwrap_err()
                .to_string()
                .contains("proof link")
        );
        let (_, _, plan) = fixture();
        let path = layout.node.root.join("godmode-head.json");
        let mut observed: Observation = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        observed.head.seqno += 1;
        write_json_atomic(&path, &observed).unwrap();
        assert!(
            install(&layout, &plan, endpoint())
                .unwrap_err()
                .to_string()
                .contains("observed node head")
        );
        assert!(!staging_dir(&layout.node).exists());
    }

    #[test]
    fn interrupted_install_restores_configuration_before_startup() {
        let (_dir, layout, plan) = fixture();
        let read = |p: &std::path::Path| serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
        let saved = InstallJournal {
            global: read(&layout.global_config),
            node_global: read(&layout.node.global_config),
            engine: read(&engine_config_path(&layout.node)),
        };
        write_json_atomic(&layout.node.root.join("godmode-install.json"), &saved).unwrap();
        install_unchecked(&layout, &plan, endpoint()).unwrap();
        recover_install(&layout).unwrap();
        assert!(
            GlobalConfig::load(&layout.global_config)
                .unwrap()
                .hardfork_seqnos()
                .is_empty()
        );
        assert!(!staging_dir(&layout.node).exists());
        assert_eq!(
            serde_json::to_value(
                ValidatorEngineConfig::load(&engine_config_path(&layout.node)).unwrap()
            )
            .unwrap()["fullnodeslaves"],
            serde_json::json!([])
        );
    }

    #[test]
    fn suspension_blocks_elections_even_without_validator_keys() {
        let (_dir, layout, _) = fixture();
        assert!(suspend_validation(&layout.node).unwrap());
        assert!(!suspend_validation(&layout.node).unwrap());
        assert!(suspended_keys_path(&layout.node).exists());
        assert!(resume_validation(&layout.node).unwrap());
        assert!(!suspended_keys_path(&layout.node).exists());
    }
}
