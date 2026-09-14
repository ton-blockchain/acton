use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::localnet::{
    block_header_gen_utime, latest_masterchain_seqno, parse_address_balance,
    pretty_json_for_snapshot, response_payload, wait_for_address_balance_at_least,
};
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use std::time::Duration;
use ton_api::toncenter::v2::responses::TonlibErrorResponse;

#[test]
fn localnet_runtime_snapshots_restore_state_and_persistent_db() {
    let project = ProjectBuilder::new("localnet-snapshots").build();
    let db_path = project.path().join("localnet.sqlite");
    let db_path_arg = db_path.display().to_string();

    let node = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            db_path_arg.as_str(),
        ])
        .start();

    let first_mine = node.post_json("/acton_mine", &json!({}));
    let first_seqno = response_payload(&first_mine)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let first_gen_utime = block_header_gen_utime(&node, first_seqno);

    let older_snapshot = node.post_json("/acton_createSnapshot", &json!({ "name": "older" }));

    let next_block_timestamp = first_gen_utime + 300;
    let set_next = node.post_json(
        "/acton_setNextBlockTimestamp",
        &json!({ "timestamp": next_block_timestamp }),
    );
    let current_snapshot = node.post_json("/acton_createSnapshot", &json!({ "name": "current" }));

    let target = "0:4444444444444444444444444444444444444444444444444444444444444444";
    let fund = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": target,
            "amount": 1_000_000_000u128,
        }),
    );
    let mine_faucet = node.post_json("/acton_mine", &json!({}));
    let faucet_block_seqno = response_payload(&mine_faucet)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let faucet_block_gen_utime = block_header_gen_utime(&node, faucet_block_seqno);
    let target_after_mine =
        wait_for_address_balance_at_least(&node, target, 1_000_000_000, Duration::from_secs(3));

    let newer_snapshot = node.post_json("/acton_createSnapshot", &json!({ "name": "newer" }));
    let list_before_restore = node.get_json("/acton_listSnapshots");
    let increase_later = node.post_json("/acton_increaseTime", &json!({ "seconds": 60 }));
    let status_after_later_change = node.get_json("/acton_nodeInfo");

    let restored = node.post_json(
        "/acton_restoreSnapshot",
        &json!({ "id": current_snapshot["result"]["id"] }),
    );
    let seqno_after_restore = latest_masterchain_seqno(&node);
    let target_after_restore =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let status_after_restore = node.get_json("/acton_nodeInfo");

    let mine_after_restore = node.post_json("/acton_mine", &json!({}));
    let replayed_seqno = response_payload(&mine_after_restore)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let replayed_gen_utime = block_header_gen_utime(&node, replayed_seqno);
    let target_after_empty_mine =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));

    let restored_same = node.post_json(
        "/acton_restoreSnapshot",
        &json!({ "id": current_snapshot["result"]["id"] }),
    );
    let restored_newer = node.post_json(
        "/acton_restoreSnapshot",
        &json!({ "id": newer_snapshot["result"]["id"] }),
    );
    let target_after_newer =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let list_after_restore = node.get_json("/acton_listSnapshots");
    let restored_older = node.post_json(
        "/acton_restoreSnapshot",
        &json!({ "id": older_snapshot["result"]["id"] }),
    );
    let seqno_after_restore_older = latest_masterchain_seqno(&node);
    let status_after_restore_older = node.get_json("/acton_nodeInfo");
    let target_after_restore_older =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let deleted_newer = node.post_json(
        "/acton_deleteSnapshot",
        &json!({ "id": newer_snapshot["result"]["id"] }),
    );
    let list_after_delete = node.get_json("/acton_listSnapshots");

    node.stop();

    let restarted = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            db_path_arg.as_str(),
        ])
        .start();
    let restarted_seqno = latest_masterchain_seqno(&restarted);
    let restarted_target =
        restarted.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let restarted_block_2 = restarted
        .get_json_error("/api/v2/getBlockHeader?workchain=0&shard=-9223372036854775808&seqno=2");

    let inventory_after_restart = restarted.get_json("/acton_listSnapshots");
    let restore_after_restart = restarted.post_json(
        "/acton_restoreSnapshot",
        &json!({"id": current_snapshot["result"]["id"]}),
    );
    let restarted_clock = restarted.get_json("/acton_nodeInfo");

    let snapshot = json!({
        "create": {
            "older": snapshot_response(&older_snapshot),
            "current": snapshot_response(&current_snapshot),
            "newer": snapshot_response(&newer_snapshot),
            "list": snapshot_response(&list_before_restore),
        },
        "mutate_after_snapshot": {
            "set_next_ok": set_next["ok"].as_bool(),
            "fund_ok": fund["ok"].as_bool(),
            "mine_ok": mine_faucet["ok"].as_bool(),
            "faucet_block_used_pending_timestamp": faucet_block_gen_utime == next_block_timestamp,
            "balance_after_mine": parse_address_balance(&target_after_mine).to_string(),
            "increase_later_ok": increase_later["ok"].as_bool(),
        },
        "restore_current": {
            "response": snapshot_response(&restored),
            "seqno_after_restore": seqno_after_restore,
            "balance_after_restore": parse_address_balance(&target_after_restore).to_string(),
            "pending_timestamp_restored": status_after_restore["result"]["next_block_timestamp"]
                .as_u64()
                == Some(u64::from(next_block_timestamp)),
            "time_offset_rolled_back": status_after_later_change["result"]["time_offset_seconds"]
                .as_i64()
                > status_after_restore["result"]["time_offset_seconds"].as_i64(),
            "empty_mine_used_restored_timestamp": replayed_gen_utime == next_block_timestamp,
            "balance_after_empty_mine": parse_address_balance(&target_after_empty_mine).to_string(),
        },
        "snapshots_are_non_destructive": {
            "current_restores_again": snapshot_response(&restored_same),
            "newer_still_restores": snapshot_response(&restored_newer),
            "balance_after_newer_restore": parse_address_balance(&target_after_newer).to_string(),
            "list_after_current_restore": snapshot_response(&list_after_restore),
            "older_still_restores": snapshot_response(&restored_older),
            "seqno_after_restore_older": seqno_after_restore_older,
            "pending_timestamp_after_restore_older": status_after_restore_older["result"]["next_block_timestamp"].clone(),
            "balance_after_restore_older": parse_address_balance(&target_after_restore_older).to_string(),
        },
        "snapshot_management": {
            "deleted_newer": snapshot_response(&deleted_newer),
            "list_after_delete": snapshot_response(&list_after_delete),
        },
        "persistent_db_after_restart": {
            "seqno": restarted_seqno,
            "balance": parse_address_balance(&restarted_target).to_string(),
            "block_2_removed": restarted_block_2["ok"].as_bool() == Some(false),
            "saved_files_survived": snapshot_response(&inventory_after_restart),
            "restored_saved_state": snapshot_response(&restore_after_restart),
            "pending_timestamp_restored": restarted_clock["result"]["next_block_timestamp"]
                .as_u64() == Some(u64::from(next_block_timestamp)),
        }
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_snapshots.summary.json"),
    );

    restarted.stop();
}

