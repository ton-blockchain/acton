use std::fs;

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;

use crate::{
    cli::{ConfigCommand, LiteCommand, StateArgs},
    storage::Layout,
    storage::Settings,
    ton::lite::{LocalLiteClient, require_existing_config},
    ton::toolchain::Toolchain,
};

pub async fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Init(state) => {
            let layout = layout(&state)?;
            let settings = Settings::load_or_create(&layout.settings)?;
            settings.validate()?;
            println!("{}", layout.settings.display());
        }
        ConfigCommand::Show { state } => {
            let layout = layout(&state)?;
            let settings = Settings::load_or_create(&layout.settings)?;
            print_json(&settings)?;
        }
        ConfigCommand::Validate(state) => {
            let layout = layout(&state)?;
            Settings::load_or_create(&layout.settings)?.validate()?;
            println!("valid: {}", layout.settings.display());
        }
        ConfigCommand::Validators { state, count } => {
            let layout = layout(&state)?;
            let mut settings = Settings::load_or_create(&layout.settings)?;
            settings.enable_validator_count(count)?;
            settings.save_atomic(&layout.settings)?;
            println!("validators={count}");
        }
    }
    Ok(())
}

pub async fn lite(command: LiteCommand) -> Result<()> {
    match command {
        LiteCommand::Last { state } => {
            let layout = layout(&state)?;
            require_existing_config(&layout.global_config)?;
            let mut client = LocalLiteClient::connect(&layout.global_config).await?;
            print_json(&client.last().await?)?;
        }
        LiteCommand::Account { state, address } => {
            let layout = layout(&state)?;
            require_existing_config(&layout.global_config)?;
            let mut client = LocalLiteClient::connect(&layout.global_config).await?;
            print_json(&client.account(&address).await?)?;
        }
        LiteCommand::RunMethod {
            state,
            address,
            method,
            params,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let command = format!("runmethod {address} {method} {params}");
            print!("{}", toolchain.lite_client(command.trim()).await?);
        }
        LiteCommand::Send { state, boc } => {
            let layout = layout(&state)?;
            require_existing_config(&layout.global_config)?;
            let bytes =
                fs::read(&boc).with_context(|| format!("failed to read {}", boc.display()))?;
            let mut client = LocalLiteClient::connect(&layout.global_config).await?;
            print_json(&serde_json::json!({
                "status": client.send_boc(bytes).await?,
                "boc": boc,
            }))?;
        }
        LiteCommand::Block {
            state,
            workchain,
            shard,
            seqno,
        } => {
            let layout = layout(&state)?;
            require_existing_config(&layout.global_config)?;
            let mut client = LocalLiteClient::connect(&layout.global_config).await?;
            let (id, boc) = client.block(workchain, &shard, seqno).await?;
            print_json(&serde_json::json!({
                "id": id,
                "boc_base64": STANDARD.encode(boc),
            }))?;
        }
        LiteCommand::Transactions {
            state,
            workchain,
            shard,
            seqno,
            count,
        } => {
            let layout = layout(&state)?;
            require_existing_config(&layout.global_config)?;
            let mut client = LocalLiteClient::connect(&layout.global_config).await?;
            let (block, transactions, incomplete) =
                client.transactions(workchain, &shard, seqno, count).await?;
            print_json(&serde_json::json!({
                "block": block,
                "transactions": transactions,
                "incomplete": incomplete,
            }))?;
        }
        LiteCommand::Shards { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let last = toolchain.lite_client("last").await?;
            let full_id = extract_latest_block_id(&last)?;
            print!(
                "{}",
                toolchain
                    .lite_client(&format!("allshards {full_id}"))
                    .await?
            );
        }
        LiteCommand::Config { state, params } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            if params.is_empty() {
                print!("{}", toolchain.lite_client("getconfig -1").await?);
            } else {
                for parameter in params {
                    println!("config {parameter}");
                    print!(
                        "{}",
                        toolchain
                            .lite_client(&format!("getconfig {parameter}"))
                            .await?
                    );
                }
            }
        }
        LiteCommand::Exec { state, args } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            print!("{}", toolchain.lite_client(&args.join(" ")).await?);
        }
    }
    Ok(())
}

fn layout(state: &StateArgs) -> Result<Layout> {
    let root = crate::ton::toolchain::absolute_path(&state.state_dir)?;
    let layout = Layout::new(root);
    layout.create_dirs()?;
    Ok(layout)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn extract_latest_block_id(output: &str) -> Result<String> {
    let start = output
        .find('(')
        .context("lite-client output does not contain a block id")?;
    let tail = &output[start..];
    let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
    let candidate = tail[..end].trim_end_matches(',');
    if candidate.contains("):") {
        Ok(candidate.to_owned())
    } else {
        bail!("lite-client output contains an invalid block id: {candidate}")
    }
}

#[cfg(test)]
mod tests {
    use super::extract_latest_block_id;

    #[test]
    fn extracts_official_lite_client_block_id() {
        let output = "latest masterchain block is (-1,8000000000000000,7):AA:BB\n";
        assert_eq!(
            extract_latest_block_id(output).unwrap(),
            "(-1,8000000000000000,7):AA:BB"
        );
    }
}
