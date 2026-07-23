use anyhow::Context;
use apalis::layers::WorkerBuilderExt;
use apalis::prelude::json::JsonCodec;
use apalis::prelude::{Data, WorkerBuilder};
use apalis_sqlite::{CompactType, Config as SqliteConfig, HookCallbackListener, SqliteStorage};
use axum_governor::GovernorLayer;
use faucet_antifraud::Antifraud;
use faucet_config::{ClaimRateLimitConfig, Config, DefaultRateLimitConfig};
use faucet_pow::Pow;
use faucet_valkey::{
    AntifraudModule, SentAmountWindowDecision, SuccessfulClaimWindowDecision, ValkeyStore,
};
use handlers::CreateClaim;
use lazy_limit::{Duration, RuleConfig, init_rate_limiter};
use moka::sync::Cache;
use real::RealIpLayer;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use ton::block_tlb::{CommonMsgInfo, CommonMsgInfoInt, CurrencyCollection, Msg};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_core::types::TonAddress;
use ton::ton_core::types::tlb_core::TLBCoins;
use toncenter::ToncenterClient;
use tower::ServiceBuilder;
use tracing::{error, info, warn};
use wallet::Wallet;

mod handlers;
mod logger;
mod wallet;

pub const LONG_VERSION: &str = env!("FAUCET_LONG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logger::init_tracing();
    let config = Config::from_env().context("Failed to load config")?;

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);

    info!(
        version = LONG_VERSION,
        bind_addr = %bind_addr,
        database_url = %config.database.url,
        toncenter_url = %config.toncenter.url,
        "Loaded startup config"
    );

    info!(database_url = %config.database.url, "Connecting to database");
    let opts = SqliteConnectOptions::from_str(&config.database.url)
        .context("Invalid database URL")?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .context("Failed to connect to database")?;
    info!("Connected to database");

    let default_rate_limit = default_rate_limit_rule(&config.rate_limit.default);
    let claim_rate_limit = claim_rate_limit_rule(&config.rate_limit.claim);
    init_rate_limiter!(
        default: default_rate_limit,
        max_memory: Some(64 * 1024 * 1024),
        routes: [
            ("/claim", claim_rate_limit),
        ]
    )
    .await;
    info!("Initialized rate limiter");

    SqliteStorage::setup(&pool)
        .await
        .context("Failed to setup storage")?;
    let storage_config = SqliteConfig::new(std::any::type_name::<CreateClaim>());
    let storage = SqliteStorage::new_with_callback(&config.database.url, &storage_config);
    info!("Initialized claim storage");

    let wallet = Wallet::new(&config.faucet.mnemonic).context("Failed to create faucet wallet")?;
    info!(
        faucet_address = %wallet.get_address(),
        "Created faucet wallet"
    );

    let client = Arc::new(
        ToncenterClient::new(&config.toncenter).context("Failed to create Toncenter client")?,
    );
    info!("Created Toncenter client");
    let valkey = ValkeyStore::new(&config.valkey)
        .await
        .context("Failed to create Valkey store")?;
    info!("Connected to Valkey");
    let antifraud = Antifraud::new(&config.antifraud);

    let shared_state = AppState {
        storage: storage.clone(),
        wallet: Arc::new(wallet),
        client: client.clone(),
        pow: Pow::new(config.pow.difficulty),
        pow_challenges: Cache::builder()
            .time_to_live(StdDuration::from_secs(config.pow.challenge_ttl_seconds))
            .max_capacity(config.pow.max_challenges)
            .build(),
        valkey,
        antifraud,
        config: Arc::new(config),
    };

    let worker_state = shared_state.clone();

    let worker = WorkerBuilder::new("claim-worker")
        .backend(storage)
        .concurrency(1)
        .data(worker_state)
        .build(send_claim);

    tokio::spawn(async move {
        info!("Starting claim worker");
        if let Err(err) = worker.run().await {
            error!(error = %err, "Worker failed");
        }
    });

    let app = handlers::router(shared_state).layer(
        ServiceBuilder::new()
            .layer(RealIpLayer::default())
            .layer(GovernorLayer::default()),
    );

    info!("Listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context("Failed to bind TCP listener")?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("HTTP server exited with error")?;

    Ok(())
}

fn default_rate_limit_rule(config: &DefaultRateLimitConfig) -> RuleConfig {
    RuleConfig::new(
        Duration::seconds(config.window_seconds),
        config.max_requests,
    )
}

