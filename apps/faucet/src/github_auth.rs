use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use faucet_config::{GitHubAuthConfig, GitHubTierConfig};
use faucet_valkey::ValkeyStore;
use rand::RngCore;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";
const GITHUB_API_VERSION: &str = "2022-11-28";
const OAUTH_STATE_KEY_PREFIX: &str = "faucet:github:oauth-state";
const GRANT_KEY_PREFIX: &str = "faucet:github:grant";
const SESSION_KEY_PREFIX: &str = "faucet:github:session";

#[derive(Clone)]
pub(crate) struct GitHubAuth {
    config: GitHubAuthConfig,
    valkey: ValkeyStore,
    client: Client,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FaucetTier {
    Guest,
    Verified,
    Established,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitHubIdentity {
    pub(crate) github_user_id: u64,
    pub(crate) login: String,
    pub(crate) tier: FaucetTier,
    pub(crate) max_requests: u32,
    pub(crate) account_age_days: u64,
    pub(crate) public_repos: u32,
    pub(crate) followers: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OAuthState {
    device_uid: String,
    pkce_verifier: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BrowserGrant {
    device_uid: String,
    identity: GitHubIdentity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BrowserSession {
    device_uid: String,
    identity: GitHubIdentity,
    expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GitHubProfile {
    id: u64,
    login: String,
    created_at: DateTime<Utc>,
    public_repos: u32,
    followers: u32,
}

pub(crate) struct SessionExchange {
    pub(crate) token: String,
    pub(crate) identity: GitHubIdentity,
    pub(crate) expires_at: i64,
}

#[derive(Debug)]
pub(crate) enum AuthError {
    Disabled,
    InvalidAuthorization,
    InvalidSession,
    Internal(anyhow::Error),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("GitHub authentication is disabled"),
            Self::InvalidAuthorization => {
                formatter.write_str("Invalid or expired GitHub authorization")
            }
            Self::InvalidSession => {
                formatter.write_str("GitHub session is not valid for this browser")
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Internal(error) => Some(error.as_ref()),
            Self::Disabled | Self::InvalidAuthorization | Self::InvalidSession => None,
        }
    }
}

impl From<anyhow::Error> for AuthError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
}

impl GitHubAuth {
    pub(crate) fn new(config: GitHubAuthConfig, valkey: ValkeyStore) -> anyhow::Result<Self> {
        validate_redirect_url("GITHUB_CALLBACK_URL", &config.callback_url)?;
        validate_redirect_url("GITHUB_FRONTEND_URL", &config.frontend_url)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .context("Failed to create GitHub HTTP client")?;

        Ok(Self {
            config,
            valkey,
            client,
        })
    }

    pub(crate) fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub(crate) fn verified_max_requests(&self) -> u32 {
        self.config.verified.max_requests
    }

    pub(crate) fn established_max_requests(&self) -> u32 {
        self.config.established.max_requests
    }

    pub(crate) async fn authorization_url(&self, device_uid: &str) -> Result<String, AuthError> {
        self.ensure_enabled()?;

        let state = random_token();
        let pkce_verifier = random_token();
        let pkce_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce_verifier.as_bytes()));
        let oauth_state = OAuthState {
            device_uid: device_uid.to_string(),
            pkce_verifier,
        };
        self.store_json(
            &ephemeral_key(OAUTH_STATE_KEY_PREFIX, &state),
            &oauth_state,
            self.config.state_ttl_seconds,
        )
        .await?;

        let mut url = Url::parse(GITHUB_AUTHORIZE_URL).expect("GitHub authorize URL must be valid");
        url.query_pairs_mut()
            .append_pair("client_id", self.client_id()?)
            .append_pair("redirect_uri", &self.config.callback_url)
            .append_pair("state", &state)
            .append_pair("code_challenge", &pkce_challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.to_string())
    }

    pub(crate) async fn finish_authorization(
        &self,
        code: &str,
        state: &str,
    ) -> Result<String, AuthError> {
        self.ensure_enabled()?;
        let oauth_state: OAuthState = self
            .take_json(
                &ephemeral_key(OAUTH_STATE_KEY_PREFIX, state),
                AuthError::InvalidAuthorization,
            )
            .await?;
        let access_token = self
            .exchange_github_code(code, &oauth_state.pkce_verifier)
            .await?;
        let profile = self.load_github_profile(&access_token).await?;
        let identity = self.identity_from_profile(profile);
        let grant = random_token();
        self.store_json(
            &ephemeral_key(GRANT_KEY_PREFIX, &grant),
            &BrowserGrant {
                device_uid: oauth_state.device_uid,
                identity,
            },
            self.config.grant_ttl_seconds,
        )
        .await?;
        Ok(grant)
    }

    pub(crate) async fn exchange_grant(
        &self,
        grant: &str,
        device_uid: &str,
    ) -> Result<SessionExchange, AuthError> {
        self.ensure_enabled()?;
        let grant: BrowserGrant = self
            .take_json(
                &ephemeral_key(GRANT_KEY_PREFIX, grant),
                AuthError::InvalidAuthorization,
            )
            .await?;
        if grant.device_uid != device_uid {
            return Err(AuthError::InvalidAuthorization);
        }

        let token = random_token();
        let expires_at = Utc::now().timestamp() + self.config.session_ttl_seconds as i64;
        let session = BrowserSession {
            device_uid: grant.device_uid,
            identity: grant.identity.clone(),
            expires_at,
        };
        self.store_json(
            &ephemeral_key(SESSION_KEY_PREFIX, &token),
            &session,
            self.config.session_ttl_seconds,
        )
        .await?;

        Ok(SessionExchange {
            token,
            identity: grant.identity,
            expires_at,
        })
    }

    pub(crate) async fn session(
        &self,
        token: &str,
        device_uid: &str,
    ) -> Result<GitHubIdentity, AuthError> {
        self.ensure_enabled()?;
        let value = self
            .valkey
            .get_ephemeral(&ephemeral_key(SESSION_KEY_PREFIX, token))
            .await
            .context("Failed to load GitHub session")?
            .ok_or(AuthError::InvalidSession)?;
        let session: BrowserSession =
            serde_json::from_str(&value).context("Failed to decode GitHub session")?;

        if session.device_uid != device_uid || session.expires_at <= Utc::now().timestamp() {
            return Err(AuthError::InvalidSession);
        }
        Ok(session.identity)
    }

    pub(crate) async fn delete_session(&self, token: &str) -> Result<(), AuthError> {
        self.ensure_enabled()?;
        self.valkey
            .delete_ephemeral(&ephemeral_key(SESSION_KEY_PREFIX, token))
            .await
            .context("Failed to delete GitHub session")?;
        Ok(())
    }

    pub(crate) fn frontend_redirect(
        &self,
        parameter: &str,
        value: &str,
    ) -> Result<String, AuthError> {
        frontend_redirect_url(&self.config.frontend_url, parameter, value).map_err(Into::into)
    }

    fn ensure_enabled(&self) -> Result<(), AuthError> {
        if self.config.enabled {
            Ok(())
        } else {
            Err(AuthError::Disabled)
        }
    }

    fn client_id(&self) -> Result<&str, AuthError> {
        self.config
            .client_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("GitHub client ID is missing").into())
    }

    fn client_secret(&self) -> Result<&str, AuthError> {
        self.config
            .client_secret
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("GitHub client secret is missing").into())
    }

