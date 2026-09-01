use std::time::{Duration, Instant};

use anyhow::Result;
use indicatif::BinaryBytes;
use tracing::{info, warn};

use crate::{
    storage::{Layout, NodeSettings, RuntimeState},
    ton::{
        lite::LocalLiteClient,
        toolchain::Toolchain,
        tools::{
            types::{OperationContext, TonPublicKey},
            validator_console::ValidatorSynchronization,
        },
    },
};

const READY_CONFIRMATIONS: usize = 3;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const LOG_INTERVAL: Duration = Duration::from_secs(5);
const STATS_TIMEOUT: Duration = Duration::from_secs(3);
const LAG_TOLERANCE_BLOCKS: u32 = 2;

/// Authenticated endpoint of the liteserver owned by this join invocation.
pub(super) struct LocalLiteserver {
    pub(super) port: u16,
    pub(super) public_key: TonPublicKey,
}

/// Waits until the local liteserver remains close to the public network head.
///
/// Consecutive confirmations prevent a transient near-head response from marking the
/// node ready. Before the liteserver answers, validator-console statistics provide
/// protocol-specific initial-sync progress for logs and runtime state.
pub(super) async fn wait_for_network_sync(
    layout: &Layout,
    toolchain: &Toolchain,
    node: &NodeSettings,
    liteserver: &LocalLiteserver,
) -> Result<u32> {
    let mut confirmations = 0;
    let mut last_log = Instant::now()
        .checked_sub(LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    let console_endpoint = toolchain.validator_console_endpoint(&layout.node, node);

    loop {
        // Compare the remote network head with the same node through its private
        // local liteserver endpoint. Both clients are recreated because an endpoint
        // may become available while validator-engine is still initializing.
        let sample: Result<(u32, u32)> = tokio::try_join!(
            async {
                let mut network = LocalLiteClient::connect(&layout.global_config).await?;
                network.last().await.map(|block| block.seqno)
            },
            async {
                let mut local =
                    LocalLiteClient::connect_node(liteserver.port, liteserver.public_key).await?;
                local.last().await.map(|block| block.seqno)
            },
        );

        match sample {
            Ok((network_head, local_head)) => {
                let lag = network_head.saturating_sub(local_head);

                if last_log.elapsed() >= LOG_INTERVAL {
                    if let Err(error) = RuntimeState::update_atomic(&layout.runtime, |runtime| {
                        runtime.node.observe_sync_progress(local_head, network_head);
                        Ok(())
                    }) {
                        warn!(node = node.name, %error, "could not publish joined node synchronization progress");
                    }

                    info!(
                        node = node.name,
                        local_head,
                        network_head,
                        lag_blocks = lag,
                        "joined node synchronization progress"
                    );
                    last_log = Instant::now();
                }

                if lag <= LAG_TOLERANCE_BLOCKS {
                    confirmations += 1;

                    if confirmations >= READY_CONFIRMATIONS {
                        if let Err(error) =
                            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                runtime.node.observe_sync_progress(local_head, network_head);
                                Ok(())
                            })
                        {
                            warn!(node = node.name, %error, "could not publish final joined node synchronization progress");
                        }

                        info!(
                            node = node.name,
                            local_head,
                            network_head,
                            lag_blocks = lag,
                            "joined node synchronized"
                        );

                        return Ok(local_head);
                    }
                } else {
                    confirmations = 0;
                }
            }
            Err(error) => {
                confirmations = 0;

                if last_log.elapsed() >= LOG_INTERVAL {
                    // The local liteserver normally rejects queries during initial
                    // state download, so fall back to validator-console statistics.
                    let stats = toolchain
                        .validator_console_tool
                        .health(
                            &OperationContext::for_node(STATS_TIMEOUT, &node.name),
                            &console_endpoint,
                        )
                        .await;

                    match stats.and_then(|stats| stats.synchronization()) {
                        Ok(ValidatorSynchronization::BlockTime {
                            block_time,
                            target_time,
                        }) => {
                            if let Err(publish_error) =
                                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                    runtime
                                        .node
                                        .observe_sync_time_progress(block_time, target_time);
                                    Ok(())
                                })
                            {
                                warn!(node = node.name, %publish_error, "could not publish time-based joined node synchronization progress");
                            }

                            info!(
                                node = node.name,
                                masterchain_block_time = block_time,
                                target_time,
                                lag_seconds = target_time.saturating_sub(block_time),
                                "joined node synchronization progress"
                            );
                        }
                        Ok(ValidatorSynchronization::Initial(progress)) => {
                            let state_download = progress.state_download.as_ref();
                            if let Err(publish_error) =
                                RuntimeState::update_atomic(&layout.runtime, |runtime| {
                                    runtime.node.observe_initial_sync_progress(progress.clone());
                                    Ok(())
                                })
                            {
                                warn!(node = node.name, %publish_error, "could not publish initial joined node synchronization progress");
                            }

                            if let Some(download) = state_download {
                                info!(
                                    node = node.name,
                                    stage = ?progress.stage,
                                    masterchain_seqno = ?progress.masterchain_seqno,
                                    current_part = ?progress.current_part,
                                    total_parts = ?progress.total_parts,
                                    downloaded = %BinaryBytes(download.downloaded_bytes),
                                    total = %BinaryBytes(download.total_bytes),
                                    speed = %format!("{}/s", BinaryBytes(download.bytes_per_second)),
                                    eta_seconds = download.remaining_seconds,
                                    "joined node initial synchronization progress"
                                );
                            } else {
                                info!(
                                    node = node.name,
                                    stage = ?progress.stage,
                                    masterchain_seqno = ?progress.masterchain_seqno,
                                    current_part = ?progress.current_part,
                                    total_parts = ?progress.total_parts,
                                    "joined node initial synchronization progress"
                                );
                            }
                        }
                        Ok(ValidatorSynchronization::WaitingForMasterchain) => info!(
                            node = node.name,
                            "joined node is preparing its first masterchain block"
                        ),
                        Err(stats_error) => {
                            warn!(
                                node = node.name,
                                liteserver_error = %format!("{error:#}"),
                                validator_stats_error = %format!("{stats_error:#}"),
                                "could not measure joined node synchronization"
                            );
                        }
                    }
                    last_log = Instant::now();
                }
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