#[test]
fn failed_snapshot_import_preserves_inventory_and_live_state() {
    let project = ProjectBuilder::new("localnet-snapshot-import-atomicity").build();
    let node = project.localnet().args(["--no-mining"]).start();
    let created = node.post_json("/acton_createSnapshot", &json!({ "name": "stable" }));

    let (status, error): (u16, TonlibErrorResponse) = node.post_bytes_with_status_as(
        "/acton_importSnapshot?name=stable",
        b"not valid snapshot JSON".to_vec(),
    );
    let id = created["result"]["id"].as_str().expect("snapshot ID");
    let export_url = format!("/acton_exportSnapshot?id={id}");
    let mut invalid_state = node.get_json(&export_url);
    invalid_state["state"]["globals"]["config_boc_hash"] = Value::String("ff".repeat(32));
    let (semantic_status, semantic_error): (u16, TonlibErrorResponse) = node
        .post_bytes_with_status_as(
            "/acton_importSnapshot?name=stable",
            serde_json::to_vec(&invalid_state).expect("invalid snapshot must serialize"),
        );
    let listed = node.get_json("/acton_listSnapshots");
    let restored = node.post_json("/acton_restoreSnapshot", &json!({ "id": id }));

    let summary = json!({
        "created": {
            "ok": created["ok"],
            "result": snapshot_response(&created),
        },
        "failed_import": {
            "status": status,
            "code": error.code,
            "reported_error": !error.error.is_empty(),
        },
        "failed_semantic_import": {
            "status": semantic_status,
            "code": semantic_error.code,
            "reported_error": !semantic_error.error.is_empty(),
        },
        "list_after_failure": {
            "ok": listed["ok"],
            "result": snapshot_response(&listed),
        },
        "restored_original": {
            "ok": restored["ok"],
            "result": snapshot_response(&restored),
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/acton_snapshot_import_is_atomic.json"),
    );

    node.stop();
}

#[test]
fn localnet_snapshot_cli_manages_and_transfers_snapshots() {
    let project = ProjectBuilder::new("localnet-snapshot-cli").build();
    let node = project.localnet().args(["--no-mining"]).start();
    let port = node.port().to_string();
    let snapshot_path = project.path().join("stable.json");
    let snapshot_path_arg = snapshot_path.display().to_string();

    let run = |args: &[&str]| {
        project
            .acton()
            .args(["simulated-localnet", "snapshot", "--json", "--port", &port])
            .args(args)
            .run()
            .success()
    };
    let create = run(&["create", "stable"]);
    let created: Value = serde_json::from_str(&create.get_stdout()).expect("snapshot JSON");
    let id = created["id"].as_str().expect("created snapshot ID");
    let export = run(&["export", id, "--out", &snapshot_path_arg]);
    let exported = std::fs::read(&snapshot_path).expect("exported JSON");

    let duplicate_export = project
        .acton()
        .args(["simulated-localnet", "snapshot", "--port", &port])
        .args(["export", id, "--out", &snapshot_path_arg])
        .run()
        .failure();
    let existing_file_preserved = std::fs::read(&snapshot_path).unwrap() == exported;
    run(&["export", id, "--out", &snapshot_path_arg, "--force"]);

    let import = run(&["import", &snapshot_path_arg, "--name", "imported"]);
    let imported: Value = serde_json::from_str(&import.get_stdout()).expect("import JSON");
    let imported_id = imported["id"].as_str().expect("imported snapshot ID");
    let restore = run(&["restore", imported_id]);
    let restored: Value = serde_json::from_str(&restore.get_stdout()).expect("restore JSON");
    let list = run(&["list"]);
    let listed: Value = serde_json::from_str(&list.get_stdout()).expect("inventory JSON");
    run(&["delete", id]);
    run(&["delete", imported_id]);
    let empty = run(&["list"]);

    let summary = json!({
        "created": snapshot_response(&json!({"ok": true, "result": created})),
        "exported": export.get_stdout().contains(id),
        "export_requires_force": duplicate_export.get_stderr().contains("--force"),
        "failed_export_preserves_file": existing_file_preserved,
        "import_gets_new_id": imported_id != id,
        "restored_import": restored["id"] == imported["id"],
        "restore_preserves_inventory": snapshot_response(&json!({"ok": true, "result": listed})),
        "empty_after_delete": serde_json::from_str::<Value>(&empty.get_stdout()).unwrap(),
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_snapshot_cli.summary.json"),
    );

    node.stop();
}

/// Records useful metadata while keeping generated IDs, timestamps, and file sizes deterministic.
fn snapshot_response(response: &Value) -> Value {
    let describe = |snapshot: &Value| {
        json!({
            "name": snapshot["name"],
            "block_seqno": snapshot["block_seqno"],
            "has_id": snapshot["id"].as_str().is_some_and(|id| !id.is_empty()),
            "has_created_at": snapshot["created_at"].as_u64().is_some_and(|time| time > 0),
            "has_size": snapshot["size_bytes"].as_u64().is_some_and(|size| size > 0),
        })
    };
    let result = &response["result"];
    let metadata = if let Some(items) = result.as_array() {
        let mut items = items.iter().map(describe).collect::<Vec<_>>();
        items.sort_by_key(|item| item["name"].as_str().unwrap_or_default().to_owned());

        Value::Array(items)
    } else if result.is_null() {
        Value::Null
    } else {
        describe(result)
    };

    json!({"ok": response["ok"], "result": metadata})
}
