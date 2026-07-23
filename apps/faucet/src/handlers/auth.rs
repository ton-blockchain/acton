use axum::{
    Extension, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::Redirect,
};
use faucet_backend::middlewares::{ClientContext, is_allowed_device_uid};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::{
    AppState,
    github_auth::{AuthError, FaucetTier, GitHubIdentity},
};

#[derive(Deserialize)]
pub(super) struct GitHubStartQuery {
    device_uid: String,
}

#[derive(Deserialize)]
pub(super) struct GitHubCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct GrantExchangeRequest {
    grant: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AuthStatusResponse {
    enabled: bool,
    guest_max_requests: u32,
    verified_max_requests: u32,
    established_max_requests: u32,
    window_seconds: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionResponse {
    authenticated: bool,
    github_user_id: u64,
    login: String,
    tier: FaucetTier,
    max_requests: u32,
    account_age_days: u64,
    public_repos: u32,
    followers: u32,
    expires_at: Option<i64>,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct ErrorResponse {
    error: &'static str,
}

pub(super) type AuthHttpError = (StatusCode, Json<ErrorResponse>);

pub(super) async fn status(State(state): State<AppState>) -> Json<AuthStatusResponse> {
    let window = &state.config.antifraud.successful_claim_window;
    Json(AuthStatusResponse {
        enabled: state.github_auth.enabled(),
        guest_max_requests: window.max_requests,
        verified_max_requests: state.github_auth.verified_max_requests(),
        established_max_requests: state.github_auth.established_max_requests(),
        window_seconds: window.window_seconds,
    })
}

pub(super) async fn github_start(
    State(state): State<AppState>,
    Query(query): Query<GitHubStartQuery>,
) -> Result<Redirect, AuthHttpError> {
    if !is_allowed_device_uid(&query.device_uid) {
        return Err(response_error(
            StatusCode::BAD_REQUEST,
            "Invalid device identifier",
        ));
    }

    let url = state
        .github_auth
        .authorization_url(&query.device_uid)
        .await
        .map_err(auth_error)?;
    Ok(Redirect::temporary(&url))
}

pub(super) async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<GitHubCallbackQuery>,
) -> Result<Redirect, AuthHttpError> {
    if let Some(github_error) = query.error {
        warn!(github_error, "GitHub authorization was not completed");
        return frontend_error_redirect(&state, "authorization_cancelled");
    }

    let (Some(code), Some(oauth_state)) = (query.code, query.state) else {
        return frontend_error_redirect(&state, "invalid_callback");
    };

    match state
        .github_auth
        .finish_authorization(&code, &oauth_state)
        .await
    {
        Ok(grant) => {
            let url = state
                .github_auth
                .frontend_redirect("github_grant", &grant)
                .map_err(auth_error)?;
            Ok(Redirect::temporary(&url))
        }
        Err(auth_failure) => {
            error!(error = %auth_failure, "Failed to complete GitHub authorization");
            frontend_error_redirect(&state, "authentication_failed")
        }
    }
}

pub(super) async fn exchange_grant(
    State(state): State<AppState>,
    Extension(client): Extension<ClientContext>,
    Json(payload): Json<GrantExchangeRequest>,
) -> Result<Json<SessionResponse>, AuthHttpError> {
    if payload.grant.trim().is_empty() {
        return Err(response_error(
            StatusCode::BAD_REQUEST,
            "GitHub grant is required",
        ));
    }

    let exchange = state
        .github_auth
        .exchange_grant(&payload.grant, &client.device_uid)
        .await
        .map_err(auth_error)?;
    let max_requests = effective_max_requests(&state, Some(&exchange.identity));
    Ok(Json(session_response(
        exchange.identity,
        max_requests,
        Some(exchange.expires_at),
        Some(exchange.token),
    )))
}

pub(super) async fn get_session(
    State(state): State<AppState>,
    Extension(client): Extension<ClientContext>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, AuthHttpError> {
    let token = required_bearer_token(&headers)?;
    let identity = state
        .github_auth
        .session(token, &client.device_uid)
        .await
        .map_err(auth_error)?;
    let max_requests = effective_max_requests(&state, Some(&identity));
    Ok(Json(session_response(identity, max_requests, None, None)))
}

pub(super) async fn delete_session(
    State(state): State<AppState>,
    Extension(_client): Extension<ClientContext>,
    headers: HeaderMap,
) -> Result<StatusCode, AuthHttpError> {
    let token = required_bearer_token(&headers)?;
    state
        .github_auth
        .delete_session(token)
        .await
        .map_err(auth_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn optional_identity(
    state: &AppState,
    headers: &HeaderMap,
    client: &ClientContext,
) -> Result<Option<GitHubIdentity>, AuthHttpError> {
    let Some(token) = optional_bearer_token(headers)? else {
        return Ok(None);
    };
    state
        .github_auth
        .session(token, &client.device_uid)
        .await
        .map(Some)
        .map_err(auth_error)
}

pub(super) fn effective_max_requests(state: &AppState, identity: Option<&GitHubIdentity>) -> u32 {
    identity
        .map(|identity| identity.max_requests)
        .filter(|max_requests| *max_requests > 0)
        .unwrap_or(state.config.antifraud.successful_claim_window.max_requests)
}

fn session_response(
    identity: GitHubIdentity,
    max_requests: u32,
    expires_at: Option<i64>,
    token: Option<String>,
) -> SessionResponse {
    SessionResponse {
        authenticated: true,
        github_user_id: identity.github_user_id,
        login: identity.login,
        tier: identity.tier,
        max_requests,
        account_age_days: identity.account_age_days,
        public_repos: identity.public_repos,
        followers: identity.followers,
        expires_at,
        token,
    }
}

fn frontend_error_redirect(state: &AppState, error_code: &str) -> Result<Redirect, AuthHttpError> {
    let url = state
        .github_auth
        .frontend_redirect("github_error", error_code)
        .map_err(auth_error)?;
    Ok(Redirect::temporary(&url))
}

fn required_bearer_token(headers: &HeaderMap) -> Result<&str, AuthHttpError> {
    optional_bearer_token(headers)?
        .ok_or_else(|| response_error(StatusCode::UNAUTHORIZED, "GitHub session token is required"))
}

fn optional_bearer_token(headers: &HeaderMap) -> Result<Option<&str>, AuthHttpError> {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| response_error(StatusCode::UNAUTHORIZED, "Invalid authorization header"))?;
    let token = value.strip_prefix("Bearer ").unwrap_or_default().trim();
    if token.is_empty() {
        return Err(response_error(
            StatusCode::UNAUTHORIZED,
            "Invalid authorization header",
        ));
    }
    Ok(Some(token))
}

fn auth_error(error: AuthError) -> AuthHttpError {
    match error {
        AuthError::Disabled => response_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub authentication is disabled",
        ),
        AuthError::CapacityReached => response_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many recent GitHub authorization attempts",
        ),
        AuthError::InvalidAuthorization | AuthError::InvalidSession => response_error(
            StatusCode::UNAUTHORIZED,
            "Invalid or expired GitHub session",
        ),
        AuthError::Internal(error) => {
            tracing::error!(error = %error, "GitHub authentication failed");
            response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GitHub authentication failed",
            )
        }
    }
}

fn response_error(status: StatusCode, error: &'static str) -> AuthHttpError {
    (status, Json(ErrorResponse { error }))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, header::AUTHORIZATION};

    use super::optional_bearer_token;

    #[test]
    fn parses_bearer_session_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer opaque-token".parse().unwrap());

        assert_eq!(
            optional_bearer_token(&headers).unwrap(),
            Some("opaque-token")
        );
    }

    #[test]
    fn allows_missing_optional_session() {
        assert_eq!(optional_bearer_token(&HeaderMap::new()).unwrap(), None);
    }
}
