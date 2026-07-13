use crate::common::assertion;
use crate::support::localnet::{assert_v3_bad_request, assert_v3_error, pretty_json_for_snapshot};
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{active_shard_account_boc64, test_std_addr};
use serde_json::{Value, json};
use ton_api::toncenter::v3::{requests, responses};
use tycho_types::cell::Cell;

const ZERO_ADDRESS: &str = "0:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn collection_endpoints_deserialize_empty_responses() {
    let project = ProjectBuilder::new("localnet-v3-empty-collection-responses").build();
    let node = project.localnet().start();

    let pending_actions: responses::ActionsResponse =
        node.get_json_as(&format!("/api/v3/pendingActions?account={ZERO_ADDRESS}"));
    let pending_traces: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/pendingTraces?account={ZERO_ADDRESS}"));
    let traces: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/traces?account={ZERO_ADDRESS}"));
    let dns: responses::DnsRecordsResponse =
        node.get_json_as("/api/v3/dns/records?domain=missing.ton");
    let multisig_orders: responses::MultisigOrdersResponse =
        node.get_json_as(&format!("/api/v3/multisig/orders?address={ZERO_ADDRESS}"));
    let multisigs: responses::MultisigsResponse = node.get_json_as(&format!(
        "/api/v3/multisig/wallets?address={ZERO_ADDRESS}&include_orders=false"
    ));
    let vesting: responses::VestingContractsResponse =
        node.get_json_as(&format!("/api/v3/vesting?contract_address={ZERO_ADDRESS}"));

    let summary = json!({
        "pending_actions": {
            "count": pending_actions.actions.len(),
            "address_book_count": pending_actions.address_book.len(),
            "metadata_count": pending_actions.metadata.len(),
        },
        "pending_traces": {
            "count": pending_traces.traces.len(),
            "address_book_count": pending_traces.address_book.len(),
            "metadata_count": pending_traces.metadata.len(),
        },
        "traces": {
            "count": traces.traces.len(),
            "address_book_count": traces.address_book.len(),
            "metadata_count": traces.metadata.len(),
        },
        "dns_record_count": dns.records.len(),
        "multisig_order_count": multisig_orders.orders.len(),
        "multisig_count": multisigs.multisigs.len(),
        "vesting_count": vesting.vesting_contracts.len(),
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v3_empty_collection_responses.json"),
    );

    node.stop();
}

#[test]
fn filter_and_stack_validation_returns_typed_errors() {
    let project = ProjectBuilder::new("localnet-v3-validation-errors").build();
    let node = project.localnet().start();

    let mut summary = Vec::new();
    for (case, path, expected_error) in [
        (
            "pending actions without filter",
            "/api/v3/pendingActions",
            "account or ext_msg_hash should be specified",
        ),
        (
            "pending traces without filter",
            "/api/v3/pendingTraces",
            "account or ext_msg_hash should be specified",
        ),
        (
            "traces without filter",
            "/api/v3/traces",
            "Exactly one of `account`, `trace_id`, `tx_hash`, or `msg_hash` is required",
        ),
        (
            "dns without filter",
            "/api/v3/dns/records",
            "Exactly one of `wallet` or `domain` is required",
        ),
        (
            "multisig orders without filter",
            "/api/v3/multisig/orders",
            "At least one of `address` or `multisig_address` should be specified",
        ),
        (
            "multisig wallets without filter",
            "/api/v3/multisig/wallets",
            "At least one of `address` or `wallet_address` should be specified",
        ),
    ] {
        let (status, error): (u16, responses::RequestError) = node.get_json_with_status_as(path);
        assert_v3_bad_request(
            status,
            &serde_json::to_value(&error).expect("RequestError must serialize"),
            expected_error,
        );
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    for (case, stack_entry, expected_error) in [
        (
            "unsupported stack type",
            requests::StackEntry {
                kind: "unsupported".to_owned(),
                value: Value::Null,
            },
            "Unsupported v3 stack entry type",
        ),
        (
            "cell without bytes",
            requests::StackEntry {
                kind: "cell".to_owned(),
                value: json!({}),
            },
            "cell stack value must be a base64 string",
        ),
        (
            "tuple without elements",
            requests::StackEntry {
                kind: "tuple".to_owned(),
                value: json!({}),
            },
            "tuple stack value must be an array",
        ),
    ] {
        let request = requests::RunGetMethodRequest {
            address: ZERO_ADDRESS.to_owned(),
            method: "seqno".to_owned(),
            stack: vec![stack_entry],
        };
        let (status, error): (u16, responses::RequestError) =
            node.post_json_with_status_as("/api/v3/runGetMethod", &request);
        assert_v3_bad_request(
            status,
            &serde_json::to_value(&error).expect("RequestError must serialize"),
            expected_error,
        );
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    assertion().eq(
        pretty_json_for_snapshot(&Value::Array(summary), project.path()),
        snapbox::file!("snapshots/v3_validation_errors.json"),
    );

    node.stop();
}

#[test]
fn message_filters_and_non_wallet_errors_match_v3_contract() {
    let project = ProjectBuilder::new("localnet-v3-message-filter-validation").build();
    let node = project.localnet().start();
    let zero_hash = "00".repeat(32);
    let one_hash = "11".repeat(32);

    let mut summary = Vec::new();
    for (case, path, expected_error) in [
        (
            "transactions by message without filter",
            "/api/v3/transactionsByMessage".to_owned(),
            "at least one of msg_hash, body_hash, opcode should be specified",
        ),
        (
            "transactions by message with direction only",
            "/api/v3/transactionsByMessage?direction=in".to_owned(),
            "at least one of msg_hash, body_hash, opcode should be specified",
        ),
        (
            "pending transactions without account",
            "/api/v3/pendingTransactions".to_owned(),
            "at least 1 account address required",
        ),
        (
            "pending transactions with trace only",
            format!("/api/v3/pendingTransactions?trace_id={zero_hash}"),
            "at least 1 account address required",
        ),
    ] {
        let (status, error): (u16, responses::RequestError) = node.get_json_with_status_as(&path);
        let error_json = serde_json::to_value(&error).expect("RequestError must serialize");
        assert_v3_error(status, &error_json, 422, expected_error);
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    let repeated_hashes: responses::TransactionsResponse = node.get_json_as(&format!(
        "/api/v3/transactionsByMessage?msg_hash={zero_hash}&msg_hash={one_hash}"
    ));
    summary.push(json!({
        "case": "transactions by repeated message hashes",
        "transaction_count": repeated_hashes.transactions.len(),
        "address_book_count": repeated_hashes.address_book.len(),
    }));

    let address = test_std_addr(0x61);
    let raw_address = format!("0:{}", hex::encode(address.address.0));
    node.post_json(
        "/acton_setShardAccount",
        &json!({
            "address": raw_address,
            "shard_account": active_shard_account_boc64(
                address,
                Cell::default(),
                None,
                5_000_000_000,
            ),
        }),
    );
    let (status, error): (u16, responses::RequestError) =
        node.get_json_with_status_as(&format!("/api/v3/walletInformation?address={raw_address}"));
    let error_json = serde_json::to_value(&error).expect("RequestError must serialize");
    assert_v3_error(status, &error_json, 409, "not a wallet");
    summary.push(json!({
        "case": "active account that is not a wallet",
        "status": status,
        "code": error.code,
        "error": error.error,
    }));

    assertion().eq(
        pretty_json_for_snapshot(&Value::Array(summary), project.path()),
        snapbox::file!("snapshots/v3_message_filter_validation.json"),
    );

    node.stop();
}
