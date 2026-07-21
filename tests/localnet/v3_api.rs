use crate::common::assertion;
use crate::support::localnet::{assert_v3_bad_request, assert_v3_error, pretty_json_for_snapshot};
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{active_shard_account_boc64, test_std_addr};
use serde_json::{Value, json};
use std::time::Duration;
use ton_api::toncenter::v3::{requests, responses};
use ton_localnet::types::Hash256;
use tycho_types::cell::Cell;

const ZERO_ADDRESS: &str = "0:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn information_endpoints_honor_use_v2_projection() {
    let project = ProjectBuilder::new("localnet-v3-information-source").build();
    let node = project.localnet().start();
    let uninit = "0:1111111111111111111111111111111111111111111111111111111111111111";
    let frozen = "0:2222222222222222222222222222222222222222222222222222222222222222";
    let destroyed = "0:3333333333333333333333333333333333333333333333333333333333333333";
    let active_address = test_std_addr(0x44);
    let active = format!("0:{}", hex::encode(active_address.address.0));

    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": uninit,
            "state": { "type": "uninit", "balance": "100" },
        }),
    );
    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": frozen,
            "state": {
                "type": "frozen",
                "frozen_hash": hex::encode([0x77; 32]),
                "balance": "200",
            },
        }),
    );
    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": destroyed,
            "state": { "type": "uninit", "balance": "300" },
        }),
    );
    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": destroyed,
            "state": { "type": "nonexist" },
        }),
    );
    node.post_json(
        "/acton_setShardAccount",
        &json!({
            "address": active,
            "shard_account": active_shard_account_boc64(
                active_address,
                Cell::default(),
                None,
                400,
            ),
        }),
    );

    let mut address_information = Vec::new();
    for (case, address) in [
        ("missing", ZERO_ADDRESS),
        ("uninit", uninit),
        ("frozen", frozen),
        ("destroyed", destroyed),
    ] {
        let indexed: responses::V2AddressInformation = node.get_json_as(&format!(
            "/api/v3/addressInformation?address={address}&use_v2=false"
        ));
        let legacy: responses::V2AddressInformation = node.get_json_as(&format!(
            "/api/v3/addressInformation?address={address}&use_v2=true"
        ));
        address_information.push(json!({
            "case": case,
            "indexed": {
                "status": indexed.status,
                "frozen_hash": indexed.frozen_hash,
            },
            "legacy": {
                "status": legacy.status,
                "frozen_hash": legacy.frozen_hash,
            },
        }));
    }
    let default_address: responses::V2AddressInformation = node.get_json_as(&format!(
        "/api/v3/addressInformation?address={ZERO_ADDRESS}"
    ));

    let mut wallet_information = Vec::new();
    for (case, address) in [
        ("missing", ZERO_ADDRESS),
        ("uninit", uninit),
        ("frozen", frozen),
        ("destroyed", destroyed),
        ("active non-wallet", active.as_str()),
    ] {
        let mut projections = serde_json::Map::new();
        for (projection, use_v2) in [("indexed", false), ("legacy", true)] {
            let path = format!("/api/v3/walletInformation?address={address}&use_v2={use_v2}");
            let (status, body): (u16, Value) = node.get_json_with_status_as(&path);
            let body = if status == 200 {
                let info: responses::V2WalletInformation =
                    serde_json::from_value(body).expect("successful wallet response must be typed");
                json!({ "status": info.status, "wallet_type": info.wallet_type })
            } else {
                let error: responses::RequestError =
                    serde_json::from_value(body).expect("wallet error response must be typed");
                json!({ "code": error.code, "error": error.error })
            };
            projections.insert(
                projection.to_owned(),
                json!({
                    "http_status": status,
                    "body": body,
                }),
            );
        }
        wallet_information.push(json!({
            "case": case,
            "projections": projections,
        }));
    }
    let default_wallet: responses::V2WalletInformation =
        node.get_json_as(&format!("/api/v3/walletInformation?address={ZERO_ADDRESS}"));

    let summary = json!({
        "default_projection": {
            "address_status": default_address.status,
            "wallet_status": default_wallet.status,
        },
        "address_information": address_information,
        "wallet_information": wallet_information,
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v3_information_source.json"),
    );

    node.stop();
}

