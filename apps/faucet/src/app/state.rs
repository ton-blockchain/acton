use apalis_sqlite::SqliteStorage;
use faucet_antifraud::Antifraud;
use faucet_config::Config;
use faucet_pow::Pow;
use faucet_valkey::{AntifraudModule, ValkeyStore};
use sqlx::SqlitePool;
use std::sync::Arc;
use toncenter::ToncenterClient;
use tracing::warn;

use crate::antifraud::{audit::AntifraudAuditStore, blacklist::BlacklistStore};
use crate::auth::github::GitHubAuth;
use crate::blockchain::wallet::Wallet;
use crate::handlers::CreateClaim;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) storage: SqliteStorage<CreateClaim>,
    pub(crate) database: SqlitePool,
    pub(super) wallet: Arc<Wallet>,
    pub(crate) client: Arc<ToncenterClient>,
    pub(crate) pow: Pow,
    pub(crate) valkey: ValkeyStore,
    pub(crate) antifraud: Antifraud,
    pub(crate) antifraud_audit: AntifraudAuditStore,
    pub(crate) blacklist: BlacklistStore,
    pub(crate) github_auth: GitHubAuth,
    pub(crate) config: Arc<Config>,
}

impl AppState {
    pub(crate) async fn record_antifraud_trigger(&self, module: AntifraudModule, address: &str) {
        if let Err(err) = self.valkey.increment_antifraud_trigger_count(module).await {
            warn!(
                module = module.name(),
                address,
                error = %err,
                "Failed to record antifraud trigger in Valkey"
            );
        }
    }
}
