use anyhow::Context;
use faucet_config::ValkeyConfig;

const SENT_AMOUNT_WINDOW_KEY: &str = "faucet:antifraud:sent-amount-window";
const SENT_AMOUNT_WINDOW_SEQ_KEY: &str = "faucet:antifraud:sent-amount-window:seq";
const SENT_AMOUNT_WINDOW_SCRIPT: &str = include_str!("../scripts/reserve_sliding_window.lua");

const SUCCESSFUL_CLAIM_WINDOW_KEY_PREFIX: &str = "faucet:antifraud:successful-claim-window";
const SUCCESSFUL_CLAIM_WINDOW_SEQ_KEY_PREFIX: &str = "faucet:antifraud:successful-claim-window:seq";
const CHECK_SUCCESSFUL_CLAIM_WINDOW_SCRIPT: &str =
    include_str!("../scripts/check_successful_claim_window.lua");
const RECORD_SUCCESSFUL_CLAIM_SCRIPT: &str = include_str!("../scripts/record_successful_claim.lua");

const TOTAL_SENT_NANOTONS_KEY: &str = "faucet:stats:sent-nanotons";
const ANTIFRAUD_TRIGGER_COUNT_KEY_PREFIX: &str = "faucet:stats:antifraud";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AntifraudModule {
    WalletBalance,
    SentAmountWindow,
    SuccessfulClaimWindow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AntifraudStats {
    pub wallet_balance: u64,
    pub sent_amount_window: u64,
    pub successful_claim_window: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FaucetStats {
    pub total_sent_nanotons: u64,
    pub antifraud: AntifraudStats,
}

impl AntifraudModule {
    pub const fn name(self) -> &'static str {
        match self {
            Self::WalletBalance => "wallet-balance",
            Self::SentAmountWindow => "sent-amount-window",
            Self::SuccessfulClaimWindow => "successful-claim-window",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentAmountWindowReservation {
    pub id: String,
    pub total: u64,
    pub max: u64,
    pub window_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SentAmountWindowDecision {
    Reserved(SentAmountWindowReservation),
    Limited {
        current: u64,
        attempted: u64,
        max: u64,
        window_seconds: u64,
        retry_after_ms: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SuccessfulClaimWindowDecision {
    Allowed {
        current: u32,
        max: u32,
        window_seconds: u64,
    },
    Limited {
        current: u32,
        max: u32,
        window_seconds: u64,
        retry_after_ms: u64,
    },
}

#[derive(Clone)]
pub struct ValkeyStore {
    connection: redis::aio::MultiplexedConnection,
}

impl ValkeyStore {
    pub async fn new(config: &ValkeyConfig) -> anyhow::Result<Self> {
        let client = redis::Client::open(config.uri.as_str()).context("Invalid Valkey URI")?;
        let connection = client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to connect to Valkey")?;

        Ok(Self { connection })
    }

    pub async fn add_sent_amount(&self, amount: u64) -> anyhow::Result<u64> {
        let mut connection = self.connection.clone();
        redis::cmd("INCRBY")
            .arg(TOTAL_SENT_NANOTONS_KEY)
            .arg(amount)
            .query_async(&mut connection)
            .await
            .context("Failed to increment total sent amount")
    }

    pub async fn increment_antifraud_trigger_count(
        &self,
        module: AntifraudModule,
    ) -> anyhow::Result<u64> {
        let mut connection = self.connection.clone();
        redis::cmd("INCR")
            .arg(antifraud_trigger_count_key(module))
            .query_async(&mut connection)
            .await
            .context("Failed to increment antifraud trigger count")
    }

    pub async fn get_stats(&self) -> anyhow::Result<FaucetStats> {
        let mut connection = self.connection.clone();
        let values: (Option<u64>, Option<u64>, Option<u64>, Option<u64>) = redis::cmd("MGET")
            .arg(TOTAL_SENT_NANOTONS_KEY)
            .arg(antifraud_trigger_count_key(AntifraudModule::WalletBalance))
            .arg(antifraud_trigger_count_key(
                AntifraudModule::SentAmountWindow,
            ))
            .arg(antifraud_trigger_count_key(
                AntifraudModule::SuccessfulClaimWindow,
            ))
            .query_async(&mut connection)
            .await
            .context("Failed to get faucet stats")?;

        Ok(FaucetStats {
            total_sent_nanotons: values.0.unwrap_or_default(),
            antifraud: AntifraudStats {
                wallet_balance: values.1.unwrap_or_default(),
                sent_amount_window: values.2.unwrap_or_default(),
                successful_claim_window: values.3.unwrap_or_default(),
            },
        })
    }

    pub async fn reserve_sent_amount_window(
        &self,
        amount: u64,
        max_amount: u64,
        window_seconds: u64,
    ) -> anyhow::Result<SentAmountWindowDecision> {
        anyhow::ensure!(window_seconds > 0, "Sent amount window must be positive");

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);

        let mut connection = self.connection.clone();
        let response: (u64, u64, String, u64) = redis::Script::new(SENT_AMOUNT_WINDOW_SCRIPT)
            .key(SENT_AMOUNT_WINDOW_KEY)
            .key(SENT_AMOUNT_WINDOW_SEQ_KEY)
            .arg(amount)
            .arg(max_amount)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to reserve sent amount window")?;

        let decision = response.0;
        let current_or_total = response.1;
        let retry_after_ms = response.3;

        match decision {
            1 => Ok(SentAmountWindowDecision::Reserved(
                SentAmountWindowReservation {
                    id: response.2,
                    total: current_or_total,
                    max: max_amount,
                    window_seconds,
                },
            )),
            0 => Ok(SentAmountWindowDecision::Limited {
                current: current_or_total,
                attempted: amount,
                max: max_amount,
                window_seconds,
                retry_after_ms,
            }),
            value => anyhow::bail!("Unexpected sent amount window decision: {value}"),
        }
    }

    pub async fn check_successful_claim_window(
        &self,
        address: &str,
        max_requests: u32,
        window_seconds: u64,
    ) -> anyhow::Result<SuccessfulClaimWindowDecision> {
        anyhow::ensure!(
            window_seconds > 0,
            "Successful claim window must be positive"
        );

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);
        let mut connection = self.connection.clone();
        let response: (u64, u32, u64) = redis::Script::new(CHECK_SUCCESSFUL_CLAIM_WINDOW_SCRIPT)
            .key(successful_claim_window_key(address))
            .arg(max_requests)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to check successful claim window")?;

        match response.0 {
            1 => Ok(SuccessfulClaimWindowDecision::Allowed {
                current: response.1,
                max: max_requests,
                window_seconds,
            }),
            0 => Ok(SuccessfulClaimWindowDecision::Limited {
                current: response.1,
                max: max_requests,
                window_seconds,
                retry_after_ms: response.2,
            }),
            value => anyhow::bail!("Unexpected successful claim window decision: {value}"),
        }
    }

    pub async fn record_successful_claim(
        &self,
        address: &str,
        window_seconds: u64,
    ) -> anyhow::Result<u32> {
        anyhow::ensure!(
            window_seconds > 0,
            "Successful claim window must be positive"
        );

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);
        let mut connection = self.connection.clone();
        redis::Script::new(RECORD_SUCCESSFUL_CLAIM_SCRIPT)
            .key(successful_claim_window_key(address))
            .key(successful_claim_window_seq_key(address))
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to record successful claim")
    }
}

fn successful_claim_window_key(address: &str) -> String {
    format!("{SUCCESSFUL_CLAIM_WINDOW_KEY_PREFIX}:{address}")
}

fn successful_claim_window_seq_key(address: &str) -> String {
    format!("{SUCCESSFUL_CLAIM_WINDOW_SEQ_KEY_PREFIX}:{address}")
}

fn antifraud_trigger_count_key(module: AntifraudModule) -> String {
    format!("{ANTIFRAUD_TRIGGER_COUNT_KEY_PREFIX}:{}", module.name())
}

#[cfg(test)]
mod tests {
    use super::{
        AntifraudModule, antifraud_trigger_count_key, successful_claim_window_key,
        successful_claim_window_seq_key,
    };

    #[test]
    fn accepts_plain_and_tls_valkey_uris() {
        redis::Client::open("redis://127.0.0.1:6379/0").unwrap();
        redis::Client::open("rediss://user:password@hostname").unwrap();
    }

    #[test]
    fn builds_successful_claim_window_keys_from_canonical_address() {
        let address = "0:e4d954ef9f4e1250a26b5bbad76a1cdd17cfd08babad6f4c23e372270aef6f76";

        assert_eq!(
            successful_claim_window_key(address),
            "faucet:antifraud:successful-claim-window:0:e4d954ef9f4e1250a26b5bbad76a1cdd17cfd08babad6f4c23e372270aef6f76"
        );
        assert_eq!(
            successful_claim_window_seq_key(address),
            "faucet:antifraud:successful-claim-window:seq:0:e4d954ef9f4e1250a26b5bbad76a1cdd17cfd08babad6f4c23e372270aef6f76"
        );
    }

    #[test]
    fn builds_antifraud_stat_key_for_each_module() {
        assert_eq!(
            antifraud_trigger_count_key(AntifraudModule::WalletBalance),
            "faucet:stats:antifraud:wallet-balance"
        );
        assert_eq!(
            antifraud_trigger_count_key(AntifraudModule::SentAmountWindow),
            "faucet:stats:antifraud:sent-amount-window"
        );
        assert_eq!(
            antifraud_trigger_count_key(AntifraudModule::SuccessfulClaimWindow),
            "faucet:stats:antifraud:successful-claim-window"
        );
    }
}
