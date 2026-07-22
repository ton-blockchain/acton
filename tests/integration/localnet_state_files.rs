use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::localnet::{
    latest_masterchain_seqno, parse_address_balance, pretty_json_for_snapshot, response_payload,
    summarize_admin_response,
};
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use std::fs;

const GIVER_ADDRESS: &str = "0:5555555555555555555555555555555555555555555555555555555555555555";

#[test]
fn localnet_state_dump_and_load_replace_live_state_and_clear_checkpoints() {
    let project = ProjectBuilder::new("localnet-state-transfer").build();
    let node = project
        .localnet()
        .args(["--no-mining", "--mine-empty-blocks"])
        .start();

    let first_mine = node.post_json("/acton_mine", &json!({}));
    let dumped_seqno = response_payload(&first_mine)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let state_path = project.path().join("state.json");
    let state_path_arg = state_path.display().to_string();
    let port = node.port().to_string();
    let dump_output = project
        .acton()
        .args(["localnet", "state", "dump"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&port)
        .run()
        .success();
    let state_json = fs::read(&state_path).expect("state command must write the JSON file");
    let state_document: Value =
        serde_json::from_slice(&state_json).expect("dumped state must be valid JSON");
    let checkpoint = node.post_json(
        "/acton_createCheckpoint",
        &json!({ "name": "cleared-on-load" }),
    );

    let target = "0:5555555555555555555555555555555555555555555555555555555555555555";
    let funded = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": target,
            "amount": 1_000_000_000u128,
        }),
    );
    let second_mine = node.post_json("/acton_mine", &json!({}));
    let mutated_seqno = response_payload(&second_mine)["last_block_seqno"]
        .as_u64()
        .expect("mine response must expose last_block_seqno") as u32;
    let target_after_mutation =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));

    let load_output = project
        .acton()
        .args(["localnet", "state", "load"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&port)
        .run()
        .success();
    let loaded_seqno = latest_masterchain_seqno(&node);
    let target_after_load =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let checkpoints_after_load = node.get_json("/acton_listCheckpoints");

    let snapshot = json!({
        "dump": {
            "output": dump_output.get_stdout().trim(),
            "seqno": dumped_seqno,
            "state_head_seqno": state_document["globals"]["head_seqno"].as_u64(),
        },
        "mutate": {
            "checkpoint": summarize_admin_response(&checkpoint),
            "fund_ok": funded["ok"].as_bool(),
            "seqno": mutated_seqno,
            "balance": parse_address_balance(&target_after_mutation).to_string(),
        },
        "load": {
            "output": load_output.get_stdout().trim(),
            "seqno": loaded_seqno,
            "balance": parse_address_balance(&target_after_load).to_string(),
            "checkpoints": summarize_admin_response(&checkpoints_after_load),
        },
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_state_transfer.summary.json"),
    );

    node.stop();
}

#[test]
fn sqlite_state_dump_preserves_transactions_and_historical_account_states() {
    let project = ProjectBuilder::new("localnet-sqlite-state-transfer").build();
    let db_path = project.path().join("localnet.sqlite");
    let db_path_arg = db_path.display().to_string();
    let state_path = project.path().join("state-from-sqlite.json");
    let state_path_arg = state_path.display().to_string();
    let target = "0:6666666666666666666666666666666666666666666666666666666666666666";

    let node = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            db_path_arg.as_str(),
        ])
        .start();
    node.post_json("/acton_mine", &json!({}));
    let first_fund = node.post_json(
        "/acton_fundAccount",
        &json!({"address": target, "amount": 1_000_000_000_u128}),
    );
    let first_state_block =
        response_payload(&node.post_json("/acton_mine", &json!({})))["last_block_seqno"]
            .as_u64()
            .expect("mine response must expose last_block_seqno") as u32;
    let second_fund = node.post_json(
        "/acton_fundAccount",
        &json!({"address": target, "amount": 2_000_000_000_u128}),
    );
    node.post_json("/acton_mine", &json!({}));

    let live = persistence_view(&node, target, first_state_block);
    node.stop();

    let reopened = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            db_path_arg.as_str(),
        ])
        .start();
    let sqlite = persistence_view(&reopened, target, first_state_block);
    let reopened_port = reopened.port().to_string();
    let dump_output = project
        .acton()
        .args(["localnet", "state", "dump"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&reopened_port)
        .run()
        .success();
    let state_document: Value = serde_json::from_slice(
        &fs::read(&state_path).expect("state dump from SQLite-backed node must exist"),
    )
    .expect("state dump from SQLite-backed node must be valid JSON");
    reopened.stop();

    let restored = project
        .localnet()
        .args(["--no-mining", "--mine-empty-blocks"])
        .start();
    let restored_port = restored.port().to_string();
    let load_output = project
        .acton()
        .args(["localnet", "state", "load"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&restored_port)
        .run()
        .success();
    let loaded = persistence_view(&restored, target, first_state_block);
    restored.stop();

    let imported_db_path = project.path().join("imported-localnet.sqlite");
    let imported_db_path_arg = imported_db_path.display().to_string();
    let imported = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            imported_db_path_arg.as_str(),
        ])
        .start();
    let imported_port = imported.port().to_string();
    let sqlite_load_output = project
        .acton()
        .args(["localnet", "state", "load"])
        .arg(&state_path_arg)
        .arg("--port")
        .arg(&imported_port)
        .run()
        .success();
    let imported_view = persistence_view(&imported, target, first_state_block);
    imported.stop();

    let reopened_import = project
        .localnet()
        .args([
            "--no-mining",
            "--mine-empty-blocks",
            "--db-path",
            imported_db_path_arg.as_str(),
        ])
        .start();
    let reopened_import_view = persistence_view(&reopened_import, target, first_state_block);

    let delta_blocks = state_document["history_deltas_by_seqno"]
        .as_array()
        .expect("state dump must contain account delta slots");
    let snapshot = json!({
        "setup": {
            "first_fund_ok": first_fund["ok"].as_bool(),
            "second_fund_ok": second_fund["ok"].as_bool(),
            "historical_state_differs_from_latest": live["historical_balance"] != live["latest_balance"],
            "transaction_count": live["transaction_hashes"].as_array().map_or(0, Vec::len),
        },
        "sqlite_reopen": {
            "head_seqno_preserved": sqlite["head_seqno"] == live["head_seqno"],
            "giver_balance_preserved": sqlite["giver_balance"] == live["giver_balance"],
            "latest_state_preserved": sqlite["latest_balance"] == live["latest_balance"],
            "historical_state_preserved": sqlite["historical_balance"] == live["historical_balance"],
            "transactions_preserved": sqlite["transaction_hashes"] == live["transaction_hashes"],
        },
        "dump": {
            "output": dump_output.get_stdout().trim(),
            "head_seqno": state_document["globals"]["head_seqno"],
            "transaction_count": state_document["history_tx_by_hash"].as_array().map_or(0, Vec::len),
            "message_count": state_document["history_msg_by_hash"].as_array().map_or(0, Vec::len),
            "message_to_transaction_count": state_document["history_msg_to_tx"].as_array().map_or(0, Vec::len),
            "delta_block_slots": delta_blocks.len(),
            "non_empty_delta_blocks": delta_blocks
                .iter()
                .filter(|deltas| deltas.as_array().is_some_and(|items| !items.is_empty()))
                .count(),
        },
        "clean_node_load": {
            "output": load_output.get_stdout().trim(),
            "head_seqno_preserved": loaded["head_seqno"] == live["head_seqno"],
            "giver_balance_preserved": loaded["giver_balance"] == live["giver_balance"],
            "latest_state_preserved": loaded["latest_balance"] == live["latest_balance"],
            "historical_state_preserved": loaded["historical_balance"] == live["historical_balance"],
            "transactions_preserved": loaded["transaction_hashes"] == live["transaction_hashes"],
        },
        "sqlite_load": {
            "output": sqlite_load_output.get_stdout().trim(),
            "head_seqno_preserved": imported_view["head_seqno"] == live["head_seqno"],
            "giver_balance_preserved": imported_view["giver_balance"] == live["giver_balance"],
            "latest_state_preserved": imported_view["latest_balance"] == live["latest_balance"],
            "historical_state_preserved": imported_view["historical_balance"] == live["historical_balance"],
            "transactions_preserved": imported_view["transaction_hashes"] == live["transaction_hashes"],
        },
        "sqlite_load_after_reopen": {
            "head_seqno_preserved": reopened_import_view["head_seqno"] == live["head_seqno"],
            "giver_balance_preserved": reopened_import_view["giver_balance"] == live["giver_balance"],
            "latest_state_preserved": reopened_import_view["latest_balance"] == live["latest_balance"],
            "historical_state_preserved": reopened_import_view["historical_balance"] == live["historical_balance"],
            "transactions_preserved": reopened_import_view["transaction_hashes"] == live["transaction_hashes"],
        },
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_sqlite_state_transfer.summary.json"),
    );

    reopened_import.stop();
}

fn persistence_view(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    historical_seqno: u32,
) -> Value {
    let latest = node.get_json(&format!("/api/v2/getAddressInformation?address={address}"));
    let giver = node.get_json(&format!(
        "/api/v2/getAddressInformation?address={GIVER_ADDRESS}"
    ));
    let historical = node.get_json(&format!(
        "/api/v2/getAddressInformation?address={address}&seqno={historical_seqno}"
    ));
    let transactions = node.get_json(&format!(
        "/api/v3/transactions?account={address}&limit=100&sort=asc"
    ));
    let mut transaction_hashes = transactions["transactions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|transaction| transaction["hash"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    transaction_hashes.sort();

    json!({
        "head_seqno": latest_masterchain_seqno(node),
        "giver_balance": parse_address_balance(&giver).to_string(),
        "latest_balance": parse_address_balance(&latest).to_string(),
        "historical_balance": parse_address_balance(&historical).to_string(),
        "transaction_hashes": transaction_hashes,
    })
}