fn claim_rate_limit_rule(config: &ClaimRateLimitConfig) -> RuleConfig {
    RuleConfig::new(
        Duration::seconds(config.window_seconds),
        config.max_requests,
    )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            error!(error = %err, "failed to install Ctrl+C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                error!(error = %err, "failed to install signal handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutting down gracefully...");
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) storage: SqliteStorage<CreateClaim, JsonCodec<CompactType>, HookCallbackListener>,
    wallet: Arc<Wallet>,
    client: Arc<ToncenterClient>,
    pub(crate) pow: Pow,
    pub(crate) pow_challenges: Cache<String, u32>,
    pub(crate) valkey: ValkeyStore,
    pub(crate) antifraud: Antifraud,
    pub(crate) config: Arc<Config>,
}

impl AppState {
    pub(crate) async fn record_antifraud_trigger(&self, module: AntifraudModule) {
        match self.valkey.increment_antifraud_trigger_count(module).await {
            Ok(trigger_count) => {
                info!(
                    module = module.name(),
                    trigger_count, "Recorded antifraud trigger in Valkey"
                );
            }
            Err(err) => {
                warn!(
                    module = module.name(),
                    error = %err,
                    "Failed to record antifraud trigger in Valkey"
                );
            }
        }
    }
}

async fn send_claim(task: CreateClaim, state: Data<AppState>) -> anyhow::Result<()> {
    let wallet = state.wallet.as_ref();
    let client = state.client.as_ref();
    let amount = state.config.faucet.amount;

    info!("Processing claim for address: {}", task.address);

    if !can_process_successful_claim_window(&state, &task.address).await? {
        return Ok(());
    }

    wait_for_sent_amount_window(&state, &task.address, amount).await?;

    let max_retries = state.config.worker.max_retries;

    for attempt in 0..=max_retries {
        let status = process_send_tokens(
            wallet,
            client,
            &task.address,
            amount,
            &state.config.faucet.message,
        )
        .await;

        match status {
            Ok(_) => {
                record_successful_claim(&state, &task.address).await;
                match state.valkey.add_sent_amount(amount).await {
                    Ok(total_sent_nanotons) => {
                        info!(
                            address = %task.address,
                            amount,
                            total_sent_nanotons,
                            "Recorded sent amount in Valkey"
                        );
                    }
                    Err(err) => {
                        warn!(
                            address = %task.address,
                            amount,
                            error = %err,
                            "Failed to record sent amount in Valkey"
                        );
                    }
                }
                info!("Successfully sent claim to {}", task.address);
                return Ok(());
            }
            Err(err) => {
                if attempt < max_retries {
                    let delay =
                        exponential_backoff(state.config.worker.retry_base_delay_ms, attempt);
                    warn!(
                        address = %task.address,
                        attempt = attempt + 1,
                        max_attempts = max_retries + 1,
                        retry_in_ms = delay.as_millis(),
                        error = %err,
                        "Claim send attempt failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                error!(
                    address = %task.address,
                    attempts = max_retries + 1,
                    error = %err,
                    "Failed to send claim after retries"
                );
                anyhow::bail!("Failed to send claim: {}", err);
            }
        }
    }

    unreachable!("send_claim loop should always return");
}

// TODO: вынести куда-то
async fn can_process_successful_claim_window(
    state: &AppState,
    address: &str,
) -> anyhow::Result<bool> {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return Ok(true);
    };

    let address_key = normalized_address_key(address)?;

    match state
        .valkey
        .check_successful_claim_window(&address_key, window.max_requests, window.window_seconds)
        .await?
    {
        SuccessfulClaimWindowDecision::Allowed {
            current,
            max,
            window_seconds,
        } => {
            info!(
                address = %address,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                "Successful claim window allows send"
            );
            Ok(true)
        }
        SuccessfulClaimWindowDecision::Limited {
            current,
            max,
            window_seconds,
            retry_after_ms,
        } => {
            state
                .record_antifraud_trigger(AntifraudModule::SuccessfulClaimWindow)
                .await;
            warn!(
                address = %address,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                retry_after_ms,
                "Successful claim window limit reached, skipping queued claim"
            );
            Ok(false)
        }
    }
}

