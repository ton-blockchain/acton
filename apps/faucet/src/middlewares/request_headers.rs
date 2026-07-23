use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode, header::USER_AGENT},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::debug;

const ALLOWED_USER_AGENT_PREFIX: &str = "acton/";
const ALLOWED_BROWSER_CLIENT_PREFIX: &str = "actonscan/";
pub const ACTON_CLIENT_HEADER: HeaderName = HeaderName::from_static("x-acton-client");
pub const DEVICE_UID_HEADER: HeaderName = HeaderName::from_static("x-device-uid");
const DEFAULT_DEVICE_UID: &str = "default";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientContext {
    pub device_uid: String,
}

pub async fn require_airdrop_headers(mut request: Request, next: Next) -> Response {
    let headers = request.headers();
    let user_agent = headers.get(USER_AGENT);
    let browser_client = headers.get(&ACTON_CLIENT_HEADER);
    let device_uid = headers.get(&DEVICE_UID_HEADER);
    let is_client_allowed = user_agent.is_some_and(is_allowed_user_agent)
        || browser_client.is_some_and(is_allowed_browser_client);
    let is_device_uid_allowed = device_uid.is_some_and(is_allowed_device_uid_header);

    if is_client_allowed && is_device_uid_allowed {
        let device_uid = device_uid
            .and_then(|value| value.to_str().ok())
            .expect("validated device UID must be UTF-8")
            .to_string();
        request
            .extensions_mut()
            .insert(ClientContext { device_uid });
        return next.run(request).await;
    }

    if !is_client_allowed {
        debug!(
            user_agent = header_value(user_agent),
            browser_client = header_value(browser_client),
            "Airdrop client headers failed validation"
        );
    }
    if !is_device_uid_allowed {
        debug!(
            header = %DEVICE_UID_HEADER,
            value = header_value(device_uid),
            "Airdrop request header failed validation"
        );
    }

    StatusCode::BAD_REQUEST.into_response()
}

fn is_allowed_browser_client(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(|client| {
        client
            .strip_prefix(ALLOWED_BROWSER_CLIENT_PREFIX)
            .is_some_and(|version| !version.trim().is_empty())
    })
}

fn is_allowed_user_agent(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(|user_agent| {
        user_agent
            .strip_prefix(ALLOWED_USER_AGENT_PREFIX)
            .is_some_and(|version| !version.trim().is_empty())
    })
}

pub fn is_allowed_device_uid(value: &str) -> bool {
    value == DEFAULT_DEVICE_UID || matches!(value.len(), 32 | 36)
}

fn is_allowed_device_uid_header(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(is_allowed_device_uid)
}

fn header_value(value: Option<&HeaderValue>) -> &str {
    match value {
        Some(value) => value.to_str().unwrap_or("<non-utf8>"),
        None => "<missing>",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_allowed_browser_client, is_allowed_device_uid, is_allowed_device_uid_header,
        is_allowed_user_agent,
    };

    #[test]
    fn allows_actonscan_version_browser_client() {
        assert!(is_allowed_browser_client(
            &"actonscan/1.0.0".parse().unwrap()
        ));
        assert!(is_allowed_browser_client(
            &"actonscan/1.0.0-beta.1".parse().unwrap()
        ));
    }

    #[test]
    fn rejects_missing_or_unknown_browser_client_version() {
        assert!(!is_allowed_browser_client(&"actonscan/".parse().unwrap()));
        assert!(!is_allowed_browser_client(&"actonscan/ ".parse().unwrap()));
        assert!(!is_allowed_browser_client(
            &"explorer/1.0.0".parse().unwrap()
        ));
    }

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
        assert!(is_allowed_device_uid_header(&"default".parse().unwrap()));
        assert!(is_allowed_device_uid_header(
            &"87c4bc1848a84471997203ee530d2fda".parse().unwrap()
        ));
        assert!(is_allowed_device_uid_header(
            &"550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
        ));
        assert!(is_allowed_device_uid_header(
            &"550E8400-E29B-41D4-A716-446655440000".parse().unwrap()
        ));
        assert!(is_allowed_device_uid("default"));
    }

    #[test]
    fn rejects_invalid_device_uid() {
        assert!(!is_allowed_device_uid_header(&"".parse().unwrap()));
        assert!(!is_allowed_device_uid_header(&" ".parse().unwrap()));
        assert!(!is_allowed_device_uid_header(&"device-1".parse().unwrap()));
        assert!(!is_allowed_device_uid_header(&"another".parse().unwrap()));
        assert!(!is_allowed_device_uid_header(
            &"{00000000-0000-0000-0000-000000000000}".parse().unwrap()
        ));
    }
}