    async fn exchange_github_code(
        &self,
        code: &str,
        pkce_verifier: &str,
    ) -> Result<String, AuthError> {
        let response = self
            .client
            .post(GITHUB_TOKEN_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id()?),
                ("client_secret", self.client_secret()?),
                ("code", code),
                ("redirect_uri", &self.config.callback_url),
                ("code_verifier", pkce_verifier),
            ])
            .send()
            .await
            .context("Failed to exchange GitHub authorization code")?
            .error_for_status()
            .context("GitHub rejected the authorization code")?
            .json::<GitHubTokenResponse>()
            .await
            .context("Failed to decode GitHub access token")?;
        if response.access_token.is_empty() {
            return Err(anyhow::anyhow!("GitHub returned an empty access token").into());
        }
        Ok(response.access_token)
    }

    async fn load_github_profile(&self, access_token: &str) -> Result<GitHubProfile, AuthError> {
        self.client
            .get(GITHUB_USER_URL)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, "acton-faucet")
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .send()
            .await
            .context("Failed to load GitHub profile")?
            .error_for_status()
            .context("GitHub rejected the profile request")?
            .json()
            .await
            .context("Failed to decode GitHub profile")
            .map_err(Into::into)
    }

    fn identity_from_profile(&self, profile: GitHubProfile) -> GitHubIdentity {
        let account_age_days = Utc::now()
            .signed_duration_since(profile.created_at)
            .num_days()
            .max(0) as u64;
        let tier = evaluate_tier(
            account_age_days,
            profile.public_repos,
            profile.followers,
            &self.config.verified,
            &self.config.established,
        );
        let max_requests = match tier {
            FaucetTier::Guest => 0,
            FaucetTier::Verified => self.config.verified.max_requests,
            FaucetTier::Established => self.config.established.max_requests,
        };

        GitHubIdentity {
            github_user_id: profile.id,
            login: profile.login,
            tier,
            max_requests,
            account_age_days,
            public_repos: profile.public_repos,
            followers: profile.followers,
        }
    }

    async fn store_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: u64,
    ) -> Result<(), AuthError> {
        let value = serde_json::to_string(value).context("Failed to encode GitHub auth state")?;
        self.valkey
            .store_ephemeral(key, &value, ttl_seconds)
            .await
            .context("Failed to store GitHub auth state")
            .map_err(Into::into)
    }

    async fn take_json<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
        missing_error: AuthError,
    ) -> Result<T, AuthError> {
        let value = self
            .valkey
            .take_ephemeral(key)
            .await
            .context("Failed to take GitHub auth state")?
            .ok_or(missing_error)?;
        serde_json::from_str(&value)
            .context("Failed to decode GitHub auth state")
            .map_err(Into::into)
    }
}

