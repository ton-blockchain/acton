use acton_studio::{
    EnvironmentRuntimeError, PublicTonNetwork, StartVerificationRequest, StudioServer,
    StudioServerConfig, VerificationFuture, VerificationOperation, VerificationPaymentRequest,
    VerificationPhase, VerificationPreview, VerificationRuntime, VerificationStatus,
};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use expect_test::expect;
use serde_json::json;
use tower::ServiceExt;

struct TestVerificationRuntime;

impl VerificationRuntime for TestVerificationRuntime {
    fn status(
        &self,
        network: PublicTonNetwork,
        address: String,
    ) -> VerificationFuture<'_, VerificationStatus> {
        Box::pin(async move {
            Ok(VerificationStatus {
                address,
                code_hash: "deployed-code".to_owned(),
                verified: network == PublicTonNetwork::Mainnet,
                verifier_url: format!("https://verifier.example/{network:?}"),
            })
        })
    }

    fn preview(
        &self,
        network: PublicTonNetwork,
        address: String,
    ) -> VerificationFuture<'_, VerificationPreview> {
        Box::pin(async move {
            Ok(VerificationPreview {
                id: "reviewed-sources".to_owned(),
                status: self.status(network, address).await?,
                compiler_version: "1.4.2".to_owned(),
                candidates: Vec::new(),
                payment: None,
            })
        })
    }

    fn start(
        &self,
        network: PublicTonNetwork,
        request: StartVerificationRequest,
    ) -> VerificationFuture<'_, VerificationOperation> {
        self.operation(network, request.preview_id)
    }

    fn operation(
        &self,
        network: PublicTonNetwork,
        id: String,
    ) -> VerificationFuture<'_, VerificationOperation> {
        Box::pin(async move {
            if network != PublicTonNetwork::Testnet {
                return Err(EnvironmentRuntimeError::Conflict {
                    code: "verification_preview_expired",
                    message: "This preview belongs to another network".to_owned(),
                });
            }

            Ok(VerificationOperation {
                id,
                phase: VerificationPhase::ConfirmingPayment,
                message: None,
                error: None,
            })
        })
    }

    fn complete_payment(
        &self,
        network: PublicTonNetwork,
        id: String,
        _request: VerificationPaymentRequest,
    ) -> VerificationFuture<'_, VerificationOperation> {
        self.operation(network, id)
    }
}

async fn snapshot(router: Router, request: Request<Body>) -> String {
    let response = router.oneshot(request).await.expect("API response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");

    format!("{status}\n{}", String::from_utf8_lossy(&body))
}

#[tokio::test]
async fn verification_routes_preserve_network_and_two_phase_publication() {
    let router = StudioServer::new(StudioServerConfig::new("test"))
        .with_verification_runtime(TestVerificationRuntime)
        .router();
    let mut responses = Vec::new();

    for network in ["mainnet", "testnet"] {
        let base = format!("/api/v1/environments/{network}/verification");
        let requests = [
            Request::get(format!("{base}/status?address=contract"))
                .body(Body::empty())
                .expect("status request"),
            Request::post(format!("{base}/preview"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"address": "contract"}).to_string()))
                .expect("preview request"),
            Request::post(format!("{base}/operations"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "previewId": "reviewed-sources",
                        "contractId": "Counter",
                        "senderAddress": "payer",
                    })
                    .to_string(),
                ))
                .expect("publication request"),
            Request::get(format!("{base}/operations/reviewed-sources"))
                .body(Body::empty())
                .expect("operation request"),
            Request::post(format!("{base}/operations/reviewed-sources/payment"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"messageHash": "33".repeat(32)}).to_string(),
                ))
                .expect("payment request"),
        ];

        for request in requests {
            responses.push(snapshot(router.clone(), request).await);
        }
    }

    expect![[r#"
        200 OK
        {"address":"contract","codeHash":"deployed-code","verified":true,"verifierUrl":"https://verifier.example/Mainnet"}

        200 OK
        {"id":"reviewed-sources","status":{"address":"contract","codeHash":"deployed-code","verified":true,"verifierUrl":"https://verifier.example/Mainnet"},"compilerVersion":"1.4.2","candidates":[],"payment":null}

        409 Conflict
        {"error":{"code":"verification_preview_expired","message":"This preview belongs to another network"}}

        409 Conflict
        {"error":{"code":"verification_preview_expired","message":"This preview belongs to another network"}}

        409 Conflict
        {"error":{"code":"verification_preview_expired","message":"This preview belongs to another network"}}

        200 OK
        {"address":"contract","codeHash":"deployed-code","verified":false,"verifierUrl":"https://verifier.example/Testnet"}

        200 OK
        {"id":"reviewed-sources","status":{"address":"contract","codeHash":"deployed-code","verified":false,"verifierUrl":"https://verifier.example/Testnet"},"compilerVersion":"1.4.2","candidates":[],"payment":null}

        200 OK
        {"id":"reviewed-sources","phase":"confirmingPayment","message":null,"error":null}

        200 OK
        {"id":"reviewed-sources","phase":"confirmingPayment","message":null,"error":null}

        200 OK
        {"id":"reviewed-sources","phase":"confirmingPayment","message":null,"error":null}"#]].assert_eq(&responses.join("\n\n"));
}

#[tokio::test]
async fn verification_without_a_project_returns_an_actionable_error() {
    let router = StudioServer::new(StudioServerConfig::new("test")).router();
    let response = snapshot(
        router,
        Request::post("/api/v1/environments/testnet/verification/preview")
            .header("content-type", "application/json")
            .body(Body::from(json!({"address": "contract"}).to_string()))
            .expect("preview request"),
    )
    .await;

    expect![[r#"409 Conflict
{"error":{"code":"verification_project_required","message":"Open Studio from an Acton project to verify its Tolk contracts"}}"#]]
        .assert_eq(&response);
}
