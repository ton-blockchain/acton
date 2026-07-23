use anyhow::Context;
use std::str::FromStr;

const NANOTONS_PER_TON: u64 = 1_000_000_000;

#[derive(Clone, Debug)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub rate_limit: RateLimitConfig,
    pub toncenter: ToncenterConfig,
    pub worker: WorkerConfig,
    pub faucet: FaucetConfig,
    pub pow: PowConfig,
    pub valkey: ValkeyConfig,
    pub antifraud: AntifraudConfig,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub default: DefaultRateLimitConfig,
    pub claim: ClaimRateLimitConfig,
}

#[derive(Clone, Debug)]
pub struct DefaultRateLimitConfig {
    pub window_seconds: u64,
    pub max_requests: u32,
}

#[derive(Clone, Debug)]
pub struct ClaimRateLimitConfig {
    pub window_seconds: u64,
    pub max_requests: u32,
}

#[derive(Clone, Debug)]
pub struct ToncenterConfig {
    pub api_key: Option<String>,
    pub url: String,
    pub timeout_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct FaucetConfig {
    pub mnemonic: String,
    pub amount: u64,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct PowConfig {
    pub enabled: bool,
    pub difficulty: u32,
    pub challenge_ttl_seconds: u64,
    pub max_challenges: u64,
    pub client: PowClientConfig,
}

#[derive(Clone, Debug)]
pub struct PowClientConfig {
    pub max_solve_ttl_seconds: u64,
    pub max_nonce_attempts: u64,
}

#[derive(Clone, Debug)]
pub struct ValkeyConfig {
    pub uri: String,
}

#[derive(Clone, Debug)]
pub struct AntifraudConfig {
    pub enabled: bool,
    pub wallet_balance: WalletBalanceCheckConfig,
    pub sent_amount_window: SentAmountWindowCheckConfig,
    pub successful_claim_window: SuccessfulClaimWindowCheckConfig,
}

#[derive(Clone, Debug)]
pub struct WalletBalanceCheckConfig {
    pub enabled: bool,
    pub max_wallet_balance: u64,
}

#[derive(Clone, Debug)]
pub struct SentAmountWindowCheckConfig {
    pub enabled: bool,
    pub max_amount: u64,
    pub window_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct SuccessfulClaimWindowCheckConfig {
    pub enabled: bool,
    pub max_requests: u32,
    pub window_seconds: u64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config {
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "sqlite:./db.sqlite".to_string()),
            },
            server: ServerConfig {
                host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: parse_env_number("PORT", 3001),
            },
            rate_limit: RateLimitConfig {
                default: DefaultRateLimitConfig {
                    window_seconds: parse_env_number("RATE_LIMIT_DEFAULT_WINDOW_SECONDS", 1),
                    max_requests: parse_env_number("RATE_LIMIT_DEFAULT_MAX_REQUESTS", 5),
                },
                claim: ClaimRateLimitConfig {
                    window_seconds: parse_env_number("RATE_LIMIT_CLAIM_WINDOW_SECONDS", 86_400),
                    max_requests: parse_env_number("RATE_LIMIT_CLAIM_MAX_REQUESTS", 100),
                },
            },
            toncenter: ToncenterConfig {
                api_key: std::env::var("TONCENTER_API_KEY").ok(),
                url: std::env::var("TONCENTER_URL")
                    .unwrap_or_else(|_| "https://testnet.toncenter.com".to_string()),
                timeout_seconds: parse_env_number("TONCENTER_TIMEOUT_SECONDS", 10),
                connect_timeout_seconds: parse_env_number("TONCENTER_CONNECT_TIMEOUT_SECONDS", 5),
                max_retries: parse_env_number("TONCENTER_MAX_RETRIES", 3),
                retry_base_delay_ms: parse_env_number("TONCENTER_RETRY_BASE_DELAY_MS", 500),
            },
            worker: WorkerConfig {
                max_retries: parse_env_number("WORKER_MAX_RETRIES", 2),
                retry_base_delay_ms: parse_env_number("WORKER_RETRY_BASE_DELAY_MS", 1_000),
            },
            faucet: FaucetConfig {
                mnemonic: std::env::var("FAUCET_MNEMONIC")
                    .context("FAUCET_MNEMONIC must be set")?,
                amount: parse_env_nanotons("FAUCET_AMOUNT_NANOTONS", 1_000_000),
                message: std::env::var("FAUCET_MESSAGE")
                    .unwrap_or_else(|_| "Testnet faucet".to_string()),
            },
            pow: PowConfig {
                enabled: parse_env_bool("POW_ENABLED", true),
                difficulty: parse_env_number("POW_DIFFICULTY", 21),
                challenge_ttl_seconds: parse_env_number("POW_CHALLENGE_TTL_SECONDS", 300),
                max_challenges: parse_env_number("POW_MAX_CHALLENGES", 10_000),
                client: PowClientConfig {
                    max_solve_ttl_seconds: parse_env_number(
                        "POW_CLIENT_MAX_SOLVE_TTL_SECONDS",
                        300,
                    ),
                    max_nonce_attempts: parse_env_number(
                        "POW_CLIENT_MAX_NONCE_ATTEMPTS",
                        1_000_000_000,
                    ),
                },
            },
            valkey: ValkeyConfig {
                uri: std::env::var("VALKEY_URI")
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            },
            antifraud: AntifraudConfig {
                enabled: parse_env_bool("ANTIFRAUD_ENABLED", true),
                wallet_balance: WalletBalanceCheckConfig {
                    enabled: parse_env_bool("ANTIFRAUD_WALLET_BALANCE_ENABLED", true),
                    max_wallet_balance: parse_env_nanotons(
                        "ANTIFRAUD_WALLET_BALANCE_MAX_NANOTONS",
                        25_000_000_000,
                    ),
                },
                sent_amount_window: SentAmountWindowCheckConfig {
                    enabled: parse_env_bool("ANTIFRAUD_SENT_AMOUNT_WINDOW_ENABLED", true),
                    max_amount: parse_env_nanotons(
                        "ANTIFRAUD_SENT_AMOUNT_WINDOW_MAX_NANOTONS",
                        10_000_000_000,
                    ),
                    window_seconds: parse_env_number("ANTIFRAUD_SENT_AMOUNT_WINDOW_SECONDS", 60),
                },
                successful_claim_window: SuccessfulClaimWindowCheckConfig {
                    enabled: parse_env_bool("ANTIFRAUD_SUCCESSFUL_CLAIM_WINDOW_ENABLED", true),
                    max_requests: parse_env_number(
                        "ANTIFRAUD_SUCCESSFUL_CLAIM_WINDOW_MAX_REQUESTS",
                        2,
                    ),
                    window_seconds: parse_env_number(
                        "ANTIFRAUD_SUCCESSFUL_CLAIM_WINDOW_SECONDS",
                        86_400,
                    ),
                },
            },
        };

        Ok(config)
    }
}

