use anyhow::Context;
use apalis::layers::WorkerBuilderExt;
use apalis::prelude::{WorkerBuilder, WorkerError};
use apalis_sqlite::SqliteStorage;
use axum::middleware;
use axum_governor::GovernorLayer;
use faucet_antifraud::Antifraud;
use faucet_config::{ClaimRateLimitConfig, Config, DefaultRateLimitConfig};
use faucet_pow::Pow;
use faucet_valkey::ValkeyStore;
use lazy_limit::{Duration, RuleConfig, init_rate_limiter};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::watch;
use toncenter::ToncenterClient;
use tower::ServiceBuilder;
use tracing::{error, info, warn};
use uuid::Uuid;

use self::worker::send_claim;
use crate::LONG_VERSION;
use crate::antifraud::audit::AntifraudAuditStore;
use crate::antifraud::blacklist::BlacklistStore;
use crate::auth::github::GitHubAuth;
use crate::blockchain::wallet::Wallet;
use crate::handlers;
use crate::middlewares::{enter_request_span, insert_client_ip};

mod logger;
mod state;
mod worker;

pub(crate) use state::AppState;

pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logger::init_tracing();
    info!("Starting Faucet server");
    let config = Config::from_env().context("Failed to load config")?;

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);

    info!(
        version = LONG_VERSION,
        bind_addr = %bind_addr,
        database_url = %config.database.url,
        antifraud_database_url = %config.antifraud_database.url,
        toncenter_url = %config.toncenter.url,
        "Loaded startup config"
    );
    if config.faucet.read_only {
        warn!("Faucet read-only mode is enabled; challenges and claims will not be accepted");
    }
    if config.server.proxy.enabled {
        info!(
            header = %config.server.proxy.header,
            ips = ?config.server.proxy.ips,
            "Trusted proxy support enabled"
        );
    }
    if config.github_auth.enabled {
        info!(
            callback_url = %config.github_auth.callback_url,
            frontend_url = %config.github_auth.frontend_url,
            "GitHub authentication enabled"
        );
    }

    info!(database_url = %config.database.url, "Connecting to database");
    let opts = SqliteConnectOptions::from_str(&config.database.url)
        .context("Invalid database URL")?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .pragma("temp_store", "MEMORY")
        .pragma("cache_size", "64000");
    let antifraud_opts = SqliteConnectOptions::from_str(&config.antifraud_database.url)
        .context("Invalid antifraud database URL")?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(StdDuration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts.clone())
        .await
        .context("Failed to connect to database")?;
    info!("Connected to database");

    info!(
        database_url = %config.antifraud_database.url,
        "Connecting to antifraud database"
    );
    let antifraud_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(antifraud_opts.clone())
        .await
        .context("Failed to connect to antifraud database")?;
    ensure_distinct_database_files(&opts, &antifraud_opts)?;
    info!("Connected to antifraud database");

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

    SqliteStorage::migrations()
        .run(&pool)
        .await
        .context("Failed to setup storage")?;
    let antifraud_audit = AntifraudAuditStore::setup(antifraud_pool)
        .await
        .context("Failed to setup antifraud audit")?;
    info!("Initialized antifraud audit");
    let blacklist = BlacklistStore::setup(pool.clone())
        .await
        .context("Failed to setup antifraud blacklist")?;
    let active_bans = blacklist
        .active_entries()
        .await
        .context("Failed to load active antifraud bans")?;
    info!(
        active_bans = active_bans.len(),
        "Initialized antifraud blacklist"
    );
    let mut bans_by_subject = BTreeMap::<_, usize>::new();
    for ban in &active_bans {
        let subject = ban
            .subject
            .split_once(':')
            .map_or(ban.subject.as_str(), |(kind, _)| kind);
        *bans_by_subject.entry(subject).or_default() += 1;
    }
    for (subject, count) in bans_by_subject {
        info!(subject, count, "Active antifraud bans");
    }
    let (storage_pool, claim_listener) = SqliteStorage::connect_with_callback(&config.database.url)
        .context("Failed to create claim storage pool")?;
    let storage = SqliteStorage::new(&storage_pool);
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
    let github_auth = GitHubAuth::new(config.github_auth.clone(), valkey.clone())
        .context("Failed to create GitHub authentication service")?;

    let shared_state = AppState {
        storage: storage.clone(),
        database: pool,
        wallet: Arc::new(wallet),
        client: client.clone(),
        pow: Pow::new(config.pow.difficulty),
        valkey,
        antifraud,
        antifraud_audit,
        blacklist,
        github_auth,
        config: Arc::new(config),
    };

    let worker_state = shared_state.clone();

    let worker_name = format!("claim-worker-{}", Uuid::new_v4());
    let worker = WorkerBuilder::new(worker_name.as_str())
        .backend(storage.with_callback(claim_listener))
        .concurrency(1)
        .data(worker_state)
        .build(send_claim);

    let proxy = shared_state.config.server.proxy.clone();
    let app = handlers::router(shared_state).layer(
        ServiceBuilder::new()
            .layer(middleware::from_fn(enter_request_span))
            .layer(middleware::from_fn_with_state(proxy, insert_client_ip))
            .layer(GovernorLayer::default()),
    );

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context("Failed to bind TCP listener")?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let worker_shutdown_rx = shutdown_rx.clone();
    let worker_shutdown_tx = shutdown_tx.clone();
    let worker_future = async move {
        info!(worker = %worker_name, "Starting claim worker");
        let result = worker
            .run_until(async move {
                wait_for_shutdown(worker_shutdown_rx).await;
                Ok::<(), WorkerError>(())
            })
            .await;

        let _ = worker_shutdown_tx.send(true);
        match result {
            Ok(()) => {
                info!(worker = %worker_name, "Claim worker stopped");
                Ok(())
            }
            Err(err) => {
                error!(worker = %worker_name, error = %err, "Worker failed");
                Err(err).context("Claim worker failed")
            }
        }
    };

    let signal_shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = signal_shutdown_tx.send(true);
    });

    info!("Listening on {}", bind_addr);
    info!("Started Faucet server");
    let server_shutdown_tx = shutdown_tx;
    let server_future = async move {
        let result = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_shutdown(shutdown_rx))
        .await;

        let shutdown_requested = *server_shutdown_tx.borrow();
        let _ = server_shutdown_tx.send(true);
        result.context("HTTP server exited with error")?;

        if !shutdown_requested {
            anyhow::bail!("HTTP server stopped unexpectedly");
        }

        Ok(())
    };

    let (server_result, worker_result) = tokio::join!(server_future, worker_future);
    server_result?;
    info!("Stopped Faucet server");
    worker_result?;

    Ok(())
}

fn ensure_distinct_database_files(
    database: &SqliteConnectOptions,
    antifraud_database: &SqliteConnectOptions,
) -> anyhow::Result<()> {
    let database_path = canonical_database_file(database, "DATABASE_URL")?;
    let antifraud_database_path =
        canonical_database_file(antifraud_database, "ANTIFRAUD_DATABASE_URL")?;

    anyhow::ensure!(
        database_path != antifraud_database_path,
        "DATABASE_URL and ANTIFRAUD_DATABASE_URL must point to different SQLite files"
    );
    Ok(())
}

fn canonical_database_file(
    options: &SqliteConnectOptions,
    variable: &str,
) -> anyhow::Result<PathBuf> {
    let path = options.get_filename();
    anyhow::ensure!(
        path != std::path::Path::new(":memory:"),
        "{variable} must point to a SQLite file"
    );
    dunce::canonicalize(path).with_context(|| {
        format!(
            "Failed to resolve SQLite file from {variable}: {}",
            path.display()
        )
    })
}

async fn wait_for_shutdown(mut shutdown: watch::Receiver<bool>) {
    while !*shutdown.borrow() {
        if shutdown.changed().await.is_err() {
            return;
        }
    }
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
