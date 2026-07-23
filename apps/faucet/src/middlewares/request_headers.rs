use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode, header::USER_AGENT},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::debug;

const ALLOWED_USER_AGENT_PREFIX: &str = "acton/";
const DEVICE_UID_HEADER: &str = "x-device-uid";
const DEFAULT_DEVICE_UID: &str = "default";

pub async fn require_airdrop_headers(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let user_agent = headers.get(USER_AGENT);
    let device_uid = headers.get(DEVICE_UID_HEADER);
    let is_user_agent_allowed = user_agent.is_some_and(is_allowed_user_agent);
    let is_device_uid_allowed = device_uid.is_some_and(is_allowed_device_uid);

    if is_user_agent_allowed && is_device_uid_allowed {
        return next.run(request).await;
    }

    if !is_user_agent_allowed {
        debug!(
            header = %USER_AGENT,
            value = header_value(user_agent),
            "Airdrop request header failed validation"
        );
    }
    if !is_device_uid_allowed {
        debug!(
            header = DEVICE_UID_HEADER,
            value = header_value(device_uid),
            "Airdrop request header failed validation"
        );
    }

    StatusCode::BAD_REQUEST.into_response()
}

fn is_allowed_user_agent(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(|user_agent| {
        user_agent
            .strip_prefix(ALLOWED_USER_AGENT_PREFIX)
            .is_some_and(|version| !version.trim().is_empty())
    })
}

fn is_allowed_device_uid(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(|device_uid| {
        device_uid == DEFAULT_DEVICE_UID || matches!(device_uid.len(), 32 | 36)
    })
}

fn header_value(value: Option<&HeaderValue>) -> &str {
    match value {
        Some(value) => value.to_str().unwrap_or("<non-utf8>"),
        None => "<missing>",
    }
}

#[cfg(test)]
mod tests {
    use super::{is_allowed_device_uid, is_allowed_user_agent};

    #[test]
    fn allows_acton_package_version_user_agent() {
        assert!(is_allowed_user_agent(&"acton/0.1.0".parse().unwrap()));
        assert!(is_allowed_user_agent(
            &"acton/1.2.3-beta.1+build.5".parse().unwrap()
        ));
        assert!(is_allowed_user_agent(
            &"acton/0.1.0 (debug)".parse().unwrap()
        ));
    }

    #[test]
    fn rejects_missing_or_non_acton_version() {
        assert!(!is_allowed_user_agent(&"acton/".parse().unwrap()));
        assert!(!is_allowed_user_agent(&"acton/ ".parse().unwrap()));
        assert!(!is_allowed_user_agent(&"faucet/0.1.0".parse().unwrap()));
    }

    #[test]
    fn allows_device_uid_values_from_supported_platforms() {
        assert!(is_allowed_device_uid(&"default".parse().unwrap()));
        assert!(is_allowed_device_uid(
            &"87c4bc1848a84471997203ee530d2fda".parse().unwrap()
        ));
        assert!(is_allowed_device_uid(
            &"550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
        ));
        assert!(is_allowed_device_uid(
            &"550E8400-E29B-41D4-A716-446655440000".parse().unwrap()
        ));
    }

    #[test]
    fn rejects_invalid_device_uid() {
        assert!(!is_allowed_device_uid(&"".parse().unwrap()));
        assert!(!is_allowed_device_uid(&" ".parse().unwrap()));
        assert!(!is_allowed_device_uid(&"device-1".parse().unwrap()));
        assert!(!is_allowed_device_uid(&"another".parse().unwrap()));
        assert!(!is_allowed_device_uid(
            &"{00000000-0000-0000-0000-000000000000}".parse().unwrap()
        ));
    }
}
