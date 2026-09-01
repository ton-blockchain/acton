//! Reads TON network state independently of signed host telemetry.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use anyhow::{Context, Result};
use tokio::{sync::watch, time::MissedTickBehavior};
use ton::{block_tlb::Block, ton_core::traits::tlb::TLB};
use tracing::warn;

use crate::{
    observability::{
        ChainHead, ElectionObservation, ElectionStage, MasterchainBlock, ProductionView, ShardHead,
        ValidatorObservation, ValidatorSetObservation, VerifiedNetworkState,
    },
    operations::validators,
    storage::{Layout, RuntimeState, Settings, unix_time},
    ton::{
        lite::{BlockRef, LocalLiteClient},
        toolchain::Toolchain,
        tools::lite_client::ValidatorSetInfo,
    },
};

const INITIAL_BACKFILL_BLOCKS: u32 = 64;
const MAX_CATCHUP_BLOCKS_PER_TICK: u32 = 128;
const MAX_RETAINED_BLOCKS: usize = 20_000;
const ELECTION_POLL_INTERVAL_SECONDS: u64 = 15;

/// Host-local liteserver position with the times needed for progress and lag checks.
#[derive(Clone, Copy)]
pub(super) struct NodeHeadSample {
    pub(super) seqno: u32,
    pub(super) observed_at: u64,
    pub(super) progressed_at: u64,
}

