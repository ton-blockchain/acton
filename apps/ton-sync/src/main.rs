mod sync;

use std::{net::SocketAddrV4, num::NonZeroU64, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use ton_indexer_p2p::start;
use ton_p2p::{
    ClientOptions, NetworkConfig, NetworkOptions, benchmark_peers, bootstrap, load_identity,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(about = "Discover TON peers and synchronize masterchain and shard blocks over P2P")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Discover a masterchain peer and query its full-node capabilities
    Bootstrap(NetworkArgs),
    /// Measure all discovered peers and save a portable connection/ranking profile
    BenchmarkPeers {
        #[command(flatten)]
        network: NetworkArgs,
        #[arg(long)]
        output: PathBuf,
        /// Seed discovery with a previously exported profile
        #[arg(long)]
        peers_file: Option<PathBuf>,
        #[arg(long, default_value = "20")]
        discovery_seconds: NonZeroU64,
        /// Deadline for a complete measured operation on one peer
        #[arg(long, default_value = "3")]
        probe_timeout: NonZeroU64,
        /// Measured requests per peer, after one unmeasured warm-up
        #[arg(long, default_value = "3", value_parser = clap::value_parser!(u16).range(1..=100))]
        samples: u16,
        #[arg(long, default_value = "32", value_parser = clap::value_parser!(u16).range(1..=128))]
        parallel: u16,
    },
    /// Download complete masterchain/shard batches, resuming local progress
    Sync {
        #[command(flatten)]
        network: NetworkArgs,
        /// Stop after this masterchain seqno; otherwise follow until Ctrl-C
        #[arg(long)]
        to_seqno: Option<u32>,
        /// Download only masterchain blocks and proofs
        #[arg(long)]
        masterchain_only: bool,
        /// Start a new directory near the head, using LiteServer only for its block ID
        #[arg(long)]
        from_latest: bool,
        /// Import connection descriptors and calibrated request latencies
        #[arg(long)]
        peers_file: Option<PathBuf>,
        /// Maximum concurrent shard downloads
        #[arg(long, default_value = "16", value_parser = clap::value_parser!(u16).range(1..=128))]
        parallel: u16,
    },
}

#[derive(Args)]
struct NetworkArgs {
    #[arg(long)]
    global_config: PathBuf,
    /// Reachable IPv4 address and UDP port advertised to peers
    #[arg(long, default_value = "127.0.0.1:0")]
    address: SocketAddrV4,
    #[arg(long, default_value = ".ton-sync")]
    data_dir: PathBuf,
    /// Network request deadline in seconds; total deadline for bootstrap
    #[arg(long, default_value = "30")]
    timeout: NonZeroU64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("ton_p2p=info,ton_indexer_p2p=info,ton_sync=info")
            }),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let args = match &cli.command {
        Command::Bootstrap(network)
        | Command::Sync { network, .. }
        | Command::BenchmarkPeers { network, .. } => network,
    };
    let mut config = NetworkConfig::load(&args.global_config)?;
    let options = NetworkOptions {
        address: args.address,
        secret_key: load_identity(&args.data_dir)?,
        timeout: Duration::from_secs(args.timeout.get()),
    };
    let started = std::time::Instant::now();
    let data_dir = args.data_dir.clone();
    let global_config = args.global_config.clone();
    let command = async {
        match cli.command {
            Command::Bootstrap(_) => {
                let report = bootstrap(&config, options).await?;
                serde_json::to_string_pretty(&report).context("cannot encode bootstrap report")
            }
            Command::BenchmarkPeers {
                output,
                peers_file,
                discovery_seconds,
                probe_timeout,
                samples,
                parallel,
                ..
            } => {
                let report = benchmark_peers(
                    &config,
                    ClientOptions {
                        network: NetworkOptions {
                            timeout: Duration::from_secs(probe_timeout.get()),
                            ..options
                        },
                        data_dir: data_dir.clone(),
                        peers_file,
                        parallelism: usize::from(parallel),
                    },
                    &output,
                    Duration::from_secs(discovery_seconds.get()),
                    usize::from(samples),
                )
                .await?;
                serde_json::to_string_pretty(&report).context("cannot encode calibration report")
            }
            Command::Sync {
                to_seqno,
                masterchain_only,
                from_latest,
                parallel,
                peers_file,
                ..
            } => {
                if from_latest {
                    start::use_latest_block(
                        &mut config,
                        &global_config,
                        &data_dir,
                        options.timeout,
                    )
                    .await?;
                }

                let report = sync::run(
                    &config,
                    ClientOptions {
                        network: options,
                        data_dir: data_dir.clone(),
                        peers_file,
                        parallelism: usize::from(parallel),
                    },
                    to_seqno,
                    masterchain_only,
                )
                .await?;
                serde_json::to_string_pretty(&report)
                    .context("cannot encode synchronization report")
            }
        }
    };

    tokio::select! {
        result = command => {
            println!("{}", result?);
        }
        result = tokio::signal::ctrl_c() => {
            result?;
            info!(
                operation = "p2p_command",
                target = %data_dir.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "cancelled",
                "P2P command interrupted",
            );
        }
    }

    Ok(())
}