fn parse_env_number<T>(name: &str, default: T) -> T
where
    T: FromStr,
{
    std::env::var(name)
        .ok()
        .and_then(|value| parse_number(&value))
        .unwrap_or(default)
}

fn parse_number<T>(value: &str) -> Option<T>
where
    T: FromStr,
{
    value.replace('_', "").parse().ok()
}

fn parse_env_nanotons(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| parse_nanotons(&value))
        .unwrap_or(default)
}

fn parse_nanotons(value: &str) -> Option<u64> {
    let value = value.trim();
    let normalized = value.to_ascii_lowercase();

    if let Some(ton_amount) = normalized.strip_suffix("ton") {
        parse_ton_amount(ton_amount)
    } else {
        parse_number(value)
    }
}

fn parse_ton_amount(value: &str) -> Option<u64> {
    let tons = value.trim().replace('_', "").parse::<f64>().ok()?;
    if !tons.is_finite() || tons < 0.0 {
        return None;
    }

    let nanotons = tons * NANOTONS_PER_TON as f64;
    let rounded = nanotons.round();
    if (nanotons - rounded).abs() > 0.000_001 || rounded > u64::MAX as f64 {
        return None;
    }

    Some(rounded as u64)
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|value| parse_bool(&value))
        .unwrap_or(default)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim_end().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{NANOTONS_PER_TON, parse_bool, parse_nanotons, parse_number};

    #[test]
    fn parses_numbers_with_underscores() {
        assert_eq!(parse_number::<u64>("500_000_000"), Some(500_000_000));
        assert_eq!(parse_number::<u32>("1_000"), Some(1_000));
        assert_eq!(parse_number::<u16>("3001"), Some(3001));
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert_eq!(parse_number::<u64>("500 TON"), None);
    }

    #[test]
    fn parses_nanotons() {
        assert_eq!(parse_nanotons("500_000_000"), Some(500_000_000));
        assert_eq!(parse_nanotons("1TON"), Some(NANOTONS_PER_TON));
        assert_eq!(parse_nanotons("1ton"), Some(NANOTONS_PER_TON));
        assert_eq!(parse_nanotons("0.5TON"), Some(500_000_000));
        assert_eq!(parse_nanotons(".25ton"), Some(250_000_000));
        assert_eq!(parse_nanotons("1e-9TON"), Some(1));
        assert_eq!(parse_nanotons("1.000000001TON"), Some(1_000_000_001));
    }

    #[test]
    fn rejects_invalid_nanoton_values() {
        assert_eq!(parse_nanotons(""), None);
        assert_eq!(parse_nanotons("TON"), None);
        assert_eq!(parse_nanotons("1.0000000001TON"), None);
        assert_eq!(parse_nanotons("oneTON"), None);
    }

    #[test]
    fn parses_bool_values() {
        for value in ["true", "TRUE", "yes", "on", "on "] {
            assert_eq!(parse_bool(value), Some(true));
        }

        for value in ["false", "FALSE", "no", "off", "off "] {
            assert_eq!(parse_bool(value), Some(false));
        }
    }

    #[test]
    fn rejects_invalid_bool_values() {
        assert_eq!(parse_bool(""), None);
        assert_eq!(parse_bool("1"), None);
        assert_eq!(parse_bool("0"), None);
        assert_eq!(parse_bool(" on"), None);
        assert_eq!(parse_bool(" off"), None);
        assert_eq!(parse_bool("maybe"), None);
    }
}
