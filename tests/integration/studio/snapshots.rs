use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde_json::{Value, json};

use super::StudioCliProcess;
use crate::common::assertion;
use crate::support::project::ProjectBuilder;

#[cfg(unix)]
#[test]
fn studio_simulated_snapshots_survive_restore_and_restart() {
    let project = ProjectBuilder::new("studio-snapshots").build();
    let mut studio = StudioCliProcess::start(&project);
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Studio client");
    let response = client
        .post(format!("{}/api/v1/environments", studio.url()))
        .json(&json!({
            "name": "Snapshot test",
            "config": {
                "kind": "actonSimulatedLocalnet",
                "accounts": [],
                "noMining": true,
            },
        }))
        .send()
        .expect("create request");
    let status = response.status();
    let created: Value = response.json().expect("environment JSON");
    assert!(status.is_success(), "create failed: {created}");
    let id = created["id"].as_str().expect("environment ID");
    let url = format!("{}/api/v1/environments/{id}", studio.url());
    let ready = wait_until(&client, &url, |value| {
        value["status"] == "running" || value["status"] == "failed"
    });
    if ready["status"] == "failed" {
        let mut child = studio.child.take().expect("owned Studio process");
        child.kill().expect("stop failed test process");
        let output = child.wait_with_output().expect("Studio diagnostics");

        panic!(
            "environment failed: {ready}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    post(
        &client,
        &format!("{url}/rpc/acton_setMiningMode"),
        json!({"skip_empty_blocks": false}),
    );
    post(&client, &format!("{url}/rpc/acton_mine"), json!({}));
    post(
        &client,
        &format!("{url}/snapshots"),
        json!({"name": "Initial state"}),
    );
    let initial = wait_until(&client, &format!("{url}/snapshot-operation"), |value| {
        value["phase"] == "completed"
    });
    let snapshot_id = initial["snapshotId"].as_str().expect("snapshot ID");
    let download_url = format!("{url}/snapshots/{snapshot_id}/download");
    let exported = client
        .get(&download_url)
        .send()
        .expect("download request")
        .error_for_status()
        .expect("download response");
    let attachment = exported.headers()["content-disposition"]
        .to_str()
        .expect("download header")
        .to_owned();
    let bytes = exported.bytes().expect("snapshot bytes");

    post(
        &client,
        &format!("{url}/rpc/acton_mine"),
        json!({"blocks": 2}),
    );
    post(
        &client,
        &format!("{url}/rpc/acton_increaseTime"),
        json!({"seconds": 600}),
    );
    post(
        &client,
        &format!("{url}/snapshots"),
        json!({"name": "Later state"}),
    );
    wait_until(&client, &format!("{url}/snapshot-operation"), |value| {
        value["phase"] == "completed"
    });

    let before_import = get(&client, &format!("{url}/rpc/acton_nodeInfo"));
    let imported: Value = client
        .post(format!("{url}/snapshots/import"))
        .header("Content-Type", "application/json")
        .body(bytes.clone())
        .send()
        .expect("import request")
        .error_for_status()
        .expect("import response")
        .json()
        .expect("import JSON");
    let after_import = get(&client, &format!("{url}/rpc/acton_nodeInfo"));

    // An uptime of several seconds distinguishes an in-process restore from a fresh node.
    let before_restore = wait_until(&client, &format!("{url}/rpc/acton_nodeInfo"), |value| {
        value["result"]["uptime_seconds"]
            .as_u64()
            .is_some_and(|uptime| uptime >= 2)
    });
    post(
        &client,
        &format!("{url}/snapshots/{snapshot_id}/restore"),
        json!({}),
    );
    wait_until(&client, &format!("{url}/snapshot-operation"), |value| {
        value["phase"] == "completed"
    });
    let restored = get(&client, &format!("{url}/rpc/acton_nodeInfo"));
    let inventory = get(&client, &format!("{url}/snapshots"));
    let unchanged_export = client.get(&download_url).send().unwrap().bytes().unwrap() == bytes;

    client
        .post(format!("{url}/stop"))
        .send()
        .expect("stop request")
        .error_for_status()
        .expect("stopped environment");
    post(
        &client,
        &format!("{url}/snapshots"),
        json!({"name": "While stopped"}),
    );
    wait_until(&client, &format!("{url}/snapshot-operation"), |value| {
        value["phase"] == "completed"
    });
    let stopped_after_save = get(&client, &url)["status"].clone();
    studio.stop();

    let restarted = StudioCliProcess::start(&project);
    let url = format!("{}/api/v1/environments/{id}", restarted.url());
    let persisted = get(&client, &format!("{url}/snapshots"));
    let stopped_after_restart = get(&client, &url)["status"].clone();
    post(
        &client,
        &format!("{url}/snapshots/{snapshot_id}/restore"),
        json!({}),
    );
    wait_until(&client, &format!("{url}/snapshot-operation"), |value| {
        value["phase"] == "completed"
    });
    let restored_after_restart = get(&client, &format!("{url}/rpc/acton_nodeInfo"));
    let deleted = client
        .delete(format!("{url}/snapshots/{snapshot_id}"))
        .send()
        .unwrap()
        .status()
        .as_u16();
    let remaining = get(&client, &format!("{url}/snapshots"));

    let summary = json!({
        "capability": created["capabilities"].as_array().unwrap().contains(&json!("snapshots")),
        "attachment": attachment,
        "import": {
            "new_id": imported["id"] != snapshot_id,
            "head_unchanged": after_import["result"]["last_block_seqno"]
                == before_import["result"]["last_block_seqno"],
            "clock_unchanged": after_import["result"]["time_offset_seconds"]
                == before_import["result"]["time_offset_seconds"],
        },
        "restore": {
            "head": restored["result"]["last_block_seqno"],
            "clock": restored["result"]["time_offset_seconds"],
            "process_kept_running": restored["result"]["uptime_seconds"].as_u64()
                >= before_restore["result"]["uptime_seconds"].as_u64(),
            "snapshot_count": inventory.as_array().unwrap().len(),
            "saved_bytes_unchanged": unchanged_export,
        },
        "restart": {
            "status_after_stopped_save": stopped_after_save,
            "status_after_studio_restart": stopped_after_restart,
            "persisted_snapshot_count": persisted.as_array().unwrap().len(),
            "restored_head": restored_after_restart["result"]["last_block_seqno"],
            "delete_status": deleted,
            "remaining_snapshot_count": remaining.as_array().unwrap().len(),
        },
    });
    assertion().eq(
        serde_json::to_string_pretty(&summary).unwrap(),
        snapbox::file!("../../snapshots/studio/simulated_snapshots.json"),
    );

    client
        .delete(&url)
        .send()
        .expect("delete request")
        .error_for_status()
        .expect("deleted environment");
    restarted.stop();
}

fn get(client: &Client, url: &str) -> Value {
    client
        .get(url)
        .send()
        .expect("GET request")
        .error_for_status()
        .expect("successful GET response")
        .json()
        .expect("GET JSON")
}

fn post(client: &Client, url: &str, body: Value) -> Value {
    client
        .post(url)
        .json(&body)
        .send()
        .expect("POST request")
        .error_for_status()
        .expect("successful POST response")
        .json()
        .expect("POST JSON")
}

fn wait_until(client: &Client, url: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);

    loop {
        let value = get(client, url);
        if done(&value) {
            return value;
        }

        assert!(value["phase"] != "failed", "snapshot failed: {value}");
        if value["status"] == "failed" {
            panic!(
                "environment failed: {}",
                get(client, &format!("{url}/startup"))
            );
        }
        assert!(Instant::now() < deadline, "request timed out: {value}");
        std::thread::sleep(Duration::from_millis(50));
    }
}
