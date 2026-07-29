use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use acton_studio::{
    STUDIO_API_VERSION, STUDIO_ENVIRONMENTS_PATH, STUDIO_INFO_PATH, StudioInfo,
    load_studio_daemon_descriptor,
};
use ton_networks::{CustomNetworkUrls, set_runtime_testnet_urls};

const STUDIO_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);
const STUDIO_REQUEST_TIMEOUT: Duration = Duration::from_millis(250);

#[must_use]
pub fn configured_studio_url(project_root: &Path) -> Option<String> {
    if let Some(url) = std::env::var("ACTON_STUDIO_URL")
        .ok()
        .and_then(|value| normalize_base_url(&value))
    {
        return Some(url);
    }

    let descriptor = load_studio_daemon_descriptor(project_root).ok()??;
    if descriptor.protocol_version != STUDIO_API_VERSION {
        return None;
    }
    normalize_base_url(&descriptor.url)
}

#[must_use]
pub fn activate_studio_testnet_gateway(project_root: &Path, expected_workspace_name: &str) -> bool {
    let Some(studio_url) = configured_studio_url(project_root) else {
        return false;
    };
    if !is_matching_studio_running(&studio_url, expected_workspace_name) {
        return false;
    }

    set_runtime_testnet_urls(testnet_gateway_urls(&studio_url)).is_ok()
}

fn is_matching_studio_running(studio_url: &str, expected_workspace_name: &str) -> bool {
    let Ok(client) = reqwest::blocking::Client::builder()
        .connect_timeout(STUDIO_CONNECT_TIMEOUT)
        .timeout(STUDIO_REQUEST_TIMEOUT)
        .build()
    else {
        return false;
    };
    let Ok(info) = client
        .get(format!("{studio_url}{STUDIO_INFO_PATH}"))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::json::<StudioInfo>)
    else {
        return false;
    };

    info.protocol_version == STUDIO_API_VERSION
        && info
            .workspace
            .is_some_and(|workspace| workspace.name == expected_workspace_name)
}

fn testnet_gateway_urls(studio_url: &str) -> CustomNetworkUrls {
    let proxy_url = format!("{studio_url}{STUDIO_ENVIRONMENTS_PATH}/testnet/rpc");
    CustomNetworkUrls {
        v2_url: Arc::from(format!("{proxy_url}/api/v2")),
        v3_url: Some(Arc::from(format!("{proxy_url}/api/v3"))),
        explorer_url: None,
    }
}

fn normalize_base_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return None;
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url.as_str().trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acton_studio::{StudioServer, StudioServerConfig, StudioWorkspace};

    #[test]
    fn builds_normalized_testnet_gateway_urls() {
        let studio_url = normalize_base_url("  http://127.0.0.1:3016/?ignored=true#fragment  ")
            .expect("url should normalize");
        let urls = testnet_gateway_urls(&studio_url);

        assert_eq!(
            urls.v2_url.as_ref(),
            "http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v2"
        );
        assert_eq!(
            urls.v3_url.as_deref(),
            Some("http://127.0.0.1:3016/api/v1/environments/testnet/rpc/api/v3")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn requires_a_running_studio_for_the_current_workspace() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be available");
        let server = StudioServer::new(StudioServerConfig::new("test").with_workspace(
            StudioWorkspace::new("matching-project", "/tmp/matching-project"),
        ));
        let task = tokio::spawn(async move { axum::serve(listener, server.router()).await });
        let studio_url = format!("http://{address}");

        let matching_url = studio_url.clone();
        assert!(
            tokio::task::spawn_blocking(move || {
                is_matching_studio_running(&matching_url, "matching-project")
            })
            .await
            .expect("probe should complete")
        );

        let mismatched_url = studio_url.clone();
        assert!(
            !tokio::task::spawn_blocking(move || {
                is_matching_studio_running(&mismatched_url, "another-project")
            })
            .await
            .expect("probe should complete")
        );

        task.abort();
    }
}
