//! Calibrates a portable peer profile using validated block downloads.

use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use service_pool::{Failure, Options, Pool};
use tokio::time::{Instant, sleep, timeout_at};
use tracing::{info, warn};
use tycho_types::{boc::Boc, models::Block};

use crate::{
    ClientOptions, NetworkConfig,
    download::{download_from_peer, download_shard_from_peer, pool_failure},
    network::Network,
    peers,
};

/// Summary of a completed calibration. The profile contains public connection
/// descriptors and separate measurements for masterchain and shard operations.
#[derive(Serialize)]
pub struct BenchmarkReport {
    pub peers: usize,
    pub masterchain_measured: usize,
    pub shard_measured: usize,
    pub duration_ms: u128,
}

/// Discovers peers for a bounded interval, then measures every discovered peer.
///
/// All peers receive the same block requests. One warm-up precedes the measured
/// samples, so connection setup does not dominate the exported arithmetic mean.
/// Downloads are validated and discarded; the block checkpoint is never changed.
/// The caller owns the UDP address and must supply a reachable advertised route.
pub async fn benchmark_peers(
    config: &NetworkConfig,
    options: ClientOptions,
    output: &Path,
    discovery_duration: Duration,
    samples: usize,
) -> Result<BenchmarkReport> {
    ensure!(samples > 0, "sample count must be positive");
    ensure!(
        !discovery_duration.is_zero(),
        "discovery duration must be positive"
    );
    ensure!(
        !options.network.timeout.is_zero(),
        "probe timeout must be positive"
    );
    ensure!(
        (1..=128).contains(&options.parallelism),
        "parallelism must be between 1 and 128"
    );
    let started = Instant::now();
    let network = Arc::new(Network::new(config, &options.network)?);
    let known = peers::open(&network, &options)?;
    let pool = Pool::new(
        format!("ton-p2p:{}", network.overlay_id),
        Options {
            max_in_flight: options.parallelism,
            attempt_timeout: options.network.timeout,
            request_timeout: options.network.timeout,
            ..Options::default()
        },
    )?;
    for endpoint in known.endpoints() {
        pool.upsert(endpoint.peer.clone().into());
    }

    let until = Instant::now() + discovery_duration;
    while Instant::now() < until {
        match timeout_at(until, network.find_peers()).await {
            Ok(Ok(found)) => {
                for peer in found {
                    pool.upsert(peer.into());
                }
                info!(
                    operation = "p2p_calibration_discovery",
                    target = %network.overlay_id,
                    peers = pool.len(),
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "discovering",
                    "collecting peers for calibration",
                );
            }
            Ok(Err(error)) => warn!(
                operation = "p2p_calibration_discovery",
                target = %network.overlay_id,
                error = %format!("{error:#}"),
                outcome = "retry",
                "peer discovery failed during calibration",
            ),
            Err(_) => break,
        }
        tokio::select! {
            () = sleep(Duration::from_millis(250)) => {}
            () = tokio::time::sleep_until(until) => break,
        }
    }
    ensure!(!pool.is_empty(), "no peers discovered for calibration");

    let anchor = config.initial_block();
    let deadline = options.network.timeout;
    let request = |endpoint: Arc<peers::PeerEndpoint>| {
        let network = Arc::clone(&network);
        async move {
            download_from_peer(&network, &endpoint.peer, &anchor, Some(&anchor), deadline)
                .await
                .map_err(pool_failure)?
                .ok_or_else(|| Failure::Unavailable(format!("successor of {anchor}")))
        }
    };
    let (id, boc, _) = pool.execute("masterchain", &request).await?;
    let block = Boc::decode(&boc)?.parse::<Block>()?;
    let custom = block
        .load_extra()?
        .load_custom()?
        .context("masterchain block has no shard commitments")?;
    let shard = custom
        .shards
        .latest_blocks()
        .next()
        .transpose()?
        .filter(|id| id.seqno > 0);

    pool.measure_all("masterchain", samples, request).await?;
    // Keep completed measurements if the command is interrupted during shard probing.
    peers::export_profile(&network, &pool, output, id, shard, samples).await?;
    if let Some(shard) = shard {
        pool.measure_all("shard", samples, |endpoint| {
            let network = Arc::clone(&network);
            async move { download_shard_from_peer(&network, &endpoint.peer, shard, deadline).await }
        })
        .await?;
    }
    peers::export_profile(&network, &pool, output, id, shard, samples).await?;

    let snapshot = pool.snapshot();
    let measured = |class: &str| {
        snapshot
            .endpoints
            .iter()
            .filter(|endpoint| {
                endpoint
                    .classes
                    .get(class)
                    .is_some_and(|metrics| metrics.latency_ms.is_some())
            })
            .count()
    };
    let report = BenchmarkReport {
        peers: pool.len(),
        masterchain_measured: measured("masterchain"),
        shard_measured: measured("shard"),
        duration_ms: started.elapsed().as_millis(),
    };
    info!(
        operation = "p2p_calibration",
        target = %output.display(),
        peers = report.peers,
        masterchain_measured = report.masterchain_measured,
        shard_measured = report.shard_measured,
        duration_ms = report.duration_ms,
        outcome = "stored",
        "saved calibrated peer profile",
    );
    Ok(report)
}
