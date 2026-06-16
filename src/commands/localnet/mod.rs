mod status;

use crate::context::Wallet;
use crate::wallets;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::Context;
use rand::RngCore;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::WalletVersion;
use ton_localnet::node::StateSource;
use ton_localnet::remote::RemoteProvider;
use ton_localnet::storage::AccountStatus;
use ton_localnet::{Localnet, ServerArgs, StartupWallet, run_server};
use ton_retrace::Network;
use toncenter_keys::LOCALNET_API_KEY_ENV;
use tycho_types::boc::BocRepr;
use tycho_types::cell::{CellBuilder, CellSliceParts};
use tycho_types::models::{
    Base64StdAddrFlags, CurrencyCollection, DisplayBase64StdAddr, IntAddr, IntMsgInfo, MsgInfo,
    OwnedMessage, StdAddr,
};

const STARTUP_ACCOUNT_TOPUP_NANOGRAMS: u128 = 100_000_000_000; // 100 GRAM
const STARTUP_DEPLOY_TRANSFER_NANOGRAMS: u128 = 50_000_000; // 0.05 GRAM
const WALLET_MSG_TTL_SECONDS: u64 = 600;
pub(crate) const LOCALNET_AUTH_TOKEN_ENV: &str = LOCALNET_API_KEY_ENV;
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
    block_interval_ms: u64,
    no_mining: bool,
    load_state: Option<String>,
    dump_state: Option<String>,
    require_auth: bool,
) -> anyhow::Result<()> {
    if load_state.is_some() && db_path.is_some() {
        anyhow::bail!("--load-state cannot be used together with --db-path for now");
    }
    if block_interval_ms == 0 {
        anyhow::bail!("localnet block interval must be greater than 0");
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

    let node = Arc::new(Localnet::new(
        state_source,
        db_path.clone(),
        Duration::from_millis(block_interval_ms),
        !no_mining,
    ));
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

    let startup_wallets = setup_startup_accounts(&node, &accounts, no_mining).await?;
    let auth_token = require_auth.then(localnet_auth_token);
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
            auth_token,
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
    manual_mining: bool,
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
            node.faucet(address.clone(), STARTUP_ACCOUNT_TOPUP_NANOGRAMS)
                .await
                .with_context(|| format!("Failed to top up wallet '{wallet_name}'"))?;
            if manual_mining {
                node.mine_blocks(1)
                    .await
                    .with_context(|| format!("Failed to mine top up for wallet '{wallet_name}'"))?;
            }

            let wallet_state = wait_for_startup_wallet_funds(node, &address, &wallet_name).await?;

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
                if manual_mining {
                    node.mine_blocks(1).await.with_context(|| {
                        format!("Failed to mine deployment for wallet '{wallet_name}'")
                    })?;
                }
                wait_for_startup_wallet_deploy(node, &address, &wallet_name).await?;
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

async fn wait_for_startup_wallet_funds(
    node: &Arc<Localnet>,
    address: &str,
    wallet_name: &str,
) -> anyhow::Result<AccountStatus> {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let wallet_state = node
            .get_address_state(address.to_owned(), None)
            .await
            .with_context(|| format!("Failed to fetch state for wallet '{wallet_name}'"))?;

        if wallet_state != AccountStatus::Nonexist {
            return Ok(wallet_state);
        }

        if Instant::now() >= deadline {
            anyhow::bail!("Timed out waiting for localnet top up of wallet '{wallet_name}'");
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_startup_wallet_deploy(
    node: &Arc<Localnet>,
    address: &str,
    wallet_name: &str,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let wallet_state = node
            .get_address_state(address.to_owned(), None)
            .await
            .with_context(|| format!("Failed to fetch state for wallet '{wallet_name}'"))?;

        if wallet_state == AccountStatus::Active {
            return Ok(());
        }

        if Instant::now() >= deadline {
            anyhow::bail!("Timed out waiting for localnet deployment of wallet '{wallet_name}'");
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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
        value: CurrencyCollection::new(STARTUP_DEPLOY_TRANSFER_NANOGRAMS),
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

pub async fn localnet_airdrop_cmd(
    address: &str,
    amount_grams: f64,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let client = crate::http::client_builder()
        .user_agent(crate::build_info::user_agent())
        .build()?;
    let amount_nanograms = (amount_grams * 1_000_000_000.0) as u128;
    let auth_token = resolve_localnet_auth_token(auth_token);
    let request = client
        .post(format!("http://127.0.0.1:{port}/acton_fundAccount"))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanograms
        }));
    let res = with_localnet_auth(request, auth_token.as_deref())
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
                "{} airdrop {} GRAM to {} on localnet",
                "Successfully".green().bold(),
                amount_grams,
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

pub async fn localnet_mine_cmd(
    blocks: u32,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = post_localnet_control(
        port,
        auth_token,
        "acton_mine",
        serde_json::json!({ "blocks": blocks }),
        "Mining",
    )
    .await?;
    let blocks_mined = result
        .get("blocks_mined")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| u64::from(blocks));
    let last_block_seqno = result
        .get("last_block_seqno")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    println!(
        "{} mined {} localnet block{}; latest seqno {}",
        "Successfully".green().bold(),
        blocks_mined,
        if blocks_mined == 1 { "" } else { "s" },
        last_block_seqno
    );

    Ok(())
}

pub async fn localnet_increase_time_cmd(
    seconds: u64,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = post_localnet_control(
        port,
        auth_token,
        "acton_increaseTime",
        serde_json::json!({ "seconds": seconds }),
        "Increase time",
    )
    .await?;
    print_clock_update("increased localnet time", &result);
    Ok(())
}

pub async fn localnet_set_time_cmd(
    timestamp: u32,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = post_localnet_control(
        port,
        auth_token,
        "acton_setTime",
        serde_json::json!({ "timestamp": timestamp }),
        "Set time",
    )
    .await?;
    print_clock_update("set localnet time", &result);
    Ok(())
}

pub async fn localnet_set_next_block_timestamp_cmd(
    timestamp: u32,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = post_localnet_control(
        port,
        auth_token,
        "acton_setNextBlockTimestamp",
        serde_json::json!({ "timestamp": timestamp }),
        "Set next block timestamp",
    )
    .await?;
    print_clock_update("set next block timestamp", &result);
    Ok(())
}

async fn post_localnet_control(
    port: u16,
    auth_token: Option<String>,
    path: &str,
    body: serde_json::Value,
    action: &str,
) -> anyhow::Result<serde_json::Value> {
    let client = crate::http::client_builder()
        .user_agent(crate::build_info::user_agent())
        .build()?;
    let auth_token = resolve_localnet_auth_token(auth_token);
    let request = client
        .post(format!("http://127.0.0.1:{port}/{path}"))
        .json(&body);
    let res = with_localnet_auth(request, auth_token.as_deref())
        .send()
        .await
        .with_context(|| format!("Failed to reach localnet control endpoint /{path}"))?;

    if res.status().is_success() {
        let json: serde_json::Value = res.json().await?;
        if json
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(json
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null))
        } else {
            let error = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("{action} failed: {error}");
        }
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("{action} failed with status {status}: {body}");
    }
}

fn print_clock_update(action: &str, result: &serde_json::Value) {
    let current = result
        .get("current_unix_time")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let offset = result
        .get("time_offset_seconds")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let pending = result
        .get("next_block_timestamp")
        .and_then(serde_json::Value::as_u64);

    if let Some(pending) = pending {
        println!(
            "{} {}; virtual time {}, offset {}s, next block timestamp {}",
            "Successfully".green().bold(),
            action,
            current,
            offset,
            pending
        );
    } else {
        println!(
            "{} {}; virtual time {}, offset {}s",
            "Successfully".green().bold(),
            action,
            current,
            offset
        );
    }
}

fn localnet_auth_token() -> String {
    localnet_auth_token_from_env().unwrap_or_else(generate_localnet_auth_token)
}

pub(crate) fn resolve_localnet_auth_token(auth_token: Option<String>) -> Option<String> {
    auth_token
        .and_then(|token| {
            let token = token.trim().to_owned();
            (!token.is_empty()).then_some(token)
        })
        .or_else(localnet_auth_token_from_env)
}

fn localnet_auth_token_from_env() -> Option<String> {
    std::env::var(LOCALNET_AUTH_TOKEN_ENV)
        .ok()
        .and_then(|token| {
            let token = token.trim().to_owned();
            (!token.is_empty()).then_some(token)
        })
}

fn generate_localnet_auth_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub(crate) fn with_localnet_auth(
    request: reqwest::RequestBuilder,
    auth_token: Option<&str>,
) -> reqwest::RequestBuilder {
    match auth_token.map(str::trim).filter(|token| !token.is_empty()) {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}
