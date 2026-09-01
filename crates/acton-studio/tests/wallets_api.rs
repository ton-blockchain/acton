use std::collections::BTreeMap;

use acton_studio::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints, EnvironmentRuntime,
    EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus, StudioEnvironment,
    StudioServer, StudioServerConfig, StudioWallet, UpdateEnvironmentRequest, WalletRuntime,
    WalletRuntimeError, WalletRuntimeFuture,
};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response};
use expect_test::expect;
use tower::ServiceExt;

struct TestEnvironmentRuntime {
    environments: BTreeMap<String, StudioEnvironment>,
}

impl TestEnvironmentRuntime {
    fn new() -> Self {
        let localnet = StudioEnvironment::new(
            "localnet",
            "Localnet",
            EnvironmentStatus::Running,
            EnvironmentConfig::ActonLocalnet {
                port: 5411,
                fork_network: None,
                fork_block_number: None,
                accounts: vec!["deployer".to_owned()],
                rate_limit: None,
                response_delay_ms: None,
                block_interval_ms: None,
                no_mining: false,
                mine_empty_blocks: false,
            },
            EnvironmentEndpoints {
                api_v2: Some("http://127.0.0.1:5411/api/v2".to_owned()),
                api_v3: Some("http://127.0.0.1:5411/api/v3".to_owned()),
                config: None,
                control: Some("http://127.0.0.1:5411".to_owned()),
            },
        );
        let full_node = StudioEnvironment::new(
            "full-node",
            "Full TON network",
            EnvironmentStatus::Running,
            EnvironmentConfig::FullTonNetwork {
                api_v2_port: 18_080,
                api_v3_port: 18_081,
                admin_port: 18_082,
                config_port: 18_083,
                imported_accounts: Vec::new(),
            },
            EnvironmentEndpoints {
                api_v2: Some("http://127.0.0.1:18080/api/v2".to_owned()),
                api_v3: Some("http://127.0.0.1:18081/api/v3".to_owned()),
                config: Some("http://127.0.0.1:18083".to_owned()),
                control: Some("http://127.0.0.1:18082".to_owned()),
            },
        );
        let mut read_only = localnet.clone();
        "read-only".clone_into(&mut read_only.id);
        "Read only".clone_into(&mut read_only.name);
        read_only.capabilities.clear();
        let mut stopped = localnet.clone();
        "stopped".clone_into(&mut stopped.id);
        "Stopped network".clone_into(&mut stopped.name);
        stopped.status = EnvironmentStatus::Stopped;

        Self {
            environments: [
                (localnet.id.clone(), localnet),
                (full_node.id.clone(), full_node),
                (read_only.id.clone(), read_only),
                (stopped.id.clone(), stopped),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn unavailable<T>() -> EnvironmentRuntimeFuture<'static, T> {
        Box::pin(async {
            Err(EnvironmentRuntimeError::Internal {
                code: "test_operation_unavailable",
                message: "This operation is not available in the wallet API test runtime"
                    .to_owned(),
            })
        })
    }
}

impl EnvironmentRuntime for TestEnvironmentRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        let environments = self.environments.values().cloned().collect();
        Box::pin(async move { Ok(environments) })
    }

    fn create(
        &self,
        _request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Self::unavailable()
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let result = self
            .environments
            .get(environment_id)
            .cloned()
            .ok_or_else(|| EnvironmentRuntimeError::NotFound {
                environment_id: environment_id.to_owned(),
            });
        Box::pin(async move { result })
    }

    fn update(
        &self,
        _environment_id: &str,
        _request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Self::unavailable()
    }

    fn delete(&self, _environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        Self::unavailable()
    }

    fn stop(&self, _environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Self::unavailable()
    }

    fn restart(&self, _environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Self::unavailable()
    }
}

struct TestWalletRuntime;

impl WalletRuntime for TestWalletRuntime {
    fn list(&self, _environment: &StudioEnvironment) -> WalletRuntimeFuture<'_, Vec<StudioWallet>> {
        Box::pin(async {
            Ok(vec![StudioWallet {
                name: "deployer".to_owned(),
                address: "EQC7mW7YSE93LQ7jZk3g4O9NYJ9KClLVFn4EKe3yYf4WZ4fN".to_owned(),
                public_key: format!("0x{}", "12".repeat(32)),
                version: "v5r1".to_owned(),
                wallet_id: 2_147_483_405,
                workchain: 0,
            }])
        })
    }

    fn sign(
        &self,
        _environment: &StudioEnvironment,
        wallet_name: &str,
        _bytes: Vec<u8>,
    ) -> WalletRuntimeFuture<'_, [u8; 64]> {
        let result = if wallet_name == "deployer" {
            Ok([0xab; 64])
        } else {
            Err(WalletRuntimeError::NotFound {
                wallet_name: wallet_name.to_owned(),
            })
        };
        Box::pin(async move { result })
    }
}

fn router() -> Router {
    StudioServer::new(StudioServerConfig::new("test-version"))
        .with_environment_runtime(TestEnvironmentRuntime::new())
        .with_wallet_runtime(TestWalletRuntime)
        .router()
}

async fn response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let body = String::from_utf8_lossy(&body);
    let separator = if body.is_empty() { "" } else { " " };
    format!("status: {status}\nbody:{separator}{body}")
}

async fn request_snapshot(request: Request<Body>) -> String {
    let response = router()
        .oneshot(request)
        .await
        .expect("wallet API request must succeed");
    response_snapshot(response).await
}

