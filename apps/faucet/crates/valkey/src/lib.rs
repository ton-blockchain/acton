use anyhow::Context;
use faucet_config::ValkeyConfig;

const SENT_AMOUNT_WINDOW_KEY: &str = "faucet:antifraud:sent-amount-window";
const SENT_AMOUNT_WINDOW_SEQ_KEY: &str = "faucet:antifraud:sent-amount-window:seq";
const SENT_AMOUNT_WINDOW_SCRIPT: &str = include_str!("../scripts/reserve_sliding_window.lua");

const SUBNET_AMOUNT_WINDOW_KEY_PREFIX: &str = "faucet:antifraud:subnet-amount-window";
const SUBNET_AMOUNT_WINDOW_SEQ_KEY_PREFIX: &str = "faucet:antifraud:subnet-amount-window:seq";
const CHECK_SUBNET_AMOUNT_WINDOW_SCRIPT: &str = include_str!("../scripts/check_amount_window.lua");
const RECORD_SUBNET_AMOUNT_WINDOW_SCRIPT: &str =
    include_str!("../scripts/record_amount_window.lua");

const SUCCESSFUL_CLAIM_WINDOW_KEY_PREFIX: &str = "faucet:antifraud:successful-claim-window";
const SUCCESSFUL_CLAIM_WINDOW_SEQ_KEY_PREFIX: &str = "faucet:antifraud:successful-claim-window:seq";
const CHECK_SUCCESSFUL_CLAIM_WINDOW_SCRIPT: &str =
    include_str!("../scripts/check_successful_claim_window.lua");
const RECORD_SUCCESSFUL_CLAIM_SCRIPT: &str = include_str!("../scripts/record_successful_claim.lua");
const STORE_CAPPED_EPHEMERAL_SCRIPT: &str = include_str!("../scripts/store_capped_ephemeral.lua");
const TAKE_CAPPED_EPHEMERAL_SCRIPT: &str = include_str!("../scripts/take_capped_ephemeral.lua");

