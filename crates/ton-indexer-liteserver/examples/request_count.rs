//! Measures the `LiteServer` requests needed to build canonical batches.
//!
//! Run with:
//! `cargo run -p ton-indexer-liteserver --example request_count -- [BATCHES] [END_SEQNO] [PARALLELISM] [CONFIG]`

use std::{env, error::Error, time::Instant};

use ton_indexer_liteserver::{CanonicalBlockSource, LiteRequestStats, TonutilsLiteClient};

const MAINNET_CONFIG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/mainnet-global.config.json"
);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let batch_count = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(2);
    let requested_end_seqno = args.next().map(|value| value.parse::<u32>()).transpose()?;
    let parallelism = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(4);
    let config = args.next().unwrap_or_else(|| MAINNET_CONFIG.to_owned());
    if args.next().is_some() {
        return Err("usage: request_count [BATCHES] [END_SEQNO] [PARALLELISM] [CONFIG]".into());
    }
    if batch_count == 0 {
        return Err("batch count must be greater than zero".into());
    }

    println!("config: {config}");
    println!("connecting to the first responsive configured liteserver...");
    let connect_started = Instant::now();
    let mut client =
        TonutilsLiteClient::connect_path_with_parallelism(&config, parallelism).await?;
    print_stats(
        "connect probe",
        client.request_stats(),
        connect_started.elapsed(),
    );
    println!(
        "exact block parallelism: {}",
        client.exact_block_parallelism()
    );

    let before_tip = client.request_stats();
    let tip_started = Instant::now();
    let tip = client.latest().await?;
    print_stats(
        "tip lookup",
        client.request_stats().since(before_tip),
        tip_started.elapsed(),
    );

    let end_seqno = requested_end_seqno.unwrap_or(tip.seqno);
    if end_seqno > tip.seqno {
        return Err(format!(
            "requested end seqno {end_seqno} is ahead of current tip {}",
            tip.seqno
        )
        .into());
    }
    let history = u32::try_from(batch_count - 1)?;
    let start_seqno = end_seqno
        .checked_sub(history)
        .ok_or("requested range starts before the masterchain zerostate")?;
    println!("measuring {batch_count} batch(es), masterchain {start_seqno}..={end_seqno}",);
    println!();
    println!(
        "{:<14} {:>10} {:>8} {:>8} {:>8} {:>8} {:>10}",
        "phase", "mc seqno", "shards", "total", "mc-info", "lookup", "get-block"
    );

    let before_indexing = client.request_stats();
    let indexing_started = Instant::now();
    let mut source = CanonicalBlockSource::new(client, start_seqno);
    let mut checkpoint = None;
    for index in 0..batch_count {
        let before = source.client().request_stats();
        let started = Instant::now();
        let batch = source
            .next_batch(checkpoint.as_ref())
            .await?
            .ok_or("liteserver tip moved behind the requested range")?;
        let elapsed = started.elapsed();
        let stats = source.client().request_stats().since(before);
        let next_checkpoint = batch.checkpoint();
        let phase = if index == 0 {
            "cold batch"
        } else {
            "warm batch"
        };

        println!(
            "{phase:<14} {:>10} {:>8} {:>8} {:>8} {:>8} {:>10}  {:>7.1} ms",
            next_checkpoint.seqno,
            batch.shards().len(),
            stats.total(),
            stats.get_masterchain_info(),
            stats.lookup_block(),
            stats.get_block(),
            elapsed.as_secs_f64() * 1_000.0,
        );
        checkpoint = Some(next_checkpoint);
    }

    println!();
    print_stats(
        "indexing only",
        source.client().request_stats().since(before_indexing),
        indexing_started.elapsed(),
    );
    print_stats(
        "whole run",
        source.client().request_stats(),
        connect_started.elapsed(),
    );
    Ok(())
}

fn print_stats(label: &str, stats: LiteRequestStats, elapsed: std::time::Duration) {
    println!(
        "{label}: {} requests (getMasterchainInfo {}, lookupBlock {}, getBlock {}) in {:.1} ms",
        stats.total(),
        stats.get_masterchain_info(),
        stats.lookup_block(),
        stats.get_block(),
        elapsed.as_secs_f64() * 1_000.0,
    );
}