fn wallet_sign_request(environment_id: &str, wallet_name: &str, bytes: &str) -> Request<Body> {
    Request::post(format!(
        "/api/v1/environments/{environment_id}/wallets/{wallet_name}/sign"
    ))
    .header("content-type", "application/json")
    .body(Body::from(format!(r#"{{"bytes":"{bytes}"}}"#)))
    .expect("wallet sign request must be valid")
}

#[tokio::test]
async fn wallet_descriptors_are_safe_for_localnet_and_full_node() {
    let localnet = request_snapshot(
        Request::get("/api/v1/environments/localnet/wallets")
            .body(Body::empty())
            .expect("localnet wallet request must be valid"),
    )
    .await;
    let full_node = request_snapshot(
        Request::get("/api/v1/environments/full-node/wallets")
            .body(Body::empty())
            .expect("full node wallet request must be valid"),
    )
    .await;
    let actual = format!("LOCALNET\n{localnet}\n\nFULL NODE\n{full_node}");

    expect![[r#"LOCALNET
status: 200 OK
body: [{"name":"deployer","address":"EQC7mW7YSE93LQ7jZk3g4O9NYJ9KClLVFn4EKe3yYf4WZ4fN","publicKey":"0x1212121212121212121212121212121212121212121212121212121212121212","version":"v5r1","walletId":2147483405,"workchain":0}]

FULL NODE
status: 200 OK
body: [{"name":"deployer","address":"EQC7mW7YSE93LQ7jZk3g4O9NYJ9KClLVFn4EKe3yYf4WZ4fN","publicKey":"0x1212121212121212121212121212121212121212121212121212121212121212","version":"v5r1","walletId":2147483405,"workchain":0}]"#]]
        .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_signing_decodes_and_encodes_prefixed_hex() {
    let actual = request_snapshot(wallet_sign_request("full-node", "deployer", "0x0001fe")).await;

    expect![[r#"status: 200 OK
body: {"signature":"0xabababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab"}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_signing_accepts_empty_and_maximum_payloads() {
    let empty = request_snapshot(wallet_sign_request("localnet", "deployer", "0x")).await;
    let maximum = format!("0x{}", "00".repeat(64 * 1024));
    let maximum = request_snapshot(wallet_sign_request("localnet", "deployer", &maximum)).await;
    let actual = format!("EMPTY\n{empty}\n\nMAXIMUM\n{maximum}");

    expect![[r#"EMPTY
status: 200 OK
body: {"signature":"0xabababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab"}

MAXIMUM
status: 200 OK
body: {"signature":"0xabababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab"}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_signing_rejects_payloads_over_the_limit() {
    let oversized = format!("0x{}", "00".repeat(64 * 1024 + 1));
    let actual = request_snapshot(wallet_sign_request("localnet", "deployer", &oversized)).await;

    expect![[r#"status: 400 Bad Request
body: {"error":{"code":"wallet_signing_payload_too_large","message":"Signing payload exceeds the 65536 byte limit"}}"#]]
        .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_signing_rejects_invalid_hex() {
    let missing_prefix =
        request_snapshot(wallet_sign_request("localnet", "deployer", "0001fe")).await;
    let malformed = request_snapshot(wallet_sign_request("localnet", "deployer", "0xzz")).await;
    let actual = format!("MISSING PREFIX\n{missing_prefix}\n\nMALFORMED HEX\n{malformed}");

    expect![[r#"MISSING PREFIX
status: 400 Bad Request
body: {"error":{"code":"wallet_signing_payload_invalid","message":"Signing bytes must be a 0x-prefixed hexadecimal string"}}

MALFORMED HEX
status: 400 Bad Request
body: {"error":{"code":"wallet_signing_payload_invalid","message":"Signing bytes are not valid hexadecimal: Invalid character 'z' at position 0"}}"#]]
        .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_signing_reports_unknown_wallets() {
    let actual = request_snapshot(wallet_sign_request("localnet", "missing", "0x01")).await;

    expect![[r#"status: 404 Not Found
body: {"error":{"code":"wallet_not_found","message":"Wallet missing was not found"}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_api_rejects_environments_without_wallet_capability() {
    let actual = request_snapshot(
        Request::get("/api/v1/environments/read-only/wallets")
            .body(Body::empty())
            .expect("read-only wallet request must be valid"),
    )
    .await;

    expect![[r#"status: 400 Bad Request
body: {"error":{"code":"environment_wallets_unavailable","message":"Wallets are not available in Read only"}}"#]]
        .assert_eq(&actual);
}

#[tokio::test]
async fn wallet_api_rejects_stopped_and_missing_environments() {
    let stopped = request_snapshot(
        Request::get("/api/v1/environments/stopped/wallets")
            .body(Body::empty())
            .expect("stopped environment wallet request must be valid"),
    )
    .await;
    let missing = request_snapshot(
        Request::get("/api/v1/environments/missing/wallets")
            .body(Body::empty())
            .expect("missing environment wallet request must be valid"),
    )
    .await;
    let actual = format!("STOPPED\n{stopped}\n\nMISSING\n{missing}");

    expect![[r#"STOPPED
status: 409 Conflict
body: {"error":{"code":"environment_not_running","message":"Environment Stopped network is not running"}}

MISSING
status: 404 Not Found
body: {"error":{"code":"environment_not_found","message":"Environment missing was not found"}}"#]]
        .assert_eq(&actual);
}
