#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use acton_studio::StudioWallet;
use ed25519_dalek::{Signature, VerifyingKey};
use expect_test::expect;
use rston::boc::BocRepr;
use rston::models::{Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, ShardAccount};
use serde_json::{Value, json};

use super::StudioCliProcess;
use crate::support::project::ProjectBuilder;

#[test]
fn studio_genesis_wallets_match_funded_accounts_and_sign_with_their_keys() {
    let project = ProjectBuilder::new("studio-genesis-wallets").build();
    let mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
    let wallets = [
        ("v5", "v5r1", ""),
        ("explicit", "v5r1", "wallet-id = 42\n"),
        ("v4", "v4r2", ""),
    ]
        .map(|(name, kind, wallet_id)| {
            format!("[wallets.{name}]\nkind = \"{kind}\"\n{wallet_id}keys = {{ mnemonic = \"{mnemonic}\" }}\n")
        })
        .join("\n");
    std::fs::write(project.path().join("wallets.toml"), wallets).unwrap();

    // Only the Docker process and TON readiness endpoints are fixtures. Wallet
    // resolution, genesis account preparation, and signing run in the real CLI.
    let bin = project.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let docker = bin.join("docker");
    std::fs::write(&docker, include_str!("../../fixtures/localnet/docker.py")).unwrap();
    std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let studio = StudioCliProcess::start_with_env(
        &project,
        &[
            (
                "PATH",
                format!(
                    "{}:{}",
                    bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            ),
            ("ACTON_LOCALNET_IMAGE", "localton:fixture".to_owned()),
            ("DOCKER_CONTEXT", "localnet-test".to_owned()),
            ("LOCALNET_TEST_DIR", project.path().display().to_string()),
        ],
    );
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut results = Vec::new();

    for global_id in [-42, -3] {
        let created: Value = client
            .post(format!("{}/api/v1/environments", studio.url()))
            .json(&json!({
                "name": format!("Wallets {global_id}"),
                "config": {
                    "kind": "fullTonNetwork",
                    "genesisToml": format!("global_id = {global_id}"),
                    "accounts": ["explicit", "v4", "v5"],
                },
            }))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        let id = created["id"].as_str().unwrap();
        let url = format!("{}/api/v1/environments/{id}", studio.url());
        let mut api_tasks = Vec::new();
        for field in ["apiV2Port", "apiV3Port"] {
            let port = u16::try_from(created["config"][field].as_u64().unwrap()).unwrap();
            let listener = runtime
                .block_on(tokio::net::TcpListener::bind((
                    std::net::Ipv4Addr::LOCALHOST,
                    port,
                )))
                .unwrap();
            let router = axum::Router::new().fallback(|| async {
                axum::Json(json!({
                    "ok": true,
                    "result": {"last": {"seqno": 10}},
                    "last": {"seqno": 10},
                }))
            });
            api_tasks.push(runtime.spawn(async move {
                axum::serve(listener, router).await.unwrap();
            }));
        }

        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            let environment: Value = client.get(&url).send().unwrap().json().unwrap();
            match environment["status"].as_str() {
                Some("running") => break,
                Some("failed") => panic!("Studio startup failed: {environment}"),
                _ if Instant::now() >= deadline => {
                    panic!("Studio startup timed out: {environment}")
                }
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }

        let listed: Vec<StudioWallet> = client
            .get(format!("{url}/wallets"))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        let networks = runtime
            .block_on(acton_localnet::catalog::list(
                &project.path().join(".acton-localnet"),
            ))
            .unwrap();
        let network = &networks
            .iter()
            .find(|entry| entry.network.name == format!("Wallets {global_id}"))
            .unwrap()
            .network;
        for prepared in &network.config.startup_wallets {
            let shard: ShardAccount = BocRepr::decode_hex(&prepared.shard_account_boc_hex).unwrap();
            let account = shard.load_account().unwrap().unwrap();
            let IntAddr::Std(address) = account.address else {
                panic!("standard wallet address")
            };
            let wallet = listed
                .iter()
                .find(|wallet| wallet.name == prepared.name)
                .unwrap();
            let encoded_address = DisplayBase64StdAddr {
                addr: &address,
                flags: Base64StdAddrFlags {
                    testnet: true,
                    bounceable: false,
                    base64_url: true,
                },
            }
            .to_string();
            let message = b"genesis wallet signing check";
            let signed: Value = client
                .post(format!("{url}/wallets/{}/sign", wallet.name))
                .json(&json!({"bytes": format!("0x{}", hex::encode(message))}))
                .send()
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .unwrap();
            let public_key = hex::decode(wallet.public_key.strip_prefix("0x").unwrap()).unwrap();
            let signature = hex::decode(
                signed["signature"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("0x")
                    .unwrap(),
            )
            .unwrap();
            let verifies = VerifyingKey::from_bytes(&public_key.try_into().unwrap())
                .unwrap()
                .verify_strict(message, &Signature::from_slice(&signature).unwrap())
                .is_ok();
            results.push(format!(
                "{global_id} {}: id={} funded={} signs={verifies}",
                wallet.name,
                wallet.wallet_id,
                wallet.address == encoded_address,
            ));
        }
        client
            .delete(&url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
        for task in api_tasks {
            task.abort();
            let _ = runtime.block_on(task);
        }
    }
    studio.stop();

    expect![[r"
        -42 explicit: id=42 funded=true signs=true
        -42 v4: id=698983191 funded=true signs=true
        -42 v5: id=2147483606 funded=true signs=true
        -3 explicit: id=42 funded=true signs=true
        -3 v4: id=698983191 funded=true signs=true
        -3 v5: id=2147483645 funded=true signs=true"]]
    .assert_eq(&results.join("\n"));
}
