use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::localnet::{
    block_header_gen_utime, latest_masterchain_seqno, parse_address_balance,
    pretty_json_for_snapshot, response_payload, summarize_admin_response,
    wait_for_address_balance_at_least,
};
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use std::time::Duration;
use ton_api::toncenter::v2::responses::TonlibErrorResponse;

#[test]
fn localnet_runtime_checkpoints_restore_state_and_persistent_db() {
    let project = ProjectBuilder::new("localnet-checkpoints").build();
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

    let older_checkpoint = node.post_json("/acton_createCheckpoint", &json!({ "name": "older" }));

    let next_block_timestamp = first_gen_utime + 300;
    let set_next = node.post_json(
        "/acton_setNextBlockTimestamp",
        &json!({ "timestamp": next_block_timestamp }),
    );
    let current_checkpoint =
        node.post_json("/acton_createCheckpoint", &json!({ "name": "current" }));

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

    let newer_checkpoint = node.post_json("/acton_createCheckpoint", &json!({ "name": "newer" }));
    let list_before_restore = node.get_json("/acton_listCheckpoints");
    let increase_later = node.post_json("/acton_increaseTime", &json!({ "seconds": 60 }));
    let status_after_later_change = node.get_json("/acton_nodeInfo");

    let restored = node.post_json("/acton_restoreCheckpoint", &json!({ "name": "current" }));
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

    let restored_same = node.post_json("/acton_restoreCheckpoint", &json!({ "name": "current" }));
    let restored_newer = node.post_json("/acton_restoreCheckpoint", &json!({ "name": "newer" }));
    let target_after_newer =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let list_after_restore = node.get_json("/acton_listCheckpoints");
    let restored_older = node.post_json("/acton_restoreCheckpoint", &json!({ "name": "older" }));
    let seqno_after_restore_older = latest_masterchain_seqno(&node);
    let status_after_restore_older = node.get_json("/acton_nodeInfo");
    let target_after_restore_older =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let deleted_newer = node.post_json("/acton_deleteCheckpoint", &json!({ "name": "newer" }));
    let cleared = node.post_json("/acton_clearCheckpoints", &json!({}));
    let list_after_clear = node.get_json("/acton_listCheckpoints");

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

    let snapshot = json!({
        "create": {
            "older": summarize_admin_response(&older_checkpoint),
            "current": summarize_admin_response(&current_checkpoint),
            "newer": summarize_admin_response(&newer_checkpoint),
            "list": summarize_admin_response(&list_before_restore),
        },
        "mutate_after_checkpoint": {
            "set_next_ok": set_next["ok"].as_bool(),
            "fund_ok": fund["ok"].as_bool(),
            "mine_ok": mine_faucet["ok"].as_bool(),
            "faucet_block_used_pending_timestamp": faucet_block_gen_utime == next_block_timestamp,
            "balance_after_mine": parse_address_balance(&target_after_mine).to_string(),
            "increase_later_ok": increase_later["ok"].as_bool(),
        },
        "restore_current": {
            "response": summarize_admin_response(&restored),
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
        "checkpoints_are_non_destructive": {
            "current_restores_again": summarize_admin_response(&restored_same),
            "newer_still_restores": summarize_admin_response(&restored_newer),
            "balance_after_newer_restore": parse_address_balance(&target_after_newer).to_string(),
            "list_after_current_restore": summarize_admin_response(&list_after_restore),
            "older_still_restores": summarize_admin_response(&restored_older),
            "seqno_after_restore_older": seqno_after_restore_older,
            "pending_timestamp_after_restore_older": status_after_restore_older["result"]["next_block_timestamp"].clone(),
            "balance_after_restore_older": parse_address_balance(&target_after_restore_older).to_string(),
        },
        "checkpoint_management": {
            "deleted_newer": summarize_admin_response(&deleted_newer),
            "clear": summarize_admin_response(&cleared),
            "list_after_clear": summarize_admin_response(&list_after_clear),
        },
        "persistent_db_after_restart": {
            "seqno": restarted_seqno,
            "balance": parse_address_balance(&restarted_target).to_string(),
            "block_2_removed": restarted_block_2["ok"].as_bool() == Some(false),
        }
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_checkpoints.summary.json"),
    );

    restarted.stop();
}

#[test]
fn failed_forced_checkpoint_import_preserves_existing_checkpoint() {
    let project = ProjectBuilder::new("localnet-checkpoint-import-atomicity").build();
    let node = project.localnet().args(["--no-mining"]).start();
    let created = node.post_json("/acton_createCheckpoint", &json!({ "name": "stable" }));

    let (status, error): (u16, TonlibErrorResponse) = node.post_bytes_with_status_as(
        "/acton_importCheckpoint?name=stable&force=true",
        b"not valid checkpoint JSON".to_vec(),
    );
    let mut invalid_state = node.get_json("/acton_dumpState");
    invalid_state["globals"]["config_boc_hash"] = Value::String("ff".repeat(32));
    let (semantic_status, semantic_error): (u16, TonlibErrorResponse) = node
        .post_bytes_with_status_as(
            "/acton_importCheckpoint?name=stable&force=true",
            serde_json::to_vec(&invalid_state).expect("invalid checkpoint must serialize"),
        );
    let listed = node.get_json("/acton_listCheckpoints");
    let restored = node.post_json("/acton_restoreCheckpoint", &json!({ "name": "stable" }));

    let summary = json!({
        "created": {
            "ok": created["ok"],
            "result": created["result"],
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
            "result": listed["result"],
        },
        "restored_original": {
            "ok": restored["ok"],
            "result": restored["result"],
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/acton_checkpoint_force_import_is_atomic.json"),
    );

    node.stop();
}

#[test]
fn localnet_checkpoint_cli_manages_and_transfers_checkpoints() {
    let project = ProjectBuilder::new("localnet-checkpoint-cli").build();
    let node = project.localnet().args(["--no-mining"]).start();
    let port = node.port().to_string();
    let checkpoint_path = project.path().join("stable.json");
    let checkpoint_path_arg = checkpoint_path.display().to_string();

    let create = project
        .acton()
        .args(["localnet", "checkpoint", "create", "stable", "--port"])
        .arg(&port)
        .run()
        .success();
    let list = project
        .acton()
        .args(["localnet", "checkpoint", "list", "--port"])
        .arg(&port)
        .run()
        .success();
    let export = project
        .acton()
        .args(["localnet", "checkpoint", "export", "stable", "--out"])
        .arg(&checkpoint_path_arg)
        .arg("--port")
        .arg(&port)
        .run()
        .success();
    let delete = project
        .acton()
        .args(["localnet", "checkpoint", "delete", "stable", "--port"])
        .arg(&port)
        .run()
        .success();
    let import = project
        .acton()
        .args(["localnet", "checkpoint", "import"])
        .arg(&checkpoint_path_arg)
        .args(["--name", "restored", "--port"])
        .arg(&port)
        .run()
        .success();
    let restore = project
        .acton()
        .args(["localnet", "checkpoint", "restore", "restored", "--port"])
        .arg(&port)
        .run()
        .success();
    let clear = project
        .acton()
        .args(["localnet", "checkpoint", "clear", "--port"])
        .arg(&port)
        .run()
        .success();
    let list_after_clear = project
        .acton()
        .args(["localnet", "checkpoint", "list", "--port"])
        .arg(&port)
        .run()
        .success();

    let summary = json!({
        "create": create.get_stdout().trim(),
        "list": list.get_stdout().trim(),
        "export": export.get_stdout().trim(),
        "delete": delete.get_stdout().trim(),
        "import": import.get_stdout().trim(),
        "restore": restore.get_stdout().trim(),
        "clear": clear.get_stdout().trim(),
        "list_after_clear": list_after_clear.get_stdout().trim(),
    });
    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&summary, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_checkpoint_cli.summary.json"),
    );

    node.stop();
}