async fn record_successful_claim(state: &AppState, address: &str) {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return;
    };

    let address_key = match normalized_address_key(address) {
        Ok(address_key) => address_key,
        Err(err) => {
            warn!(
                address = %address,
                error = %err,
                "Failed to normalize successful claim address"
            );
            return;
        }
    };

    match state
        .valkey
        .record_successful_claim(&address_key, window.window_seconds)
        .await
    {
        Ok(successful_claims) => {
            info!(
                address = %address,
                successful_claims,
                window_seconds = window.window_seconds,
                "Recorded successful claim in Valkey"
            );
        }
        Err(err) => {
            warn!(
                address = %address,
                error = %err,
                "Failed to record successful claim in Valkey"
            );
        }
    }
}

fn normalized_address_key(address: &str) -> anyhow::Result<String> {
    Ok(TonAddress::from_str(address)?.to_hex())
}

async fn wait_for_sent_amount_window(
    state: &AppState,
    address: &str,
    amount: u64,
) -> anyhow::Result<()> {
    let Some(window) = state.antifraud.sent_amount_window() else {
        return Ok(());
    };

    if let Err(err) = state.antifraud.check_sent_amount_window_transfer(amount) {
        state
            .record_antifraud_trigger(AntifraudModule::SentAmountWindow)
            .await;
        error!(
            address = %address,
            amount,
            max_amount = window.max_amount,
            error = ?err,
            "Claim amount exceeds sent amount window limit"
        );
        anyhow::bail!("Claim amount exceeds sent amount window limit: {err:?}");
    }

    let mut trigger_recorded = false;
    loop {
        match state
            .valkey
            .reserve_sent_amount_window(amount, window.max_amount, window.window_seconds)
            .await?
        {
            SentAmountWindowDecision::Reserved(reservation) => {
                info!(
                    address = %address,
                    amount,
                    reserved_total_nanotons = reservation.total,
                    max_amount = reservation.max,
                    window_seconds = reservation.window_seconds,
                    "Reserved sent amount sliding window"
                );
                return Ok(());
            }
            SentAmountWindowDecision::Limited {
                current,
                attempted,
                max,
                window_seconds,
                retry_after_ms,
            } => {
                if !trigger_recorded {
                    state
                        .record_antifraud_trigger(AntifraudModule::SentAmountWindow)
                        .await;
                    trigger_recorded = true;
                }
                warn!(
                    address = %address,
                    current_sent_nanotons = current,
                    attempted_amount = attempted,
                    max_amount = max,
                    window_seconds,
                    retry_after_ms,
                    "Sent amount sliding window limit reached, waiting"
                );
                tokio::time::sleep(StdDuration::from_millis(retry_after_ms.max(1))).await;
            }
        }
    }
}

fn exponential_backoff(base_delay_ms: u64, attempt: u32) -> StdDuration {
    let multiplier = 1u64 << attempt.min(8);
    StdDuration::from_millis(base_delay_ms.saturating_mul(multiplier))
}

async fn process_send_tokens(
    wallet: &Wallet,
    client: &ToncenterClient,
    dest: &str,
    amount: u64,
    message: &str,
) -> anyhow::Result<()> {
    let dest = TonAddress::from_str(dest)?;

    let message_cell = build_message(wallet, amount, dest, message)?;

    let seqno = client.get_wallet_seqno(&wallet.get_address()).await?;

    let expire_at = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        + 600) as u32;

    let external = wallet
        .wallet
        .create_ext_in_msg(vec![message_cell], seqno, expire_at, false)?;

    let response = client.send_boc(&external.to_boc_base64()?).await?;

    if let Some(ok) = response.get("ok").and_then(|v| v.as_bool())
        && !ok
    {
        anyhow::bail!("Toncenter returned ok: false. Response: {:?}", response);
    }

    Ok(())
}

fn build_message(
    wallet: &Wallet,
    amount: u64,
    dest: TonAddress,
    message: &str,
) -> anyhow::Result<TonCell> {
    let message_info = CommonMsgInfoInt {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: wallet.wallet.address.to_msg_address(),
        dst: dest.to_msg_address(),
        value: CurrencyCollection::new(TLBCoins::from_num(&amount)?),
        ihr_fee: TLBCoins::ZERO,
        fwd_fee: TLBCoins::ZERO,
        created_at: 0,
        created_lt: 0,
    };

    let mut message_body_builder = TonCell::builder();
    message_body_builder.write_num(&0u32, 32)?;
    message_body_builder.write_bits(message.as_bytes(), message.len() * 8)?;
    let message_body = message_body_builder.build()?;

    let message = Msg::new(CommonMsgInfo::Int(message_info), message_body);

    Ok(message.to_cell()?)
}
