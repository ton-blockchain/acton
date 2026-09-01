use std::{fs, time::Duration};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{
    cli::HardforkArgs,
    runtime::run_checked,
    ton::lite::{BlockRef, LocalLiteClient},
    ton::toolchain::Toolchain,
};

pub async fn execute(args: HardforkArgs) -> Result<()> {
    let toolchain = Toolchain::resolve(&args.state.state_dir, None).await?;
    let node_layout = &toolchain.layout.node;
    let binary = toolchain
        .binaries
        .optional_command("create-hardfork")
        .context(
            "create-hardfork is not shipped in the official TON release; provide it in TON_BIN_DIR",
        )?;
    let mut client = LocalLiteClient::connect(&node_layout.global_config).await?;
    let source = client.last().await?;
    let mut command = Command::new(binary);
    command
        .args(["-D"])
        .arg(&node_layout.db)
        .args(["-T", &block_id_text(&source)])
        .args(["-w", &format!("{}:{}", source.workchain, source.shard)]);
    if let Some(message) = args.external_message.as_ref() {
        command.args(["-m"]).arg(message);
    }
    let output = run_checked("create-hardfork", command, Duration::from_secs(180)).await?;
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let fork = parse_created_block(&combined)?;
    let output_path = args
        .output
        .unwrap_or_else(|| node_layout.root.join("my-ton-forked.config.json"));
    write_fork_config(&node_layout.global_config, &output_path, &fork)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "source": source,
            "fork": fork,
            "global_config": output_path,
        }))?
    );
    Ok(())
}

fn write_fork_config(
    source: &std::path::Path,
    output: &std::path::Path,
    fork: &BlockRef,
) -> Result<()> {
    let mut config: Value = serde_json::from_slice(
        &fs::read(source).with_context(|| format!("failed to read {}", source.display()))?,
    )?;
    let validator = config
        .get_mut("validator")
        .and_then(Value::as_object_mut)
        .context("global config does not contain a validator object")?;
    validator.insert(
        "hardforks".to_owned(),
        json!([{
            "file_hash": STANDARD.encode(hex::decode(&fork.file_hash)?),
            "seqno": fork.seqno,
            "root_hash": STANDARD.encode(hex::decode(&fork.root_hash)?),
            "workchain": fork.workchain,
            "shard": i64::from_str_radix(&fork.shard, 16)
                .unwrap_or_else(|_| u64::from_str_radix(&fork.shard, 16).unwrap_or_default() as i64),
        }]),
    );
    let parent = output.parent().context("hardfork output has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = output.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&config)?)?;
    fs::rename(&temporary, output)?;
    Ok(())
}

fn parse_created_block(output: &str) -> Result<BlockRef> {
    let expression =
        Regex::new(r"\((-?\d+),([0-9A-Fa-f]+),(\d+)\):([0-9A-Fa-f]{64}):([0-9A-Fa-f]{64})")?;
    let captures = expression
        .captures_iter(output)
        .last()
        .context("create-hardfork output does not contain a created block id")?;
    if !output.contains("created block") && !output.contains("success, block") {
        bail!("create-hardfork did not report success: {output}")
    }
    Ok(BlockRef {
        workchain: captures[1].parse()?,
        shard: captures[2].to_lowercase(),
        seqno: captures[3].parse()?,
        root_hash: captures[4].to_lowercase(),
        file_hash: captures[5].to_lowercase(),
    })
}

fn block_id_text(block: &BlockRef) -> String {
    format!(
        "({},{},{}):{}:{}",
        block.workchain, block.shard, block.seqno, block.root_hash, block.file_hash
    )
}

#[cfg(test)]
mod tests {
    use super::parse_created_block;

    #[test]
    fn parses_create_hardfork_block() {
        let output = format!(
            "created block\nsuccess, block (-1,8000000000000000,9):{}:{}",
            "aa".repeat(32),
            "bb".repeat(32)
        );
        let block = parse_created_block(&output).unwrap();
        assert_eq!(block.seqno, 9);
        assert_eq!(block.workchain, -1);
    }
}
