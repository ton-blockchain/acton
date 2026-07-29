use anyhow::Context;
use http::HeaderName;
use ipnet::IpNet;
use std::{net::IpAddr, str::FromStr};

const NANOGRAMS_PER_GRAM: u64 = 1_000_000_000;

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
    pub github_auth: GitHubAuthConfig,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub proxy: ProxyConfig,
}

#[derive(Clone, Debug)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub header: String,
    pub ips: Vec<IpNet>,
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
    pub subnet_amount_window: SubnetAmountWindowCheckConfig,
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
pub struct SubnetAmountWindowCheckConfig {
    pub enabled: bool,
    pub max_amount: u64,
    pub ipv4_prefix_length: u32,
    pub window_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct SuccessfulClaimWindowCheckConfig {
    pub enabled: bool,
    pub max_requests: u32,
    pub window_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct GitHubAuthConfig {
    pub enabled: bool,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub callback_url: String,
    pub frontend_url: String,
    pub oauth_max_pending_states: u64,
    pub state_ttl_seconds: u64,
    pub grant_ttl_seconds: u64,
    pub session_ttl_seconds: u64,
    pub verified: GitHubTierConfig,
    pub established: GitHubTierConfig,
}

#[derive(Clone, Debug)]
pub struct GitHubTierConfig {
    pub max_requests: u32,
    pub min_account_age_days: u64,
    pub min_public_repos: u32,
    pub min_followers: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let github_auth_enabled = parse_env_bool("GITHUB_AUTH_ENABLED", false);
        let config = Config {
            database: DatabaseConfig {
                url: std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "sqlite:./db.sqlite".to_string()),
            },
            server: ServerConfig {
                host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: parse_env_number("PORT", 3001),
                proxy: ProxyConfig {
                    enabled: parse_env_bool("SERVER_TRUST_PROXY", false),
                    header: std::env::var("SERVER_TRUST_PROXY_HEADER")
                        .unwrap_or_else(|_| "X-Real-IP".to_string()),
                    ips: parse_ip_list(
                        &std::env::var("SERVER_TRUST_PROXY_IPS").unwrap_or_default(),
                    )
                    .context("Invalid SERVER_TRUST_PROXY_IPS")?,
                },
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
                amount: parse_env_nanograms(
                    "FAUCET_AMOUNT_NANOGRAMS",
                    "FAUCET_AMOUNT_NANOTONS",
                    1_000_000,
                ),
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
                    max_wallet_balance: parse_env_nanograms(
                        "ANTIFRAUD_WALLET_BALANCE_MAX_NANOGRAMS",
                        "ANTIFRAUD_WALLET_BALANCE_MAX_NANOTONS",
                        25_000_000_000,
                    ),
                },
                sent_amount_window: SentAmountWindowCheckConfig {
                    enabled: parse_env_bool("ANTIFRAUD_SENT_AMOUNT_WINDOW_ENABLED", true),
                    max_amount: parse_env_nanograms(
                        "ANTIFRAUD_SENT_AMOUNT_WINDOW_MAX_NANOGRAMS",
                        "ANTIFRAUD_SENT_AMOUNT_WINDOW_MAX_NANOTONS",
                        10_000_000_000,
                    ),
                    window_seconds: parse_env_number("ANTIFRAUD_SENT_AMOUNT_WINDOW_SECONDS", 60),
                },
                subnet_amount_window: SubnetAmountWindowCheckConfig {
                    enabled: parse_env_bool("ANTIFRAUD_SUBNET_AMOUNT_WINDOW_ENABLED", true),
                    max_amount: parse_env_nanograms(
                        "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_MAX_NANOGRAMS",
                        "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_MAX_NANOTONS",
                        32_000_000_000,
                    ),
                    ipv4_prefix_length: parse_env_number(
                        "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_IPV4_PREFIX_LENGTH",
                        24,
                    ),
                    window_seconds: parse_env_number(
                        "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_SECONDS",
                        86_400,
                    ),
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
            github_auth: GitHubAuthConfig {
                enabled: github_auth_enabled,
                client_id: optional_env("GITHUB_CLIENT_ID"),
                client_secret: optional_env("GITHUB_CLIENT_SECRET"),
                callback_url: std::env::var("GITHUB_CALLBACK_URL").unwrap_or_else(|_| {
                    "https://faucet.acton.monster/auth/github/callback".to_string()
                }),
                frontend_url: std::env::var("GITHUB_FRONTEND_URL")
                    .unwrap_or_else(|_| "https://actonscan.com/faucet".to_string()),
                oauth_max_pending_states: parse_env_number("GITHUB_OAUTH_MAX_PENDING_STATES", 256),
                state_ttl_seconds: parse_env_number("GITHUB_STATE_TTL_SECONDS", 600),
                grant_ttl_seconds: parse_env_number("GITHUB_GRANT_TTL_SECONDS", 120),
                session_ttl_seconds: parse_env_number("GITHUB_SESSION_TTL_SECONDS", 604_800),
                verified: GitHubTierConfig {
                    max_requests: parse_env_number("GITHUB_VERIFIED_MAX_REQUESTS", 4),
                    min_account_age_days: parse_env_number(
                        "GITHUB_VERIFIED_MIN_ACCOUNT_AGE_DAYS",
                        90,
                    ),
                    min_public_repos: parse_env_number("GITHUB_VERIFIED_MIN_PUBLIC_REPOS", 2),
                    min_followers: parse_env_number("GITHUB_VERIFIED_MIN_FOLLOWERS", 0),
                },
                established: GitHubTierConfig {
                    max_requests: parse_env_number("GITHUB_ESTABLISHED_MAX_REQUESTS", 8),
                    min_account_age_days: parse_env_number(
                        "GITHUB_ESTABLISHED_MIN_ACCOUNT_AGE_DAYS",
                        365,
                    ),
                    min_public_repos: parse_env_number("GITHUB_ESTABLISHED_MIN_PUBLIC_REPOS", 5),
                    min_followers: parse_env_number("GITHUB_ESTABLISHED_MIN_FOLLOWERS", 5),
                },
            },
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.server.proxy.header.parse::<HeaderName>().is_ok(),
            "SERVER_TRUST_PROXY_HEADER must be a valid HTTP header name"
        );
        anyhow::ensure!(
            self.pow.max_challenges > 0,
            "POW_MAX_CHALLENGES must be positive"
        );
        anyhow::ensure!(
            self.pow.challenge_ttl_seconds > 0,
            "POW_CHALLENGE_TTL_SECONDS must be positive"
        );
        anyhow::ensure!(
            self.rate_limit.claim.window_seconds > 0,
            "RATE_LIMIT_CLAIM_WINDOW_SECONDS must be positive"
        );
        if self.antifraud.enabled && self.antifraud.successful_claim_window.enabled {
            anyhow::ensure!(
                self.antifraud.successful_claim_window.window_seconds > 0,
                "ANTIFRAUD_SUCCESSFUL_CLAIM_WINDOW_SECONDS must be positive"
            );
        }
        if self.antifraud.enabled && self.antifraud.subnet_amount_window.enabled {
            anyhow::ensure!(
                self.antifraud.subnet_amount_window.ipv4_prefix_length <= 32,
                "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_IPV4_PREFIX_LENGTH must be between 0 and 32"
            );
            anyhow::ensure!(
                self.antifraud.subnet_amount_window.window_seconds > 0,
                "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_SECONDS must be positive"
            );
        }

        if !self.github_auth.enabled {
            return Ok(());
        }

        anyhow::ensure!(
            self.antifraud.enabled && self.antifraud.successful_claim_window.enabled,
            "GitHub authentication requires the successful claim window"
        );
        anyhow::ensure!(
            self.github_auth.client_id.is_some(),
            "GITHUB_CLIENT_ID must be set when GitHub authentication is enabled"
        );
        anyhow::ensure!(
            self.github_auth.client_secret.is_some(),
            "GITHUB_CLIENT_SECRET must be set when GitHub authentication is enabled"
        );
        anyhow::ensure!(
            self.github_auth.oauth_max_pending_states > 0,
            "GITHUB_OAUTH_MAX_PENDING_STATES must be positive"
        );
        anyhow::ensure!(
            self.github_auth.state_ttl_seconds > 0
                && self.github_auth.grant_ttl_seconds > 0
                && self.github_auth.session_ttl_seconds > 0,
            "GitHub authentication TTLs must be positive"
        );
        anyhow::ensure!(
            self.github_auth.verified.max_requests
                >= self.antifraud.successful_claim_window.max_requests,
            "GITHUB_VERIFIED_MAX_REQUESTS must not be below the guest limit"
        );
        anyhow::ensure!(
            self.github_auth.established.max_requests >= self.github_auth.verified.max_requests,
            "GITHUB_ESTABLISHED_MAX_REQUESTS must not be below the verified limit"
        );
        anyhow::ensure!(
            self.github_auth.established.min_account_age_days
                >= self.github_auth.verified.min_account_age_days,
            "GITHUB_ESTABLISHED_MIN_ACCOUNT_AGE_DAYS must not be below the verified threshold"
        );
        anyhow::ensure!(
            self.github_auth.established.min_public_repos
                >= self.github_auth.verified.min_public_repos,
            "GITHUB_ESTABLISHED_MIN_PUBLIC_REPOS must not be below the verified threshold"
        );
        anyhow::ensure!(
            self.github_auth.established.min_followers >= self.github_auth.verified.min_followers,
            "GITHUB_ESTABLISHED_MIN_FOLLOWERS must not be below the verified threshold"
        );
        let claim_capacity = u128::from(self.rate_limit.claim.max_requests)
            * u128::from(self.antifraud.successful_claim_window.window_seconds);
        let established_capacity = u128::from(self.github_auth.established.max_requests)
            * u128::from(self.rate_limit.claim.window_seconds);
        anyhow::ensure!(
            claim_capacity >= established_capacity,
            "claim rate limit capacity must not be below the established GitHub limit"
        );

        Ok(())
    }
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn parse_env_nanograms(name: &str, legacy_name: &str, default: u64) -> u64 {
    std::env::var(name)
        .or_else(|_| std::env::var(legacy_name))
        .ok()
        .and_then(|value| parse_nanograms(&value))
        .unwrap_or(default)
}

fn parse_nanograms(value: &str) -> Option<u64> {
    let value = value.trim();
    let normalized = value.to_ascii_lowercase();

    if let Some(gram_amount) = normalized
        .strip_suffix("gram")
        .or_else(|| normalized.strip_suffix("ton"))
    {
        parse_gram_amount(gram_amount)
    } else {
        parse_number(value)
    }
}

fn parse_gram_amount(value: &str) -> Option<u64> {
    let grams = value.trim().replace('_', "").parse::<f64>().ok()?;
    if !grams.is_finite() || grams < 0.0 {
        return None;
    }

    let nanograms = grams * NANOGRAMS_PER_GRAM as f64;
    let rounded = nanograms.round();
    if (nanograms - rounded).abs() > 0.000_001 || rounded > u64::MAX as f64 {
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

fn parse_ip_list(value: &str) -> anyhow::Result<Vec<IpNet>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<IpNet>()
                .or_else(|_| value.parse::<IpAddr>().map(IpNet::from))
                .with_context(|| format!("Invalid IP address or network `{value}`"))
        })
        .collect()
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
    use super::{
        AntifraudConfig, ClaimRateLimitConfig, Config, DatabaseConfig, DefaultRateLimitConfig,
        FaucetConfig, GitHubAuthConfig, GitHubTierConfig, NANOGRAMS_PER_GRAM, PowClientConfig,
        PowConfig, ProxyConfig, RateLimitConfig, SentAmountWindowCheckConfig, ServerConfig,
        SubnetAmountWindowCheckConfig, SuccessfulClaimWindowCheckConfig, ToncenterConfig,
        ValkeyConfig, WalletBalanceCheckConfig, WorkerConfig, parse_bool, parse_ip_list,
        parse_nanograms, parse_number,
    };
    use ipnet::IpNet;

    fn valid_config() -> Config {
        Config {
            database: DatabaseConfig {
                url: "sqlite::memory:".to_string(),
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3001,
                proxy: ProxyConfig {
                    enabled: false,
                    header: "X-Real-IP".to_string(),
                    ips: Vec::new(),
                },
            },
            rate_limit: RateLimitConfig {
                default: DefaultRateLimitConfig {
                    window_seconds: 1,
                    max_requests: 5,
                },
                claim: ClaimRateLimitConfig {
                    window_seconds: 3_600,
                    max_requests: 32,
                },
            },
            toncenter: ToncenterConfig {
                api_key: None,
                url: "https://testnet.toncenter.com".to_string(),
                timeout_seconds: 10,
                connect_timeout_seconds: 5,
                max_retries: 3,
                retry_base_delay_ms: 500,
            },
            worker: WorkerConfig {
                max_retries: 2,
                retry_base_delay_ms: 1_000,
            },
            faucet: FaucetConfig {
                mnemonic: "test mnemonic".to_string(),
                amount: 1_000_000,
                message: "Testnet faucet".to_string(),
            },
            pow: PowConfig {
                enabled: true,
                difficulty: 21,
                challenge_ttl_seconds: 120,
                max_challenges: 10_000,
                client: PowClientConfig {
                    max_solve_ttl_seconds: 300,
                    max_nonce_attempts: 1_000_000_000,
                },
            },
            valkey: ValkeyConfig {
                uri: "redis://127.0.0.1:6379".to_string(),
            },
            antifraud: AntifraudConfig {
                enabled: true,
                wallet_balance: WalletBalanceCheckConfig {
                    enabled: true,
                    max_wallet_balance: 25_000_000_000,
                },
                sent_amount_window: SentAmountWindowCheckConfig {
                    enabled: true,
                    max_amount: 10_000_000_000,
                    window_seconds: 60,
                },
                subnet_amount_window: SubnetAmountWindowCheckConfig {
                    enabled: true,
                    max_amount: 10_000_000_000,
                    ipv4_prefix_length: 24,
                    window_seconds: 86_400,
                },
                successful_claim_window: SuccessfulClaimWindowCheckConfig {
                    enabled: true,
                    max_requests: 2,
                    window_seconds: 3_600,
                },
            },
            github_auth: GitHubAuthConfig {
                enabled: true,
                client_id: Some("client-id".to_string()),
                client_secret: Some("client-secret".to_string()),
                callback_url: "https://faucet.example/auth/github/callback".to_string(),
                frontend_url: "https://example.com/faucet".to_string(),
                oauth_max_pending_states: 256,
                state_ttl_seconds: 600,
                grant_ttl_seconds: 120,
                session_ttl_seconds: 604_800,
                verified: GitHubTierConfig {
                    max_requests: 4,
                    min_account_age_days: 90,
                    min_public_repos: 2,
                    min_followers: 0,
                },
                established: GitHubTierConfig {
                    max_requests: 8,
                    min_account_age_days: 365,
                    min_public_repos: 5,
                    min_followers: 5,
                },
            },
        }
    }

    fn validation_error(config: &Config) -> String {
        config.validate().unwrap_err().to_string()
    }

    #[test]
    fn parses_numbers_with_underscores() {
        assert_eq!(parse_number::<u64>("500_000_000"), Some(500_000_000));
        assert_eq!(parse_number::<u32>("1_000"), Some(1_000));
        assert_eq!(parse_number::<u16>("3001"), Some(3001));
    }

    #[test]
    fn rejects_invalid_numbers() {
        assert_eq!(parse_number::<u64>("500 GRAM"), None);
    }

    #[test]
    fn parses_nanograms() {
        assert_eq!(parse_nanograms("500_000_000"), Some(500_000_000));
        assert_eq!(parse_nanograms("1GRAM"), Some(NANOGRAMS_PER_GRAM));
        assert_eq!(parse_nanograms("10GRAM"), Some(10 * NANOGRAMS_PER_GRAM));
        assert_eq!(parse_nanograms("1gram"), Some(NANOGRAMS_PER_GRAM));
        assert_eq!(parse_nanograms("0.5GRAM"), Some(500_000_000));
        assert_eq!(parse_nanograms(".25gram"), Some(250_000_000));
        assert_eq!(parse_nanograms("1e-9GRAM"), Some(1));
        assert_eq!(parse_nanograms("1.000000001GRAM"), Some(1_000_000_001));
    }

    #[test]
    fn parses_legacy_ton_suffix() {
        assert_eq!(parse_nanograms("1TON"), Some(NANOGRAMS_PER_GRAM));
        assert_eq!(parse_nanograms("10TON"), Some(10 * NANOGRAMS_PER_GRAM));
        assert_eq!(parse_nanograms("0.5ton"), Some(500_000_000));
    }

    #[test]
    fn rejects_invalid_nanogram_values() {
        assert_eq!(parse_nanograms(""), None);
        assert_eq!(parse_nanograms("GRAM"), None);
        assert_eq!(parse_nanograms("1.0000000001GRAM"), None);
        assert_eq!(parse_nanograms("oneGRAM"), None);
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

    #[test]
    fn parses_proxy_header_trust_flag() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
    }

    #[test]
    fn parses_trusted_proxy_ip_list() {
        assert_eq!(
            parse_ip_list("192.168.100.1, 192.168.200.0/24, ::1, fd00::/64").unwrap(),
            vec![
                "192.168.100.1/32".parse::<IpNet>().unwrap(),
                "192.168.200.0/24".parse().unwrap(),
                "::1/128".parse().unwrap(),
                "fd00::/64".parse().unwrap(),
            ]
        );
        assert!(parse_ip_list("192.168.100.0/33").is_err());
        assert!(parse_ip_list("not-an-ip").is_err());
    }

    #[test]
    fn rejects_invalid_proxy_header_name() {
        let mut config = valid_config();
        config.server.proxy.header = "invalid header".to_string();

        assert_eq!(
            validation_error(&config),
            "SERVER_TRUST_PROXY_HEADER must be a valid HTTP header name"
        );
    }

    #[test]
    fn rejects_zero_pow_challenge_ttl_when_github_auth_is_disabled() {
        let mut config = valid_config();
        config.github_auth.enabled = false;
        config.pow.challenge_ttl_seconds = 0;

        assert_eq!(
            validation_error(&config),
            "POW_CHALLENGE_TTL_SECONDS must be positive"
        );
    }

    #[test]
    fn validates_active_claim_windows_when_github_auth_is_disabled() {
        let mut config = valid_config();
        config.github_auth.enabled = false;
        config.rate_limit.claim.window_seconds = 0;
        assert_eq!(
            validation_error(&config),
            "RATE_LIMIT_CLAIM_WINDOW_SECONDS must be positive"
        );

        let mut config = valid_config();
        config.github_auth.enabled = false;
        config.antifraud.successful_claim_window.window_seconds = 0;
        assert_eq!(
            validation_error(&config),
            "ANTIFRAUD_SUCCESSFUL_CLAIM_WINDOW_SECONDS must be positive"
        );

        let mut config = valid_config();
        config.github_auth.enabled = false;
        config.antifraud.subnet_amount_window.window_seconds = 0;
        assert_eq!(
            validation_error(&config),
            "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_SECONDS must be positive"
        );
    }

    #[test]
    fn validates_ipv4_subnet_prefix_length() {
        for prefix_length in 0..=32 {
            let mut config = valid_config();
            config.antifraud.subnet_amount_window.ipv4_prefix_length = prefix_length;
            config.validate().unwrap();
        }

        let mut config = valid_config();
        config.antifraud.subnet_amount_window.ipv4_prefix_length = 33;
        assert_eq!(
            validation_error(&config),
            "ANTIFRAUD_SUBNET_AMOUNT_WINDOW_IPV4_PREFIX_LENGTH must be between 0 and 32"
        );
    }

    #[test]
    fn allows_unused_successful_claim_window_values_when_antifraud_is_disabled() {
        let mut config = valid_config();
        config.github_auth.enabled = false;
        config.antifraud.enabled = false;
        config.antifraud.successful_claim_window.window_seconds = 0;
        config.antifraud.subnet_amount_window.ipv4_prefix_length = 33;
        config.antifraud.subnet_amount_window.window_seconds = 0;

        config.validate().unwrap();
    }

    #[test]
    fn requires_successful_claim_window_for_github_tiers() {
        let mut config = valid_config();
        config.antifraud.successful_claim_window.enabled = false;

        assert_eq!(
            validation_error(&config),
            "GitHub authentication requires the successful claim window"
        );
    }

    #[test]
    fn rejects_zero_oauth_pending_state_cap() {
        let mut config = valid_config();
        config.github_auth.oauth_max_pending_states = 0;

        assert_eq!(
            validation_error(&config),
            "GITHUB_OAUTH_MAX_PENDING_STATES must be positive"
        );
    }

    #[test]
    fn rejects_established_tier_thresholds_weaker_than_verified() {
        type MakeInvalid = fn(&mut Config);

        let cases: &[(MakeInvalid, &str)] = &[
            (
                |config| config.github_auth.established.min_account_age_days = 89,
                "GITHUB_ESTABLISHED_MIN_ACCOUNT_AGE_DAYS must not be below the verified threshold",
            ),
            (
                |config| config.github_auth.established.min_public_repos = 1,
                "GITHUB_ESTABLISHED_MIN_PUBLIC_REPOS must not be below the verified threshold",
            ),
            (
                |config| config.github_auth.verified.min_followers = 6,
                "GITHUB_ESTABLISHED_MIN_FOLLOWERS must not be below the verified threshold",
            ),
        ];

        for (make_invalid, expected) in cases {
            let mut config = valid_config();
            make_invalid(&mut config);
            assert_eq!(validation_error(&config), *expected);
        }
    }

    #[test]
    fn accepts_claim_rate_limit_with_sufficient_capacity() {
        let production_config = valid_config();
        production_config.validate().unwrap();

        let mut equivalent_short_window = valid_config();
        equivalent_short_window.rate_limit.claim.window_seconds = 1_800;
        equivalent_short_window.rate_limit.claim.max_requests = 4;
        equivalent_short_window.validate().unwrap();
    }

    #[test]
    fn rejects_claim_rate_limit_with_insufficient_capacity() {
        let mut config = valid_config();
        config.rate_limit.claim.window_seconds = 86_400;
        config.rate_limit.claim.max_requests = 8;

        assert_eq!(
            validation_error(&config),
            "claim rate limit capacity must not be below the established GitHub limit"
        );
    }
}