fn evaluate_tier(
    account_age_days: u64,
    public_repos: u32,
    followers: u32,
    verified: &GitHubTierConfig,
    established: &GitHubTierConfig,
) -> FaucetTier {
    if meets_tier(account_age_days, public_repos, followers, established) {
        FaucetTier::Established
    } else if meets_tier(account_age_days, public_repos, followers, verified) {
        FaucetTier::Verified
    } else {
        FaucetTier::Guest
    }
}

fn meets_tier(
    account_age_days: u64,
    public_repos: u32,
    followers: u32,
    tier: &GitHubTierConfig,
) -> bool {
    account_age_days >= tier.min_account_age_days
        && public_repos >= tier.min_public_repos
        && followers >= tier.min_followers
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn ephemeral_key(prefix: &str, token: &str) -> String {
    format!("{prefix}:{}", hex::encode(Sha256::digest(token.as_bytes())))
}

fn validate_redirect_url(name: &str, value: &str) -> anyhow::Result<()> {
    let url = Url::parse(value).with_context(|| format!("{name} must be a valid URL"))?;
    let local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    anyhow::ensure!(
        url.scheme() == "https" || local_http,
        "{name} must use HTTPS outside localhost"
    );
    Ok(())
}

fn frontend_redirect_url(base_url: &str, parameter: &str, value: &str) -> anyhow::Result<String> {
    let mut url = Url::parse(base_url).context("GITHUB_FRONTEND_URL must be a valid URL")?;
    url.set_fragment(Some(&format!("{parameter}={value}")));
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use faucet_config::GitHubTierConfig;

    use super::{
        FaucetTier, ephemeral_key, evaluate_tier, frontend_redirect_url, validate_redirect_url,
    };

    fn tier(max_requests: u32, age: u64, repos: u32, followers: u32) -> GitHubTierConfig {
        GitHubTierConfig {
            max_requests,
            min_account_age_days: age,
            min_public_repos: repos,
            min_followers: followers,
        }
    }

    #[test]
    fn evaluates_github_tiers_from_public_profile_signals() {
        let verified = tier(4, 90, 2, 0);
        let established = tier(8, 365, 5, 5);

        assert_eq!(
            evaluate_tier(30, 20, 20, &verified, &established),
            FaucetTier::Guest
        );
        assert_eq!(
            evaluate_tier(100, 2, 0, &verified, &established),
            FaucetTier::Verified
        );
        assert_eq!(
            evaluate_tier(400, 5, 5, &verified, &established),
            FaucetTier::Established
        );
    }

    #[test]
    fn hashes_browser_tokens_before_using_them_as_keys() {
        let key = ephemeral_key("faucet:github:session", "secret-token");

        assert_eq!(
            key,
            "faucet:github:session:930bbdc51b6aed5c2a5678fd6e28dee7a05e8a4b643cfc0b4427c3efb86c0d94"
        );
        assert!(!key.contains("secret-token"));
    }

    #[test]
    fn requires_https_for_non_local_redirects() {
        assert!(validate_redirect_url("URL", "https://actonscan.com/faucet").is_ok());
        assert!(validate_redirect_url("URL", "http://localhost:3007/faucet").is_ok());
        assert!(validate_redirect_url("URL", "http://actonscan.com/faucet").is_err());
        assert!(validate_redirect_url("URL", "not-a-url").is_err());
    }

    #[test]
    fn keeps_one_time_grant_out_of_http_query_and_referer() {
        let redirect =
            frontend_redirect_url("https://actonscan.com/faucet", "github_grant", "secret")
                .unwrap();

        assert_eq!(redirect, "https://actonscan.com/faucet#github_grant=secret");
    }
}
