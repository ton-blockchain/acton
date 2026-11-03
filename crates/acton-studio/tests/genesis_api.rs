use acton_studio::{StudioServer, StudioServerConfig};
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use expect_test::expect;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn genesis_import_returns_lossless_fields_and_rejects_invalid_creation() {
    let app = StudioServer::new(StudioServerConfig::new("test")).router();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/genesis")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"toml":"global_id = -42\ncell_price = 9007199254740993"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let fields = body["sections"][0]["fields"].as_array().unwrap();
    let price = fields
        .iter()
        .find(|field| field["key"] == "cell_price")
        .unwrap();
    expect![["200 OK 9007199254740993 -42"]].assert_eq(&format!(
        "{status} {} {}",
        price["value"].as_str().unwrap(),
        acton_localnet::GenesisConfig::from_toml(body["toml"].as_str().unwrap())
            .unwrap()
            .global_id,
    ));
    let response = app.oneshot(Request::builder().method("POST").uri("/api/v1/environments")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name":"invalid", "config":{"kind":"fullTonNetwork", "genesisToml":"global_id = 0"}}).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    expect![[r#"
        400 Bad Request
        {"error":{"code":"invalid_genesis","message":"Network ID must be negative"}}"#]]
    .assert_eq(&format!("{status}\n{body}"));
}

#[tokio::test]
async fn genesis_editor_reports_related_fields_and_nested_type_errors() {
    let app = StudioServer::new(StudioServerConfig::new("test")).router();
    let cases = [
        ("elections", "elected_for_seconds = 60"),
        (
            "multiple constraints",
            "global_id = 0\n[gas]\ngas_credit = 1000001\n[block_limits.bytes]\nsoft = 1",
        ),
        ("nested integer width", "[voting]\nmin_rounds = 256"),
        ("corrected elections", "elected_for_seconds = 120"),
    ];
    let mut results = Vec::new();
    for (name, source) in cases {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/genesis")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"toml": source}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        results.push(json!({
            "case": name,
            "status": status.as_u16(),
            "issues": body.get("issues"),
            "error": body.get("error"),
            "fields": body.get("fields"),
        }));
    }
    expect![[r#"
        [
          {
            "case": "elections",
            "error": null,
            "fields": null,
            "issues": [
              {
                "fields": [
                  "election_start_before_seconds",
                  "elected_for_seconds"
                ],
                "message": "Elections start offset must be less than the validator round duration: 90 s ≥ 60 s"
              }
            ],
            "status": 200
          },
          {
            "case": "multiple constraints",
            "error": null,
            "fields": null,
            "issues": [
              {
                "fields": [
                  "gas.gas_credit",
                  "gas.gas_limit"
                ],
                "message": "Workchain gas: gas credit and flat gas limit must not exceed the transaction gas limit"
              },
              {
                "fields": [
                  "block_limits.bytes.underload",
                  "block_limits.bytes.soft",
                  "block_limits.bytes.hard"
                ],
                "message": "Workchain block limits (bytes): the underload threshold must be below the soft limit, and the soft limit below the hard limit"
              },
              {
                "fields": [
                  "global_id"
                ],
                "message": "Network ID must be negative"
              }
            ],
            "status": 200
          },
          {
            "case": "nested integer width",
            "error": {
              "code": "invalid_genesis",
              "message": "invalid genesis.toml: invalid value: integer `256`, expected u8"
            },
            "fields": [
              "voting.min_rounds"
            ],
            "issues": null,
            "status": 400
          },
          {
            "case": "corrected elections",
            "error": null,
            "fields": null,
            "issues": [],
            "status": 200
          }
        ]"#]].assert_eq(&serde_json::to_string_pretty(&results).unwrap());
}
