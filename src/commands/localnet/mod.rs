mod status;

use crate::context::Wallet;
use crate::wallets;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::Context;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::WalletVersion;
use ton_localnet::node::StateSource;
use ton_localnet::remote::RemoteProvider;
use ton_localnet::storage::AccountStatus;
use ton_localnet::{Localnet, ServerArgs, StartupWallet, run_server};
use ton_retrace::Network;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{CellBuilder, CellSliceParts};
use tycho_types::models::{
    Base64StdAddrFlags, CurrencyCollection, DisplayBase64StdAddr, IntAddr, IntMsgInfo, MsgInfo,
    OwnedMessage, StdAddr,
};

const STARTUP_ACCOUNT_TOPUP_NANOTONS: u128 = 100_000_000_000; // 100 TON
const STARTUP_DEPLOY_TRANSFER_NANOTONS: u128 = 50_000_000; // 0.05 TON
const WALLET_MSG_TTL_SECONDS: u64 = 600;
pub use status::localnet_status_cmd;

#[allow(clippy::too_many_arguments)]
pub async fn localnet_start_cmd(
    port: u16,
    db_path: Option<String>,
    fork_net: Option<String>,
    fork_block_number: Option<u64>,
    accounts: Vec<String>,
    rate_limit: Option<u32>,
    response_delay_ms: Option<u64>,
    load_state: Option<String>,
    dump_state: Option<String>,
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
            }),
            Some(fork_network),
        )
    } else {
        (StateSource::Local, None)
    };

    let node = Arc::new(Localnet::new(state_source, db_path.clone()));
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

    let startup_wallets = setup_startup_accounts(&node, &accounts).await?;
    let run_result = run_server(
        node.clone(),
        ServerArgs {
            port,
            db_path,
            fork_network,
            fork_block_number,
            rate_limit_rps: rate_limit,
            response_delay_ms,
            startup_wallets,
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

async fn setup_startup_accounts(
    node: &Arc<Localnet>,
    accounts: &[String],
) -> anyhow::Result<Vec<StartupWallet>> {
    if accounts.is_empty() {
        return Ok(Vec::new());
    }

    let config =
        ActonConfig::load().context("Failed to load Acton.toml to resolve [localnet].accounts")?;
    let selected_wallets = wallets::open_selected_wallets(&config, accounts, &Network::Localnet)?;

    if selected_wallets.is_empty() {
        return Ok(Vec::new());
    }

    let configured_wallets = config
        .wallets
        .as_ref()
        .map(|wallets| &wallets.wallets)
        .context("No wallets are configured in Acton.toml")?;
    let mut startup_wallets = Vec::with_capacity(selected_wallets.len());

    for (wallet_name, wallet) in selected_wallets {
        let address = format_std_address(&wallet.address(), &Network::Localnet);
        let wallet_config = configured_wallets
            .get(&wallet_name)
            .with_context(|| format!("Wallet '{wallet_name}' disappeared from Acton.toml"))?;
        let mnemonic = wallets::load_mnemonic(&wallet_name, wallet_config)
            .with_context(|| format!("Failed to load mnemonic for wallet '{wallet_name}'"))?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let version = wallet_version_to_string(wallet.wallet.version).to_owned();

        let has_history = node
            .get_transactions(address.clone(), 1, None, None, None)
            .await
            .ok()
            .is_some_and(|transactions| !transactions.is_empty());

        if has_history {
            println!(
                "       {} wallet {} {}",
                "Found".green().bold(),
                wallet_name.cyan(),
                address.as_str().dimmed(),
            );
        } else {
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
                    address.as_str().dimmed(),
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
                    address.as_str().dimmed(),
                );
            }
        }

        startup_wallets.push(StartupWallet {
            name: wallet_name,
            mnemonic,
            version,
            network: "localnet".to_owned(),
            address,
            public_key: hex::encode(wallet.wallet.key_pair.public_key),
            wallet_id: wallet.wallet.wallet_id,
        });
    }

    Ok(startup_wallets)
}

const fn wallet_version_to_string(version: WalletVersion) -> &'static str {
    match version {
        WalletVersion::V1R1 => "v1r1",
        WalletVersion::V1R2 => "v1r2",
        WalletVersion::V1R3 => "v1r3",
        WalletVersion::V2R1 => "v2r1",
        WalletVersion::V2R2 => "v2r2",
        WalletVersion::V3R1 => "v3r1",
        WalletVersion::V3R2 => "v3r2",
        WalletVersion::V4R1 => "v4r1",
        WalletVersion::V4R2 => "v4r2",
        WalletVersion::V5R1 => "v5r1",
        WalletVersion::HLV1R1 => "highloadv1r1",
        WalletVersion::HLV1R2 => "highloadv1r2",
        WalletVersion::HLV2 => "highloadv2",
        WalletVersion::HLV2R1 => "highloadv2r1",
        WalletVersion::HLV2R2 => "highloadv2r2",
    }
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

pub async fn localnet_airdrop_cmd(address: &str, amount_ton: f64, port: u16) -> anyhow::Result<()> {
    let client = crate::http::client_builder()
        .user_agent(crate::build_info::user_agent())
        .build()?;
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;
    let res = client
        .post(format!("http://127.0.0.1:{port}/acton_fundAccount"))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanotons
        }))
        .send()
        .await
        .context("Failed to reach localnet faucet")?;

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
