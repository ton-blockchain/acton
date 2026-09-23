use std::io::Write;

use super::{
    BlockchainClient, BlockchainError, MultiNetworkToncenterClient, client, client_for_network,
    normalize_code_hash, user_agent,
};
use crate::config::{Config, TonNetwork};

#[tokio::test]
async fn selected_network_sends_application_headers() {
    use axum::{Json, Router, http::HeaderMap, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let router = Router::new().route(
        "/api/v3/accountStates",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["user-agent"], user_agent());
            assert_eq!(headers["x-api-key"], "testnet-key");
            Json(serde_json::json!({"accounts": []}))
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let mut config_file = tempfile::NamedTempFile::new().expect("config file");
    writeln!(
        config_file,
        "[toncenter]\ntestnet_base_url = \"http://{address}\"\ntestnet_api_key = \"testnet-key\""
    )
    .expect("write config");
    let config = Config::load_from_path(config_file.path()).expect("load config");
    let client = client_for_network(&config, TonNetwork::Testnet).expect("client");
    assert_eq!(
        client.get_code_hash("0:account").await.expect("lookup"),
        None
    );
    server.abort();
}

#[tokio::test]
async fn account_lookup_rejects_malformed_hashes_and_normalizes_valid_ones() {
    use axum::{Json, Router, routing::get};
    for (hash, expected) in [
        ("../unexpected", None),
        (
            "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
            Some("a".repeat(64)),
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let router = Router::new().route(
            "/api/v3/accountStates",
            get(
                move || async move { Json(serde_json::json!({"accounts": [{"code_hash": hash}]})) },
            ),
        );
        let server = tokio::spawn(async move { axum::serve(listener, router).await });
        let result = client(&format!("http://{address}"), None)
            .expect("client")
            .get_code_hash("0:account")
            .await;
        server.abort();
        if let Some(expected) = expected {
            assert_eq!(result.expect("valid hash"), Some(expected));
        } else {
            assert!(matches!(result, Err(BlockchainError::InvalidCodeHash)));
        }
    }
}

#[tokio::test]
async fn multi_network_lookup_finds_one_network_and_rejects_duplicates() {
    use axum::{Json, Router, extract::Query, routing::get};
    use std::collections::HashMap;

    const MAINNET_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const TESTNET_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let server_address = listener.local_addr().expect("address");
    let router = Router::new()
        .route(
            "/mainnet/api/v3/accountStates",
            get(|Query(query): Query<HashMap<String, String>>| async move {
                let code_hash = match query.get("address").map(String::as_str) {
                    Some("mainnet-only" | "duplicate") => Some(MAINNET_HASH),
                    _ => None,
                };
                Json(serde_json::json!({
                    "accounts": code_hash.map(|code_hash| vec![serde_json::json!({
                        "code_hash": code_hash
                    })]).unwrap_or_default()
                }))
            }),
        )
        .route(
            "/testnet/api/v3/accountStates",
            get(|Query(query): Query<HashMap<String, String>>| async move {
                let code_hash = match query.get("address").map(String::as_str) {
                    Some("testnet-only" | "duplicate") => Some(TESTNET_HASH),
                    _ => None,
                };
                Json(serde_json::json!({
                    "accounts": code_hash.map(|code_hash| vec![serde_json::json!({
                        "code_hash": code_hash
                    })]).unwrap_or_default()
                }))
            }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let client = MultiNetworkToncenterClient::new(
        client(&format!("http://{server_address}/mainnet"), None).expect("mainnet client"),
        client(&format!("http://{server_address}/testnet"), None).expect("testnet client"),
    );

    assert_eq!(
        client.get_code_hash("mainnet-only").await.expect("lookup"),
        Some(MAINNET_HASH.to_owned())
    );
    assert_eq!(
        client.get_code_hash("testnet-only").await.expect("lookup"),
        Some(TESTNET_HASH.to_owned())
    );
    assert_eq!(client.get_code_hash("missing").await.expect("lookup"), None);
    assert!(matches!(
        client.get_code_hash("duplicate").await,
        Err(BlockchainError::AddressFoundOnBothNetworks {
            address,
            mainnet_code_hash,
            testnet_code_hash,
        }) if address == "duplicate"
            && mainnet_code_hash == MAINNET_HASH
            && testnet_code_hash == TESTNET_HASH
    ));

    server.abort();
}

#[test]
fn normalize_code_hash_keeps_hex_as_lowercase() {
    assert_eq!(
        normalize_code_hash("AF8F72E22D3DD6EEC1F312693C026E4D1751E2DFEC9B3F6577E8C8B3A668947C"),
        "af8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c"
    );
}

#[test]
fn normalize_code_hash_decodes_base64_to_hex() {
    assert_eq!(
        normalize_code_hash("r49y4i091u7B8xJpPAJuTRdR4t/smz9ld+jIs6ZolHw="),
        "af8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c"
    );
}
