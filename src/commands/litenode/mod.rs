use crate::context::Wallet;
use crate::wallets;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::Context;
use retrace::Network;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton_litenode::node::StateSource;
use ton_litenode::remote::RemoteProvider;
use ton_litenode::storage::AccountStatus;
use ton_litenode::{LiteNode, ServerArgs, run_server};
use tycho_types::boc::BocRepr;
use tycho_types::cell::{CellBuilder, CellSliceParts};
use tycho_types::models::{
    Base64StdAddrFlags, CurrencyCollection, DisplayBase64StdAddr, IntAddr, IntMsgInfo, MsgInfo,
    OwnedMessage, StdAddr,
};

const STARTUP_ACCOUNT_TOPUP_NANOTONS: u128 = 100_000_000_000; // 100 TON
const STARTUP_DEPLOY_TRANSFER_NANOTONS: u128 = 50_000_000; // 0.05 TON
const WALLET_MSG_TTL_SECONDS: u64 = 600;

#[allow(clippy::too_many_arguments)]
pub async fn litenode_start_cmd(
    port: u16,
    db_path: Option<String>,
    fork_net: Option<String>,
    fork_block_number: Option<u64>,
    accounts: Vec<String>,
    rate_limit: Option<u32>,
    load_state: Option<String>,
    dump_state: Option<String>,
    api_key: Option<String>,
) -> anyhow::Result<()> {
    if load_state.is_some() && db_path.is_some() {
        anyhow::bail!("--load-state cannot be used together with --db-path for now");
    }

    let (state_source, fork_network) = if let Some(network) = fork_net {
        let network = Network::from_str(&network)?;
        let fork_network = network.to_string();
        (
            StateSource::Remote(RemoteProvider {
                network,
                fork_block_number,
                api_key,
            }),
            Some(fork_network),
        )
    } else {
        (StateSource::Local, None)
    };

    let node = Arc::new(LiteNode::new(state_source, db_path.clone()));
    if let Some(path) = load_state.as_deref() {
        node.load_state(path.to_owned())
            .await
            .with_context(|| format!("Failed to load state snapshot from {path}"))?;
        println!(
            "      {} state from {}",
            "Loaded".green().bold(),
            path.dimmed()
        );
    }

    setup_startup_accounts(&node, &accounts).await?;
    let run_result = run_server(
        node.clone(),
        ServerArgs {
            port,
            db_path,
            fork_network,
            fork_block_number,
            rate_limit_rps: rate_limit,
        },
    )
    .await;

    if run_result.is_ok()
        && let Some(path) = dump_state.as_deref()
    {
        node.dump_state(path.to_owned())
            .await
            .with_context(|| format!("Failed to dump state snapshot to {path}"))?;
        println!(
            "       {} state to {}",
            "Saved".green().bold(),
            path.dimmed()
        );
    }

    run_result?;
    Ok(())
}

async fn setup_startup_accounts(node: &Arc<LiteNode>, accounts: &[String]) -> anyhow::Result<()> {
    if accounts.is_empty() {
        return Ok(());
    }

    let config =
        ActonConfig::load().context("Failed to load Acton.toml to resolve [litenode].accounts")?;
    let selected_wallets = wallets::open_selected_wallets(&config, accounts, &Network::Localnet)?;

    if selected_wallets.is_empty() {
        return Ok(());
    }

    for (wallet_name, wallet) in selected_wallets {
        let address = format_std_address(&wallet.address(), &Network::Localnet);

        node.faucet(address.clone(), STARTUP_ACCOUNT_TOPUP_NANOTONS)
            .await
            .with_context(|| format!("Failed to top up wallet '{wallet_name}'"))?;

        let wallet_state = node
            .get_address_state(address.clone(), None)
            .await
            .with_context(|| format!("Failed to fetch state for wallet '{wallet_name}'"))?;

        if wallet_state == AccountStatus::Active {
            println!(
                "      {} wallet {} {}",
                "Funded".green().bold(),
                wallet_name.cyan(),
                address.dimmed(),
            );
        } else {
            let deploy_boc = build_wallet_deploy_message(&wallet)?;
            node.send_boc(deploy_boc)
                .await
                .with_context(|| format!("Failed to deploy wallet '{wallet_name}'"))?;
            println!(
                "       {} wallet {} {}",
                "Ready".green().bold(),
                wallet_name.cyan(),
                address.dimmed(),
            );
        }
    }

    Ok(())
}

fn build_wallet_deploy_message(wallet: &Wallet) -> anyhow::Result<String> {
    let expire_at = (SystemTime::now() + Duration::from_secs(WALLET_MSG_TTL_SECONDS))
        .duration_since(UNIX_EPOCH)?
        .as_secs() as u32;

    let wallet_addr = wallet.address();
    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: IntAddr::Std(wallet_addr.clone()),
        dst: IntAddr::Std(wallet_addr),
        value: CurrencyCollection::new(STARTUP_DEPLOY_TRANSFER_NANOTONS),
        ihr_fee: Default::default(),
        fwd_fee: Default::default(),
        created_at: 0,
        created_lt: 0,
    };

    let message = OwnedMessage {
        info: MsgInfo::Int(message_info),
        init: None,
        body: CellSliceParts::from(CellBuilder::new().build()?),
        layout: None,
    };

    let message_cell_boc = BocRepr::encode(message)?;
    let message_cell = TonCell::from_boc(message_cell_boc)?;
    let external = wallet
        .wallet
        .create_ext_in_msg(vec![message_cell], 0, expire_at, true)?;
    Ok(external.to_boc_base64()?)
}

fn format_std_address(address: &StdAddr, network: &Network) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string()
}

pub async fn litenode_airdrop_cmd(address: &str, amount_ton: f64, port: u16) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;

    let res = client
        .post(format!("http://localhost:{port}/admin/faucet"))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanotons
        }))
        .send()
        .await?;

    if res.status().is_success() {
        let json: serde_json::Value = res.json().await?;
        if json
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || json
                .get("success")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        {
            println!(
                "{} airdrop {} TON to {} on localnet",
                "Successfully".green().bold(),
                amount_ton,
                address
            );
        } else {
            let error = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("Airdrop failed: {error}");
        }
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("Airdrop failed with status {status}: {body}");
    }

    Ok(())
}