// Keep the existing key so deployments retain their accumulated stats.
const TOTAL_SENT_NANOGRAMS_KEY: &str = "faucet:stats:sent-nanotons";
const ANTIFRAUD_TRIGGER_COUNT_KEY_PREFIX: &str = "faucet:stats:antifraud";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AntifraudModule {
    WalletBalance,
    SentAmountWindow,
    SubnetAmountWindow,
    SuccessfulClaimWindow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AntifraudStats {
    pub wallet_balance: u64,
    pub sent_amount_window: u64,
    pub subnet_amount_window: u64,
    pub successful_claim_window: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FaucetStats {
    pub total_sent_nanograms: u64,
    pub antifraud: AntifraudStats,
}

impl AntifraudModule {
    pub const fn name(self) -> &'static str {
        match self {
            Self::WalletBalance => "wallet-balance",
            Self::SentAmountWindow => "sent-amount-window",
            Self::SubnetAmountWindow => "subnet-amount-window",
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
pub enum AmountWindowDecision {
    Allowed {
        current: u64,
        attempted: u64,
        max: u64,
        window_seconds: u64,
    },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CappedEphemeralStoreDecision {
    Stored,
    Full,
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
            .arg(TOTAL_SENT_NANOGRAMS_KEY)
            .arg(amount)
            .query_async(&mut connection)
            .await
            .context("Failed to increment total sent amount")
    }

    pub async fn store_ephemeral(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: u64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(ttl_seconds > 0, "Ephemeral value TTL must be positive");

        let mut connection = self.connection.clone();
        redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut connection)
            .await
            .context("Failed to store ephemeral value")
    }

    pub async fn store_capped_ephemeral(
        &self,
        index_key: &str,
        key: &str,
        value: &str,
        ttl_seconds: u64,
        max_entries: u64,
    ) -> anyhow::Result<CappedEphemeralStoreDecision> {
        anyhow::ensure!(ttl_seconds > 0, "Ephemeral value TTL must be positive");
        anyhow::ensure!(max_entries > 0, "Ephemeral value cap must be positive");

        let mut connection = self.connection.clone();
        let response: i64 = redis::Script::new(STORE_CAPPED_EPHEMERAL_SCRIPT)
            .key(index_key)
            .key(key)
            .arg(value)
            .arg(ttl_seconds)
            .arg(max_entries)
            .invoke_async(&mut connection)
            .await
            .context("Failed to store capped ephemeral value")?;

        match response {
            1 => Ok(CappedEphemeralStoreDecision::Stored),
            0 => Ok(CappedEphemeralStoreDecision::Full),
            -1 => anyhow::bail!("Capped ephemeral value already exists"),
            value => anyhow::bail!("Unexpected capped ephemeral store decision: {value}"),
        }
    }

    pub async fn get_ephemeral(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut connection = self.connection.clone();
        redis::cmd("GET")
            .arg(key)
            .query_async(&mut connection)
            .await
            .context("Failed to get ephemeral value")
    }

    pub async fn take_ephemeral(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut connection = self.connection.clone();
        redis::cmd("GETDEL")
            .arg(key)
            .query_async(&mut connection)
            .await
            .context("Failed to take ephemeral value")
    }

    pub async fn take_capped_ephemeral(
        &self,
        index_key: &str,
        key: &str,
    ) -> anyhow::Result<Option<String>> {
        let mut connection = self.connection.clone();
        redis::Script::new(TAKE_CAPPED_EPHEMERAL_SCRIPT)
            .key(index_key)
            .key(key)
            .invoke_async(&mut connection)
            .await
            .context("Failed to take capped ephemeral value")
    }

    pub async fn delete_ephemeral(&self, key: &str) -> anyhow::Result<bool> {
        let mut connection = self.connection.clone();
        let removed: u64 = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut connection)
            .await
            .context("Failed to delete ephemeral value")?;
        Ok(removed > 0)
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
        let values: Vec<Option<u64>> = redis::cmd("MGET")
            .arg(TOTAL_SENT_NANOGRAMS_KEY)
            .arg(antifraud_trigger_count_key(AntifraudModule::WalletBalance))
            .arg(antifraud_trigger_count_key(
                AntifraudModule::SentAmountWindow,
            ))
            .arg(antifraud_trigger_count_key(
                AntifraudModule::SubnetAmountWindow,
            ))
            .arg(antifraud_trigger_count_key(
                AntifraudModule::SuccessfulClaimWindow,
            ))
            .query_async(&mut connection)
            .await
            .context("Failed to get faucet stats")?;
        let value = |index| values.get(index).copied().flatten().unwrap_or_default();

        Ok(FaucetStats {
            total_sent_nanograms: value(0),
            antifraud: AntifraudStats {
                wallet_balance: value(1),
                sent_amount_window: value(2),
                subnet_amount_window: value(3),
                successful_claim_window: value(4),
            },
        })
    }

    pub async fn reserve_sent_amount_window(
        &self,
        amount: u64,
        max_amount: u64,
        window_seconds: u64,
    ) -> anyhow::Result<SentAmountWindowDecision> {
        self.reserve_amount_window(
            SENT_AMOUNT_WINDOW_KEY,
            SENT_AMOUNT_WINDOW_SEQ_KEY,
            amount,
            max_amount,
            window_seconds,
            "sent amount",
        )
        .await
    }

    pub async fn check_subnet_amount_window(
        &self,
        subject: &str,
        amount: u64,
        max_amount: u64,
        window_seconds: u64,
    ) -> anyhow::Result<AmountWindowDecision> {
        anyhow::ensure!(window_seconds > 0, "Subnet amount window must be positive");

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);
        let mut connection = self.connection.clone();
        let response: (u64, u64, u64) = redis::Script::new(CHECK_SUBNET_AMOUNT_WINDOW_SCRIPT)
            .key(subnet_amount_window_key(subject))
            .arg(amount)
            .arg(max_amount)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to check subnet amount window")?;

        match response.0 {
            1 => Ok(AmountWindowDecision::Allowed {
                current: response.1,
                attempted: amount,
                max: max_amount,
                window_seconds,
            }),
            0 => Ok(AmountWindowDecision::Limited {
                current: response.1,
                attempted: amount,
                max: max_amount,
                window_seconds,
                retry_after_ms: response.2,
            }),
            value => anyhow::bail!("Unexpected subnet amount window decision: {value}"),
        }
    }

    pub async fn record_subnet_amount_window(
        &self,
        subject: &str,
        amount: u64,
        window_seconds: u64,
    ) -> anyhow::Result<u64> {
        anyhow::ensure!(window_seconds > 0, "Subnet amount window must be positive");

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);
        let mut connection = self.connection.clone();
        redis::Script::new(RECORD_SUBNET_AMOUNT_WINDOW_SCRIPT)
            .key(subnet_amount_window_key(subject))
            .key(subnet_amount_window_seq_key(subject))
            .arg(amount)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .context("Failed to record subnet amount window")
    }

    async fn reserve_amount_window(
        &self,
        window_key: &str,
        sequence_key: &str,
        amount: u64,
        max_amount: u64,
        window_seconds: u64,
        name: &str,
    ) -> anyhow::Result<SentAmountWindowDecision> {
        anyhow::ensure!(window_seconds > 0, "Sent amount window must be positive");

        let ttl_seconds = window_seconds.saturating_mul(2).max(1);

        let mut connection = self.connection.clone();
        let response: (u64, u64, String, u64) = redis::Script::new(SENT_AMOUNT_WINDOW_SCRIPT)
            .key(window_key)
            .key(sequence_key)
            .arg(amount)
            .arg(max_amount)
            .arg(window_seconds)
            .arg(ttl_seconds)
            .invoke_async(&mut connection)
            .await
            .with_context(|| format!("Failed to reserve {name} window"))?;

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

fn subnet_amount_window_key(subject: &str) -> String {
    format!("{SUBNET_AMOUNT_WINDOW_KEY_PREFIX}:{subject}")
}

fn subnet_amount_window_seq_key(subject: &str) -> String {
    format!("{SUBNET_AMOUNT_WINDOW_SEQ_KEY_PREFIX}:{subject}")
}

fn antifraud_trigger_count_key(module: AntifraudModule) -> String {
    format!("{ANTIFRAUD_TRIGGER_COUNT_KEY_PREFIX}:{}", module.name())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use faucet_config::ValkeyConfig;

    use super::{
        AmountWindowDecision, AntifraudModule, CappedEphemeralStoreDecision, ValkeyStore,
        antifraud_trigger_count_key, subnet_amount_window_key, subnet_amount_window_seq_key,
        successful_claim_window_key, successful_claim_window_seq_key,
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
            antifraud_trigger_count_key(AntifraudModule::SubnetAmountWindow),
            "faucet:stats:antifraud:subnet-amount-window"
        );
        assert_eq!(
            antifraud_trigger_count_key(AntifraudModule::SuccessfulClaimWindow),
            "faucet:stats:antifraud:successful-claim-window"
        );
    }

    #[test]
    fn builds_subnet_amount_window_keys_from_subject() {
        let subject = "client-subnet:203.0.113.0/24";

        assert_eq!(
            subnet_amount_window_key(subject),
            "faucet:antifraud:subnet-amount-window:client-subnet:203.0.113.0/24"
        );
        assert_eq!(
            subnet_amount_window_seq_key(subject),
            "faucet:antifraud:subnet-amount-window:seq:client-subnet:203.0.113.0/24"
        );
    }

    #[tokio::test]
    async fn records_only_sent_amounts_and_isolates_subnet_windows() {
        let Ok(uri) = std::env::var("VALKEY_TEST_URI") else {
            return;
        };
        let store = ValkeyStore::new(&ValkeyConfig { uri }).await.unwrap();
        let namespace = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let first_subject = format!("client-subnet:test-{namespace}-first");
        let second_subject = format!("client-subnet:test-{namespace}-second");

        assert_eq!(
            store
                .check_subnet_amount_window(&first_subject, 6, 10, 60)
                .await
                .unwrap(),
            AmountWindowDecision::Allowed {
                current: 0,
                attempted: 6,
                max: 10,
                window_seconds: 60,
            }
        );
        assert_eq!(
            store
                .check_subnet_amount_window(&first_subject, 6, 10, 60)
                .await
                .unwrap(),
            AmountWindowDecision::Allowed {
                current: 0,
                attempted: 6,
                max: 10,
                window_seconds: 60,
            }
        );
        assert_eq!(
            store
                .record_subnet_amount_window(&first_subject, 6, 60)
                .await
                .unwrap(),
            6
        );

        match store
            .check_subnet_amount_window(&first_subject, 5, 10, 60)
            .await
            .unwrap()
        {
            AmountWindowDecision::Limited {
                current,
                attempted,
                max,
                window_seconds,
                retry_after_ms,
            } => {
                assert_eq!(current, 6);
                assert_eq!(attempted, 5);
                assert_eq!(max, 10);
                assert_eq!(window_seconds, 60);
                assert!(retry_after_ms > 0);
            }
            AmountWindowDecision::Allowed { .. } => {
                panic!("send above the subnet limit must be rejected");
            }
        }

        assert_eq!(
            store
                .check_subnet_amount_window(&second_subject, 6, 10, 60)
                .await
                .unwrap(),
            AmountWindowDecision::Allowed {
                current: 0,
                attempted: 6,
                max: 10,
                window_seconds: 60,
            }
        );
    }

    #[tokio::test]
    async fn caps_active_ephemeral_values_and_reclaims_slots() {
        let Ok(uri) = std::env::var("VALKEY_TEST_URI") else {
            return;
        };
        let store = ValkeyStore::new(&ValkeyConfig { uri }).await.unwrap();
        let namespace = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let index_key = format!("faucet:test:{{capped-{namespace}}}:active");
        let first_key = format!("faucet:test:{{capped-{namespace}}}:first");
        let second_key = format!("faucet:test:{{capped-{namespace}}}:second");

        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &first_key, "first", 1, 1)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );
        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &second_key, "second", 1, 1)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Full
        );
        assert_eq!(
            store
                .take_capped_ephemeral(&index_key, &first_key)
                .await
                .unwrap()
                .as_deref(),
            Some("first")
        );
        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &second_key, "second", 1, 1)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &first_key, "first", 1, 1)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );

        store
            .take_capped_ephemeral(&index_key, &first_key)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn preserves_cap_when_new_values_have_shorter_ttl() {
        let Ok(uri) = std::env::var("VALKEY_TEST_URI") else {
            return;
        };
        let store = ValkeyStore::new(&ValkeyConfig { uri }).await.unwrap();
        let namespace = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let index_key = format!("faucet:test:{{mixed-ttl-{namespace}}}:active");
        let first_key = format!("faucet:test:{{mixed-ttl-{namespace}}}:first");
        let second_key = format!("faucet:test:{{mixed-ttl-{namespace}}}:second");
        let third_key = format!("faucet:test:{{mixed-ttl-{namespace}}}:third");
        let fourth_key = format!("faucet:test:{{mixed-ttl-{namespace}}}:fourth");

        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &first_key, "first", 30, 2)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );
        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &second_key, "second", 1, 2)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );

        tokio::time::sleep(Duration::from_millis(1_100)).await;

        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &third_key, "third", 2, 2)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Stored
        );
        assert_eq!(
            store
                .store_capped_ephemeral(&index_key, &fourth_key, "fourth", 2, 2)
                .await
                .unwrap(),
            CappedEphemeralStoreDecision::Full
        );

        store
            .take_capped_ephemeral(&index_key, &first_key)
            .await
            .unwrap();
        store
            .take_capped_ephemeral(&index_key, &third_key)
            .await
            .unwrap();
    }
}
