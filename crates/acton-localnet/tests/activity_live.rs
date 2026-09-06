//! Opt-in checks against a dedicated running Full localnet. The caller supplies
//! its network directory; these tests spend faucet funds and replace activity settings.

use acton_localnet::{
    activity::{ActivityConfig, ActivityState, ActivityStatus, ScenarioWeights},
    client::Client,
};
use expect_test::expect;
use serde_json::json;
use std::{path::Path, time::Duration};

async fn wait_for(
    client: &Client,
    ready: impl Fn(&ActivityState) -> bool + Send + Sync,
) -> ActivityState {
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let state = client.activity().await.expect("activity state");
            if ready(&state) {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("activity reached the expected state")
}

#[tokio::test]
#[ignore = "requires ACTON_ACTIVITY_TEST_NETWORK_DIR pointing to a dedicated running localnet"]
async fn bursts_confirm_on_chain_and_cancellation_releases_every_slot() {
    let root = std::env::var("ACTON_ACTIVITY_TEST_NETWORK_DIR").expect("test network directory");
    let client = Client::connect(Path::new(&root))
        .await
        .expect("localnet client");
    let previous = client.activity().await.expect("saved settings");
    expect![["false"]].assert_eq(
        &matches!(
            previous.status,
            ActivityStatus::Running | ActivityStatus::Stopping
        )
        .to_string(),
    );

    // An oversized burst must only occupy available slots. Stop while funding is
    // pending, then start fresh runs to prove cancellation leaves no queued replay.
    let config = ActivityConfig {
        interval_seconds: 3600,
        scenarios_per_launch: 1000,
        concurrency: 3,
        ..Default::default()
    };
    client
        .configure_activity(config, true)
        .await
        .expect("start burst");
    let occupied = wait_for(&client, |state| state.active == 3).await;
    let stopped = client.stop_activity().await.expect("stop burst");
    expect![[r#"
        {
          "activeAfterStop": 0,
          "cancelled": 3,
          "occupied": 3,
          "skipped": 997,
          "status": "stopped"
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "occupied": occupied.active,
            "skipped": stopped.skipped,
            "activeAfterStop": stopped.active,
            "status": stopped.status,
            "cancelled": stopped.recent.len(),
        }))
        .expect("cancellation snapshot"),
    );

    let mut results = Vec::new();
    for (name, count) in [
        ("transfers", 20),
        ("batches", 2),
        ("jettons", 2),
        ("nfts", 2),
    ] {
        let config = ActivityConfig {
            interval_seconds: 3600,
            scenarios_per_launch: count,
            concurrency: count,
            duration_seconds: 1,
            max_batch_size: 128,
            scenarios: ScenarioWeights {
                transfers: u16::from(name == "transfers"),
                batches: u16::from(name == "batches"),
                jettons: u16::from(name == "jettons"),
                nfts: u16::from(name == "nfts"),
            },
            ..Default::default()
        };
        client
            .configure_activity(config, true)
            .await
            .expect("start scenarios");
        let finished = wait_for(&client, |state| state.status == ActivityStatus::Completed).await;
        results.push(json!({
            "scenario": name,
            "completed": finished.completed,
            "confirmedMessages": finished.confirmed_messages,
            "failed": finished.failed,
            "errors": finished.recent.iter().filter_map(|run| run.error.as_deref()).collect::<Vec<_>>(),
        }));
    }

    let randomized = ActivityConfig {
        interval_seconds: 3600,
        scenarios_per_launch: 20,
        concurrency: 20,
        duration_seconds: 1,
        max_batch_size: 8,
        randomize_batch_size: true,
        scenarios: ScenarioWeights {
            transfers: 0,
            batches: 1,
            jettons: 0,
            nfts: 0,
        },
        ..Default::default()
    };
    client
        .configure_activity(randomized, true)
        .await
        .expect("randomized batches");
    let finished = wait_for(&client, |state| state.status == ActivityStatus::Completed).await;
    let sizes: Vec<_> = finished
        .recent
        .iter()
        .filter_map(|run| run.batch_size)
        .collect();
    expect![["completed=20 failed=0 sizes=20 bounds=true confirmed=true"]].assert_eq(&format!(
        "completed={} failed={} sizes={} bounds={} confirmed={}",
        finished.completed,
        finished.failed,
        sizes.len(),
        sizes.iter().all(|size| (2..=8).contains(size)),
        sizes.iter().copied().map(u64::from).sum::<u64>() == finished.confirmed_messages,
    ));

    client
        .configure_activity(previous.config, false)
        .await
        .expect("restore settings");
    expect![[r#"
        [
          {
            "completed": 20,
            "confirmedMessages": 20,
            "errors": [],
            "failed": 0,
            "scenario": "transfers"
          },
          {
            "completed": 2,
            "confirmedMessages": 256,
            "errors": [],
            "failed": 0,
            "scenario": "batches"
          },
          {
            "completed": 2,
            "confirmedMessages": 6,
            "errors": [],
            "failed": 0,
            "scenario": "jettons"
          },
          {
            "completed": 2,
            "confirmedMessages": 4,
            "errors": [],
            "failed": 0,
            "scenario": "nfts"
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&results).expect("on-chain results"));
}