/// Continuously refreshes the TON state used by the dashboard.
///
/// A genesis instance also publishes this verified head as its local node sample.
/// Both values then come from the same liteserver response instead of two requests
/// that can observe opposite sides of a block boundary.
pub(super) async fn collection_loop(
    toolchain: Toolchain,
    interval_seconds: u64,
    block_window_seconds: u64,
    updates: watch::Sender<Option<VerifiedNetworkState>>,
    local_node_updates: Option<watch::Sender<Option<NodeHeadSample>>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut reader = NetworkReader::default();
    let mut latest_local_head: Option<NodeHeadSample> = None;
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let now = unix_time();
                if let Err(error) = reader.update(&toolchain, now, block_window_seconds).await {
                    warn!(%error, "TON network observation update failed");
                }

                updates.send_replace(reader.network.clone());

                if let Some(local_node_updates) = &local_node_updates {
                    let sample = reader.network.as_ref().map(|network| {
                        let observed_at = network.head.observed_at;
                        let progressed_at = latest_local_head.map_or(observed_at, |previous| {
                            if network.head.seqno > previous.seqno {
                                observed_at
                            } else {
                                previous.progressed_at
                            }
                        });

                        NodeHeadSample {
                            seqno: network.head.seqno,
                            observed_at,
                            progressed_at,
                        }
                    });
                    latest_local_head = sample;
                    local_node_updates.send_replace(sample);
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

/// Samples host-local progress without waiting for the network reader.
pub(super) async fn node_collection_loop(
    layout: Layout,
    interval_seconds: u64,
    updates: watch::Sender<Option<NodeHeadSample>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut latest: Option<NodeHeadSample> = None;
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match collect_node_head(&layout).await {
                    Ok(Some(seqno)) => {
                        let observed_at = unix_time();
                        let progressed_at = latest.map_or(observed_at, |previous| {
                            if seqno > previous.seqno {
                                observed_at
                            } else {
                                previous.progressed_at
                            }
                        });
                        let sample = NodeHeadSample {
                            seqno,
                            observed_at,
                            progressed_at,
                        };
                        latest = Some(sample);
                        updates.send_replace(Some(sample));
                    }
                    Ok(None) => {
                        latest = None;
                        updates.send_replace(None);
                    }
                    Err(error) => warn!(%error, "local node head observation update failed"),
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

/// Reads one head from the liteserver owned by this state directory.
async fn collect_node_head(layout: &Layout) -> Result<Option<u32>> {
    let settings = Settings::load(&layout.settings)?;
    let runtime = RuntimeState::load(&layout.runtime)?;
    if !runtime.node.running {
        return Ok(None);
    }
    let Some(public_key) = runtime.node.liteserver_public_key else {
        return Ok(None);
    };

    let mut client =
        LocalLiteClient::connect_node(settings.node.liteserver_port, public_key).await?;
    Ok(Some(client.last().await?.seqno))
}

#[derive(Debug, Clone)]
struct BlockObservation {
    id: String,
    workchain: i32,
    seqno: u32,
    gen_utime: u32,
    creator: String,
}

#[derive(Default)]
struct NetworkReader {
    last_scanned_seqno: Option<u32>,
    last_election_update: Option<u64>,
    blocks: BTreeMap<String, BlockObservation>,
    election: Option<ElectionObservation>,
    current_validator_keys: Option<BTreeSet<String>>,
    next_validator_keys: Option<BTreeSet<String>>,
    network: Option<VerifiedNetworkState>,
}

impl NetworkReader {
    /// Advances the rolling chain window without replacing the last complete view on error.
    async fn update(&mut self, toolchain: &Toolchain, now: u64, window_seconds: u64) -> Result<()> {
        let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
        let network_head = client.last().await?;

        let validator_round_ended = self
            .election
            .as_ref()
            .is_some_and(|election| now >= u64::from(election.current.validation_ended_at));
        if validator_round_ended
            || self
                .last_election_update
                .is_none_or(|updated| now.saturating_sub(updated) >= ELECTION_POLL_INTERVAL_SECONDS)
        {
            self.last_election_update = Some(now);
            match validators::election_status(toolchain).await {
                Ok(info) => {
                    self.current_validator_keys = Some(
                        info.current
                            .validators
                            .iter()
                            .map(|validator| validator.public_key.clone())
                            .collect::<BTreeSet<_>>(),
                    );
                    self.next_validator_keys = info.next.as_ref().map(|set| {
                        set.validators
                            .iter()
                            .map(|validator| validator.public_key.clone())
                            .collect::<BTreeSet<_>>()
                    });

                    let elections_open_at = info
                        .current
                        .until
                        .saturating_sub(info.elections_start_before);
                    let elections_close_at =
                        info.current.until.saturating_sub(info.elections_end_before);
                    let next_set_activation_at = info.current.until;
                    let next_set_available = info.next.is_some();
                    self.election = Some(ElectionObservation {
                        stage: election_stage(
                            now,
                            elections_open_at,
                            elections_close_at,
                            next_set_activation_at,
                            next_set_available,
                        ),
                        elections_open_at,
                        elections_close_at,
                        validators_elected_for: info.validators_elected_for,
                        stake_held_for: info.stake_held_for,
                        previous: info.previous.as_ref().map(validator_set_observation),
                        current: validator_set_observation(&info.current),
                        next: info.next.as_ref().map(validator_set_observation),
                    });
                }
                Err(error) => warn!(
                    error = %format_args!("{error:#}"),
                    "election observation update failed"
                ),
            }
        }

        if let Some(election) = &mut self.election {
            election.stage = election_stage(
                now,
                election.elections_open_at,
                election.elections_close_at,
                election.current.validation_ended_at,
                election.next.is_some(),
            );
        }

        let first = self.last_scanned_seqno.map_or_else(
            || {
                network_head
                    .seqno
                    .saturating_sub(INITIAL_BACKFILL_BLOCKS - 1)
                    .max(1)
            },
            |seqno| seqno.saturating_add(1).min(network_head.seqno),
        );
        let last = network_head
            .seqno
            .min(first.saturating_add(MAX_CATCHUP_BLOCKS_PER_TICK - 1));
        let mut latest = None;

        for seqno in first..=last {
            let (id, bytes) = match client.block(-1, "8000000000000000", seqno).await {
                Ok(block) => block,
                Err(error) => {
                    warn!(seqno, %error, "masterchain block observation skipped");
                    continue;
                }
            };
            let block = match parse_block(&id, bytes) {
                Ok(block) => block,
                Err(error) => {
                    warn!(seqno, %error, "invalid masterchain block observation skipped");
                    continue;
                }
            };

            let shard_ids = block
                .1
                .extra
                .mc_block_extra
                .as_ref()
                .map(|extra| extra.shard_ids())
                .unwrap_or_default();
            let shards = block
                .1
                .extra
                .mc_block_extra
                .as_ref()
                .map(|extra| {
                    extra
                        .shard_hashes
                        .iter()
                        .flat_map(|(workchain, shards)| {
                            shards.iter().map(|(prefix, shard)| ShardHead {
                                workchain: *workchain,
                                shard: format!("{:016x}", prefix.to_shard()),
                                seqno: shard.seqno,
                                root_hash: hex::encode(shard.root_hash.as_slice_sized()),
                                file_hash: hex::encode(shard.file_hash.as_slice_sized()),
                                gen_utime: shard.gen_utime,
                                before_split: shard.before_split,
                                before_merge: shard.before_merge,
                                want_split: shard.want_split,
                                want_merge: shard.want_merge,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            self.blocks.insert(block.0.id.clone(), block.0.clone());
            latest = Some((id.clone(), block.0.gen_utime, shards));

            for shard_id in shard_ids {
                if shard_id.seqno == 0 {
                    continue;
                }

                let shard = format!("{:016x}", shard_id.shard_ident.shard);
                let key = format!(
                    "{}:{shard}:{}",
                    shard_id.shard_ident.workchain, shard_id.seqno
                );
                if self.blocks.contains_key(&key) {
                    continue;
                }

                let (id, bytes) = match client
                    .block(shard_id.shard_ident.workchain, &shard, shard_id.seqno)
                    .await
                {
                    Ok(block) => block,
                    Err(error) => {
                        warn!(
                            workchain = shard_id.shard_ident.workchain,
                            shard,
                            seqno = shard_id.seqno,
                            %error,
                            "shard block observation skipped"
                        );
                        continue;
                    }
                };
                let parsed = match parse_block(&id, bytes) {
                    Ok(block) => block.0,
                    Err(error) => {
                        warn!(
                            workchain = shard_id.shard_ident.workchain,
                            shard,
                            seqno = shard_id.seqno,
                            %error,
                            "invalid shard block observation skipped"
                        );
                        continue;
                    }
                };
                self.blocks.insert(parsed.id.clone(), parsed);
            }

            self.last_scanned_seqno = Some(seqno);
        }

        let cutoff = now.saturating_sub(window_seconds);
        self.blocks
            .retain(|_, block| u64::from(block.gen_utime) >= cutoff);
        while self.blocks.len() > MAX_RETAINED_BLOCKS {
            let Some(first) = self.blocks.keys().next().cloned() else {
                break;
            };
            self.blocks.remove(&first);
        }

        if let Some((head, gen_utime, shards)) = latest {
            let mut production = BTreeMap::<String, ProductionView>::new();
            for block in self.blocks.values() {
                let entry =
                    production
                        .entry(block.creator.clone())
                        .or_insert_with(|| ProductionView {
                            creator: block.creator.clone(),
                            masterchain_blocks: 0,
                            shard_blocks: 0,
                            last_block_at: 0,
                        });
                if block.workchain == -1 {
                    entry.masterchain_blocks = entry.masterchain_blocks.saturating_add(1);
                } else {
                    entry.shard_blocks = entry.shard_blocks.saturating_add(1);
                }
                entry.last_block_at = entry.last_block_at.max(block.gen_utime);
            }

            self.network = Some(VerifiedNetworkState {
                head: ChainHead {
                    seqno: head.seqno,
                    root_hash: head.root_hash,
                    file_hash: head.file_hash,
                    gen_utime,
                    observed_at: unix_time(),
                    shard_count: shards.len(),
                },
                masterchain_history: self
                    .blocks
                    .values()
                    .filter(|block| block.workchain == -1)
                    .map(|block| MasterchainBlock {
                        seqno: block.seqno,
                        gen_utime: block.gen_utime,
                    })
                    .collect(),
                shards,
                election: self.election.clone(),
                production: production.into_values().collect(),
                current_validator_keys: self.current_validator_keys.clone(),
                next_validator_keys: self.next_validator_keys.clone(),
            });
        }

        Ok(())
    }
}

fn validator_set_observation(set: &ValidatorSetInfo) -> ValidatorSetObservation {
    ValidatorSetObservation {
        round_id: set.since,
        validation_started_at: set.since,
        validation_ended_at: set.until,
        validators: set.validators.len(),
        main_validators: set.main,
        total_weight: set.total_weight.to_string(),
        members: set
            .validators
            .iter()
            .map(|validator| ValidatorObservation {
                public_key: validator.public_key.clone(),
                adnl_address: validator.adnl_address.clone(),
                weight: validator.weight.to_string(),
            })
            .collect(),
    }
}

fn election_stage(
    now: u64,
    elections_open_at: u32,
    elections_close_at: u32,
    next_set_activation_at: u32,
    has_next_set: bool,
) -> ElectionStage {
    if now < u64::from(elections_open_at) {
        ElectionStage::Validation
    } else if now < u64::from(elections_close_at) {
        ElectionStage::AcceptingEntries
    } else if now < u64::from(next_set_activation_at) {
        if has_next_set {
            ElectionStage::NextSetReady
        } else {
            ElectionStage::Finalizing
        }
    } else if has_next_set {
        ElectionStage::ActivationOverdue
    } else {
        ElectionStage::Retrying
    }
}

/// Decodes one liteserver response and keeps only fields required for local aggregation.
fn parse_block(id: &BlockRef, bytes: Vec<u8>) -> Result<(BlockObservation, Block)> {
    let block = Block::from_boc(bytes).context("failed to decode TON block")?;
    let info = &block.info;
    let observation = BlockObservation {
        id: format!("{}:{}:{}", id.workchain, id.shard, id.seqno),
        workchain: id.workchain,
        seqno: id.seqno,
        gen_utime: info.gen_utime,
        creator: hex::encode(block.extra.created_by.as_slice_sized()),
    };
    Ok((observation, block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ton::tools::lite_client::ValidatorSetMemberInfo;

    #[test]
    fn election_without_a_next_set_is_reported_as_retrying() {
        let stages = (
            election_stage(121, 30, 90, 120, false),
            election_stage(121, 30, 90, 120, true),
        );

        expect_test::expect![[r#"["retrying","activation_overdue"]"#]]
            .assert_eq(&serde_json::to_string(&stages).unwrap());
    }

    #[test]
    fn validator_set_observation_preserves_member_identity_and_weight() {
        let set = ValidatorSetInfo {
            since: 100,
            until: 220,
            main: 1,
            total_weight: 30,
            validators: vec![
                ValidatorSetMemberInfo {
                    public_key: hex::encode([1; 32]),
                    adnl_address: Some(hex::encode([2; 32])),
                    weight: 10,
                },
                ValidatorSetMemberInfo {
                    public_key: hex::encode([3; 32]),
                    adnl_address: None,
                    weight: 20,
                },
            ],
        };

        expect_test::expect![[r#"
            {
              "round_id": 100,
              "validation_started_at": 100,
              "validation_ended_at": 220,
              "validators": 2,
              "main_validators": 1,
              "total_weight": "30",
              "members": [
                {
                  "public_key": "0101010101010101010101010101010101010101010101010101010101010101",
                  "adnl_address": "0202020202020202020202020202020202020202020202020202020202020202",
                  "weight": "10"
                },
                {
                  "public_key": "0303030303030303030303030303030303030303030303030303030303030303",
                  "adnl_address": null,
                  "weight": "20"
                }
              ]
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&validator_set_observation(&set)).unwrap());
    }
}
