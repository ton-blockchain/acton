use std::{fs, time::Duration};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use num_bigint::BigInt;
use serde::Serialize;

use crate::{
    cli::{ConfigCommand, LiteCommand, StateArgs},
    storage::Layout,
    storage::Settings,
    ton::{
        lite::{parse_shard, require_existing_config},
        toolchain::Toolchain,
        tools::{
            lite_client::{
                AccountStateRequest, BlockTransactionsRequest, Boc, LiteTarget, LookupBlock,
                RunMethodRequest,
            },
            types::OperationContext,
        },
    },
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
    }
    Ok(())
}

pub async fn lite(command: LiteCommand) -> Result<()> {
    match command {
        LiteCommand::Last { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let target = lite_target(&toolchain)?;
            let info = toolchain
                .lite_client_tool
                .masterchain_info(&OperationContext::new(Duration::from_secs(30)), &target)
                .await?;
            print_json(&info.last)?;
        }
        LiteCommand::Account { state, address } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let account = toolchain
                .lite_client_tool
                .account_state(
                    &OperationContext::new(Duration::from_secs(30)),
                    &lite_target(&toolchain)?,
                    AccountStateRequest::new(&address)?,
                )
                .await?;
            print_json(&account)?;
        }
        LiteCommand::RunMethod {
            state,
            address,
            method,
            params,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let result = toolchain
                .lite_client_tool
                .run_method(
                    &OperationContext::new(Duration::from_secs(30)),
                    &lite_target(&toolchain)?,
                    RunMethodRequest::new(
                        &address,
                        method,
                        params
                            .iter()
                            .map(|value| parse_stack_integer(value))
                            .collect::<Result<Vec<_>>>()?,
                    )?,
                )
                .await?;
            print_json(&result)?;
        }
        LiteCommand::Send { state, boc } => {
            let bytes =
                fs::read(&boc).with_context(|| format!("failed to read {}", boc.display()))?;
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let result = toolchain
                .lite_client_tool
                .send_boc(
                    &OperationContext::new(Duration::from_secs(30)),
                    &lite_target(&toolchain)?,
                    Boc::new(bytes)?,
                )
                .await?;
            print_json(&serde_json::json!({
                "status": result.status,
                "boc": boc,
            }))?;
        }
        LiteCommand::Block {
            state,
            workchain,
            shard,
            seqno,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let block = toolchain
                .lite_client_tool
                .block(
                    &OperationContext::new(Duration::from_secs(30)),
                    &lite_target(&toolchain)?,
                    LookupBlock {
                        workchain,
                        shard: parse_shard(&shard)?,
                        seqno,
                    },
                )
                .await?;
            print_json(&serde_json::json!({
                "id": block.id,
                "boc_base64": STANDARD.encode(block.boc.as_bytes()),
            }))?;
        }
        LiteCommand::Transactions {
            state,
            workchain,
            shard,
            seqno,
            count,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let block = toolchain
                .lite_client_tool
                .block_transactions(
                    &OperationContext::new(Duration::from_secs(30)),
                    &lite_target(&toolchain)?,
                    BlockTransactionsRequest::new(
                        toolchain
                            .lite_client_tool
                            .lookup_block(
                                &OperationContext::new(Duration::from_secs(30)),
                                &lite_target(&toolchain)?,
                                LookupBlock {
                                    workchain,
                                    shard: parse_shard(&shard)?,
                                    seqno,
                                },
                            )
                            .await?,
                        count,
                    )?,
                )
                .await?;
            print_json(&serde_json::json!({
                "block": block.block,
                "transactions": block.transactions,
                "incomplete": block.incomplete,
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
        LiteCommand::Elections { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            print_json(&crate::operations::validators::election_status(&toolchain).await?)?;
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

/// Selects the trusted network config used by typed liteserver operations
fn lite_target(toolchain: &Toolchain) -> Result<LiteTarget> {
    require_existing_config(&toolchain.layout.global_config)?;
    Ok(LiteTarget::new(&toolchain.layout.global_config).with_label("localton"))
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Parses the integer-only TVM argument subset accepted by the typed CLI.
fn parse_stack_integer(value: &str) -> Result<BigInt> {
    let (negative, digits) = value
        .strip_prefix("-0x")
        .map(|digits| (true, digits))
        .or_else(|| value.strip_prefix("0x").map(|digits| (false, digits)))
        .unwrap_or((false, value));
    let radix = if value.contains("0x") { 16 } else { 10 };
    let integer = BigInt::parse_bytes(digits.as_bytes(), radix)
        .with_context(|| format!("invalid TVM integer `{value}`"))?;
    Ok(if negative { -integer } else { integer })
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