#[test]
fn trace_extras_follow_filtering_and_pagination() {
    let project = ProjectBuilder::new("localnet-v3-trace-page-extras").build();
    let node = project.localnet().start();
    let first_address = "0:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second_address = "0:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    for address in [first_address, second_address] {
        node.post_json(
            "/acton_fundAccount",
            &json!({
                "address": address,
                "amount": 1_000_000_000u64,
            }),
        );
    }

    let first_transactions: responses::TransactionsResponse =
        serde_json::from_value(node.wait_for_non_empty_v3_transactions(
            &format!("/api/v3/transactions?account={first_address}&limit=1&sort=desc"),
            Duration::from_secs(8),
        ))
        .expect("first faucet transaction response must be typed");
    let second_transactions: responses::TransactionsResponse =
        serde_json::from_value(node.wait_for_non_empty_v3_transactions(
            &format!("/api/v3/transactions?account={second_address}&limit=1&sort=desc"),
            Duration::from_secs(8),
        ))
        .expect("second faucet transaction response must be typed");
    let first_transaction = first_transactions
        .transactions
        .first()
        .expect("first faucet request must create a transaction");
    let second_transaction = second_transactions
        .transactions
        .first()
        .expect("second faucet request must create a transaction");
    let first_hash = Hash256::from_base64(&first_transaction.hash)
        .expect("transaction hash must be base64")
        .to_hex();
    let second_hash = Hash256::from_base64(&second_transaction.hash)
        .expect("transaction hash must be base64")
        .to_hex();
    let base_query = format!("trace_id={first_hash}&trace_id={second_hash}&sort=asc");

    let first_page: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/traces?{base_query}&limit=1"));
    let second_page: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/traces?{base_query}&limit=1&offset=1"));
    let filtered: responses::TracesResponse = node.get_json_as(&format!(
        "/api/v3/traces?{base_query}&start_lt={}&limit=10",
        second_transaction.lt
    ));

    let summarize = |response: responses::TracesResponse| {
        let mut transaction_accounts = response
            .traces
            .iter()
            .flat_map(|trace| trace.transactions.values())
            .map(|transaction| transaction.account.clone())
            .collect::<Vec<_>>();
        transaction_accounts.sort_unstable();
        transaction_accounts.dedup();
        let mut address_book = response.address_book.into_keys().collect::<Vec<_>>();
        address_book.sort_unstable();
        let mut metadata = response.metadata.into_keys().collect::<Vec<_>>();
        metadata.sort_unstable();
        json!({
            "trace_count": response.traces.len(),
            "transaction_accounts": transaction_accounts,
            "address_book": address_book,
            "metadata": metadata,
        })
    };
    let summary = json!({
        "first_page": summarize(first_page),
        "second_page": summarize(second_page),
        "start_lt_filtered": summarize(filtered),
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v3_trace_page_extras.json"),
    );

    node.stop();
}

#[test]
fn address_book_and_metadata_accept_mixed_address_batches() {
    let project = ProjectBuilder::new("localnet-v3-permissive-address-batches").build();
    let node = project.localnet().start();
    let invalid = "not-an-address";

    let address_book: responses::AddressBook = node.get_json_as(&format!(
        "/api/v3/addressBook?address={ZERO_ADDRESS}&address={invalid}"
    ));
    let metadata: responses::Metadata = node.get_json_as(&format!(
        "/api/v3/metadata?address={invalid}&address={ZERO_ADDRESS}"
    ));
    let valid_row = address_book
        .get(ZERO_ADDRESS)
        .expect("valid address must remain in the address book");
    let invalid_row = address_book
        .get(invalid)
        .expect("invalid requested key must remain in the address book");
    let mut address_book_keys = address_book.keys().collect::<Vec<_>>();
    address_book_keys.sort_unstable();
    let summary = json!({
        "address_book_keys": address_book_keys,
        "invalid": invalid_row,
        "valid": {
            "has_user_friendly": valid_row.user_friendly.is_some(),
            "domain": &valid_row.domain,
            "interfaces": &valid_row.interfaces,
        },
        "metadata": metadata,
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v3_permissive_address_batches.json"),
    );

    node.stop();
}

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
