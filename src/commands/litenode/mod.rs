use crate::wallets;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::Context;
use num_bigint::BigUint;
use retrace::Network;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ton_litenode::node::StateSource;
use ton_litenode::remote::RemoteProvider;
use ton_litenode::storage::AccountStatus;
use ton_litenode::{LiteNode, ServerArgs, run_server};
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::coins::{CurrencyCollection, Grams};
use tonlib_core::tlb_types::block::message::{CommonMsgInfo, IntMsgInfo, Message};
use tonlib_core::tlb_types::primitives::either::EitherRef;
use tonlib_core::tlb_types::tlb::TLB;
use tonlib_core::wallet::ton_wallet::TonWallet;

const STARTUP_ACCOUNT_TOPUP_NANOTONS: u128 = 100_000_000_000; // 100 TON
const STARTUP_DEPLOY_TRANSFER_NANOTONS: u128 = 50_000_000; // 0.05 TON
const WALLET_MSG_TTL_SECONDS: u64 = 600;

pub async fn litenode_start_cmd(
    port: u16,
    db_path: Option<String>,
    fork_net: Option<String>,
    fork_block_number: Option<u64>,
    accounts: Vec<String>,
    api_key: Option<String>,
) -> anyhow::Result<()> {
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
    setup_startup_accounts(&node, &accounts).await?;
    run_server(
        node,
        ServerArgs {
            port,
            db_path,
            fork_network,
            fork_block_number,
        },
    )
    .await?;
    Ok(())
}

async fn setup_startup_accounts(node: &Arc<LiteNode>, accounts: &[String]) -> anyhow::Result<()> {
    if accounts.is_empty() {
        return Ok(());
    }

    let config =
        ActonConfig::load().context("Failed to load Acton.toml to resolve [litenode].accounts")?;
    let selected_wallets = wallets::open_selected_wallets(&config, accounts, &Network::Testnet)?;

    if selected_wallets.is_empty() {
        return Ok(());
    }

    for (wallet_name, wallet) in selected_wallets {
        let address = wallet.wallet.address.to_base64_url_flags(false, true);

        node.faucet(address.clone(), STARTUP_ACCOUNT_TOPUP_NANOTONS)
            .await
            .with_context(|| format!("Failed to top up wallet '{wallet_name}'"))?;

        let wallet_state = node
            .get_address_state(address.clone(), None)
            .await
            .with_context(|| format!("Failed to fetch state for wallet '{wallet_name}'"))?;

        if wallet_state != AccountStatus::Active {
            let deploy_boc = build_wallet_deploy_message(&wallet.wallet)?;
            node.send_boc(deploy_boc)
                .await
                .with_context(|| format!("Failed to deploy wallet '{wallet_name}'"))?;
            println!(
                "       {} wallet {} {}",
                "Ready".green().bold(),
                wallet_name.cyan(),
                address.dimmed(),
            );
        } else {
            println!(
                "      {} wallet {} {}",
                "Funded".green().bold(),
                wallet_name.cyan(),
                address.dimmed(),
            );
        }
    }

    Ok(())
}

fn build_wallet_deploy_message(wallet: &TonWallet) -> anyhow::Result<String> {
    let expire_at = (SystemTime::now() + Duration::from_secs(WALLET_MSG_TTL_SECONDS))
        .duration_since(UNIX_EPOCH)?
        .as_secs() as u32;

    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: wallet.address.to_msg_address(),
        dest: wallet.address.to_msg_address(),
        value: CurrencyCollection::new(BigUint::from(STARTUP_DEPLOY_TRANSFER_NANOTONS)),
        ihr_fee: Grams::new(BigUint::from(0u64)),
        fwd_fee: Grams::new(BigUint::from(0u64)),
        created_at: 0,
        created_lt: 0,
    };

    let message = Message {
        info: CommonMsgInfo::Int(message_info),
        init: None,
        body: EitherRef::new(ArcCell::default()),
    };

    let message_cell = message.to_cell()?;
    let external = wallet.create_external_msg(expire_at, 0, true, vec![message_cell.to_arc()])?;
    Ok(external.to_boc_b64(false)?)
}

pub async fn litenode_airdrop_cmd(address: &str, amount_ton: f64, port: u16) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;

    let res = client
        .post(format!("http://localhost:{}/admin/faucet", port))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanotons
        }))
        .send()
        .await?;

    if res.status().is_success() {
        let json: serde_json::Value = res.json().await?;
        if json.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
            || json
                .get("success")
                .and_then(|v| v.as_bool())
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
            anyhow::bail!("Airdrop failed: {}", error);
        }
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("Airdrop failed with status {}: {}", status, body);
    }

    Ok(())
}
