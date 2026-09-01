use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use tracing::{info, warn};

use crate::{
    cli::WalletVersion,
    operations::{validators, wallets},
    ton::toolchain::Toolchain,
};

pub(super) const VALIDATOR_WALLET_WORKCHAIN: i32 = -1;
const VALIDATOR_WALLET_ID: u32 = 42;
const VALIDATOR_FEE_RESERVE_NANO: u64 = 5_000_000_000;

/// Development faucet response used only to verify the requested wallet and amount.
#[derive(Deserialize)]
struct FaucetGrant {
    address: String,
    amount_nano: u64,
}

/// Applies the joined network's election timing and stake limits to validator automation.
///
/// The caller must select the synchronized node's liteserver config before invoking
/// this function. Persisted values are refreshed on every join start so later manual
/// validator commands use the same limits as the running network.
pub(super) async fn apply_network_validator_config(toolchain: &Toolchain) -> Result<()> {
    let network = validators::election_status(toolchain).await?;
    ensure!(
        network.min_stake_nano <= network.max_stake_nano,
        "network validator stake limits are inconsistent"
    );
    ensure!(
        network.max_stake_factor_q16 >= 1 << 16,
        "network maximum stake factor is below 1"
    );

    let mut settings = toolchain.settings()?;
    settings.network.elected_for_seconds = network.validators_elected_for;
    settings.network.election_start_before_seconds = network.elections_start_before;
    settings.network.election_end_before_seconds = network.elections_end_before;
    settings.network.stakes_frozen_for_seconds = network.stake_held_for;
    settings.node.validator_stake_nano = settings
        .node
        .validator_stake_nano
        .clamp(network.min_stake_nano, network.max_stake_nano);
    settings.validation.max_factor = settings
        .validation
        .max_factor
        .min(f64::from(network.max_stake_factor_q16) / f64::from(1_u32 << 16));

    settings.validate()?;
    settings.save_atomic(&toolchain.layout.settings)?;
    info!(
        operation = "configure_join_validator",
        node = settings.node.name,
        elected_for_seconds = settings.network.elected_for_seconds,
        validator_stake_nano = settings.node.validator_stake_nano,
        max_factor = settings.validation.max_factor,
        outcome = "success",
        "joined network validator configuration applied"
    );

    Ok(())
}

/// Runs election maintenance for the node owned by this join instance.
///
/// A failed tick is logged and the loop remains alive so a temporary faucet,
/// liteserver, or election-contract failure can recover later.
pub(super) async fn validation_loop(
    toolchain: Toolchain,
    faucet: Option<String>,
    interval_seconds: u64,
) -> Result<()> {
    let client = reqwest::Client::new();
    let node_name = toolchain.settings()?.node.name;

    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        if let Err(error) = validation_tick(&toolchain, &client, faucet.as_deref()).await {
            warn!(node = node_name, %error, "validator election tick failed");
        }
    }
}

/// Ensures the validator has funding and delegates election state transitions.
///
/// Wallet creation is idempotent. Faucet use is limited to bringing the wallet up
/// to the configured stake plus fee reserve; normal election participation then
/// proceeds entirely through TON contracts and validator-console operations.
async fn validation_tick(
    toolchain: &Toolchain,
    client: &reqwest::Client,
    faucet: Option<&str>,
) -> Result<()> {
    let settings = toolchain.settings()?;
    let node = &settings.node;
    if !node.validator {
        return Ok(());
    }

    let wallet_name = validators::validator_wallet_name(node);
    let wallet = wallets::ensure_wallet_for_toolchain(
        toolchain,
        &wallet_name,
        WalletVersion::V4r2,
        VALIDATOR_WALLET_WORKCHAIN,
        VALIDATOR_WALLET_ID,
    )
    .await?;

    if settings.validation.auto_participate && node.participate_in_elections {
        let minimum_balance = node
            .validator_stake_nano
            .saturating_add(VALIDATOR_FEE_RESERVE_NANO);
        let balance = wallets::wallet_balance_nano(toolchain, &wallet_name).await?;

        // The faucet is a bootstrap convenience only and is contacted when the
        // persisted wallet cannot cover the configured stake plus fee reserve.
        if balance < u128::from(minimum_balance) {
            let faucet_url = faucet.with_context(|| {
                format!(
                    "validator wallet {} needs at least {minimum_balance} nanotons; fund it or pass --faucet",
                    wallet.address
                )
            })?;

            let grant = client
                .post(faucet_url)
                .json(&serde_json::json!({"address": &wallet.address}))
                .send()
                .await
                .with_context(|| format!("failed to request {faucet_url}"))?
                .error_for_status()
                .with_context(|| format!("development faucet rejected {faucet_url}"))?
                .json::<FaucetGrant>()
                .await
                .context("development faucet returned an invalid grant")?;

            ensure!(
                grant.address == wallet.address,
                "development faucet funded a different address"
            );

            wallets::wait_for_wallet_balance(
                toolchain,
                &wallet.address,
                balance.saturating_add(u128::from(grant.amount_nano)),
            )
            .await?;
            info!(node = node.name, wallet = %wallet.address, amount_nano = grant.amount_nano, "validator wallet funded");
        }

        wallets::ensure_wallet_deployed(toolchain, &wallet_name).await?;
    }

    validators::join_auto_tick(toolchain, &wallet_name).await
}
