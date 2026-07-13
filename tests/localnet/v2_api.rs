use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::bounceable_user_friendly_address;
use serde_json::{Value, json};
use ton_api::toncenter::v2::StringOrNumber;
use ton_api::toncenter::v2::{requests, responses};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

const ZERO_ADDRESS: &str = "0:0000000000000000000000000000000000000000000000000000000000000000";
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MASTERCHAIN_SHARD: i64 = i64::MIN;

#[test]
fn json_rpc_deserializes_utility_and_account_responses() {
    let project = ProjectBuilder::new("localnet-v2-json-rpc-typed-responses").build();
    let node = project.localnet().start();

    let masterchain: responses::JsonRpcResponse<responses::MasterchainInfo> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("masterchain".to_owned()),
            "getMasterchainInfo",
            requests::EmptyRequest {},
        );
    let detected_address: responses::JsonRpcResponse<responses::DetectAddress> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::Number(-7),
            "detectAddress",
            requests::AddressRequest {
                address: ZERO_ADDRESS.to_owned(),
            },
        );
    let detected_hash: responses::JsonRpcResponse<responses::DetectHash> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::Unsigned(7),
        "detectHash",
        requests::DetectHashRequest {
            hash: ZERO_HASH.to_owned(),
        },
    );
    let packed_address: responses::JsonRpcResponse<String> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("pack".to_owned()),
        "packAddress",
        requests::AddressRequest {
            address: ZERO_ADDRESS.to_owned(),
        },
    );
    let unpacked_address: responses::JsonRpcResponse<String> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("unpack".to_owned()),
        "unpackAddress",
        requests::AddressRequest {
            address: packed_address.response.result.clone(),
        },
    );
    let account: responses::JsonRpcResponse<responses::AddressInformation> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("account".to_owned()),
        "getAddressInformation",
        requests::AddressInformationRequest {
            address: ZERO_ADDRESS.to_owned(),
            seqno: None,
        },
    );
    let balance: responses::JsonRpcResponse<String> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("balance".to_owned()),
        "getAddressBalance",
        requests::AddressInformationRequest {
            address: ZERO_ADDRESS.to_owned(),
            seqno: None,
        },
    );
    let state: responses::JsonRpcResponse<String> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("state".to_owned()),
        "getAddressState",
        requests::AddressInformationRequest {
            address: ZERO_ADDRESS.to_owned(),
            seqno: None,
        },
    );
    let extended: responses::JsonRpcResponse<responses::ExtendedAddressInformation> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("extended".to_owned()),
            "getExtendedAddressInformation",
            requests::AddressInformationRequest {
                address: ZERO_ADDRESS.to_owned(),
                seqno: None,
            },
        );
    let wallet: responses::JsonRpcResponse<responses::WalletInformation> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("wallet".to_owned()),
        "getWalletInformation",
        requests::AddressInformationRequest {
            address: ZERO_ADDRESS.to_owned(),
            seqno: None,
        },
    );
    let libraries: responses::JsonRpcResponse<responses::LibraryResult> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("libraries".to_owned()),
        "getLibraries",
        requests::LibrariesRequest {
            libraries: vec![ZERO_HASH.to_owned()],
        },
    );
    let transactions: responses::JsonRpcResponse<Vec<responses::Transaction>> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("transactions".to_owned()),
            "getTransactions",
            requests::TransactionsRequest {
                address: ZERO_ADDRESS.to_owned(),
                limit: Some(StringOrNumber::Unsigned(10)),
                lt: None,
                hash: None,
                to_lt: None,
                archival: Some(false),
            },
        );
    let raw_transactions: responses::JsonRpcResponse<responses::RawTransactions> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("raw-transactions".to_owned()),
            "getTransactionsStd",
            requests::TransactionsRequest {
                address: ZERO_ADDRESS.to_owned(),
                limit: Some(StringOrNumber::String("10".to_owned())),
                lt: None,
                hash: None,
                to_lt: None,
                archival: None,
            },
        );
    let config: responses::JsonRpcResponse<responses::ConfigInfo> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("config".to_owned()),
        "getConfigAll",
        requests::ConfigAllRequest { seqno: None },
    );

    let summary = json!({
        "ids": {
            "string": masterchain.id,
            "signed": detected_address.id,
            "unsigned": detected_hash.id,
        },
        "masterchain_type": masterchain.response.result.type_field,
        "detected_address": {
            "type": detected_address.response.result.type_field,
            "raw_form": detected_address.response.result.raw_form,
            "given_type": detected_address.response.result.given_type,
            "test_only": detected_address.response.result.test_only,
        },
        "detected_hash": {
            "type": detected_hash.response.result.type_field,
            "hex": detected_hash.response.result.hex,
        },
        "address_roundtrip": unpacked_address.response.result == ZERO_ADDRESS,
        "packed_address_length": packed_address.response.result.len(),
        "account": {
            "type": account.response.result.type_field,
            "state": account.response.result.state,
            "balance": balance.response.result,
            "state_endpoint": state.response.result,
            "extended_type": extended.response.result.type_field,
            "extended_address_uses_upstream_bounceable_form":
                extended.response.result.address.account_address
                    == bounceable_user_friendly_address(ZERO_ADDRESS),
            "wallet_type": wallet.response.result.type_field,
            "is_wallet": wallet.response.result.wallet,
        },
        "library_count": libraries.response.result.result.len(),
        "transaction_count": transactions.response.result.len(),
        "raw_transaction_count": raw_transactions.response.result.transactions.len(),
        "config_type": config.response.result.type_field,
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_json_rpc_typed_responses.json"),
    );

    node.stop();
}

#[test]
fn account_and_config_queries_match_v2_validation_rules() {
    let project = ProjectBuilder::new("localnet-v2-account-validation").build();
    let node = project.localnet().start();

    let rest_state: responses::TonlibResponse<String> =
        node.get_json_as(&format!("/api/v2/getAddressState?address={ZERO_ADDRESS}"));
    let rpc_state: responses::JsonRpcResponse<String> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("state".to_owned()),
        "getAddressState",
        requests::AddressInformationRequest {
            address: ZERO_ADDRESS.to_owned(),
            seqno: None,
        },
    );

    let rest_paths = [
        (
            "address information",
            format!("/api/v2/getAddressInformation?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "address balance",
            format!("/api/v2/getAddressBalance?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "address state",
            format!("/api/v2/getAddressState?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "extended address information",
            format!("/api/v2/getExtendedAddressInformation?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "wallet information",
            format!("/api/v2/getWalletInformation?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "token data",
            format!("/api/v2/getTokenData?address={ZERO_ADDRESS}&seqno=0"),
        ),
        (
            "shard account cell",
            format!("/api/v2/getShardAccountCell?address={ZERO_ADDRESS}&seqno=0"),
        ),
        ("config all", "/api/v2/getConfigAll?seqno=0".to_owned()),
        (
            "config param",
            "/api/v2/getConfigParam?param=0&seqno=0".to_owned(),
        ),
    ];
    let mut validation = Vec::new();
    for (case, path) in rest_paths {
        let (status, error): (u16, responses::TonlibErrorResponse) =
            node.get_json_with_status_as(&path);
        validation.push(json!({
            "transport": "rest",
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    for method in [
        "getAddressInformation",
        "getAddressBalance",
        "getAddressState",
        "getExtendedAddressInformation",
        "getWalletInformation",
        "getTokenData",
        "getShardAccountCell",
    ] {
        let (status, error): (u16, responses::TonlibErrorResponse) = node
            .post_v2_json_rpc_with_status(
                "/api/v2",
                StringOrNumber::String(method.to_owned()),
                method,
                requests::AddressInformationRequest {
                    address: ZERO_ADDRESS.to_owned(),
                    seqno: Some(StringOrNumber::Number(0)),
                },
            );
        validation.push(json!({
            "transport": "json-rpc",
            "case": method,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    for (case, seqno) in [("negative", "-1"), ("i32 overflow", "2147483648")] {
        let path = format!("/api/v2/getAddressInformation?address={ZERO_ADDRESS}&seqno={seqno}");
        let (status, error): (u16, responses::TonlibErrorResponse) =
            node.get_json_with_status_as(&path);
        validation.push(json!({
            "transport": "rest",
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    let config_by_param: responses::TonlibResponse<responses::ConfigInfo> =
        node.get_json_as("/api/v2/getConfigParam?param=0");
    let config_by_id: responses::TonlibResponse<responses::ConfigInfo> =
        node.get_json_as("/api/v2/getConfigParam?config_id=0");
    for (case, path) in [
        ("missing config selector", "/api/v2/getConfigParam"),
        (
            "both config selectors",
            "/api/v2/getConfigParam?param=0&config_id=0",
        ),
    ] {
        let (status, error): (u16, responses::TonlibErrorResponse) =
            node.get_json_with_status_as(path);
        validation.push(json!({
            "transport": "rest",
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    let summary = json!({
        "missing_account_state": {
            "rest": rest_state.result,
            "json_rpc": rpc_state.response.result,
        },
        "config_aliases_match": config_by_param.result.config.bytes
            == config_by_id.result.config.bytes,
        "validation": validation,
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_account_validation.json"),
    );

    node.stop();
}

#[test]
fn rest_block_endpoints_deserialize_canonical_responses() {
    let project = ProjectBuilder::new("localnet-v2-rest-block-responses").build();
    let node = project.localnet().arg("--mine-empty-blocks").start();
    let _: Value = node.post_json("/acton_mine", &json!({}));

    let masterchain: responses::TonlibResponse<responses::MasterchainInfo> =
        node.get_json_as("/api/v2/getMasterchainInfo");
    let seqno = masterchain.result.last.seqno;
    let block_query = format!("workchain=-1&shard={MASTERCHAIN_SHARD}&seqno={seqno}");
    let header: responses::TonlibResponse<responses::BlockHeader> =
        node.get_json_as(&format!("/api/v2/getBlockHeader?{block_query}"));
    let transactions: responses::TonlibResponse<responses::BlockTransactions> =
        node.get_json_as(&format!("/api/v2/getBlockTransactions?{block_query}"));
    let transactions_ext: responses::TonlibResponse<responses::BlockTransactionsExt> =
        node.get_json_as(&format!("/api/v2/getBlockTransactionsExt?{block_query}"));
    let consensus: responses::TonlibResponse<responses::ConsensusBlock> =
        node.get_json_as("/api/v2/getConsensusBlock");
    let queue: responses::TonlibResponse<responses::OutMsgQueueSizes> =
        node.get_json_as("/api/v2/getOutMsgQueueSize");
    let shards: responses::TonlibResponse<responses::Shards> =
        node.get_json_as(&format!("/api/v2/getShards?seqno={seqno}"));
    let lookup: responses::TonlibResponse<responses::TonBlockIdExt> = node.get_json_as(&format!(
        "/api/v2/lookupBlock?workchain=-1&shard={MASTERCHAIN_SHARD}&seqno={seqno}"
    ));

    let summary = json!({
        "header": {
            "type": header.result.type_field,
            "matches_requested_block": header.result.id.seqno == seqno,
        },
        "transactions": {
            "type": transactions.result.type_field,
            "matches_requested_block": transactions.result.id.seqno == seqno,
            "count": transactions.result.transactions.len(),
            "incomplete": transactions.result.incomplete,
        },
        "transactions_ext": {
            "type": transactions_ext.result.type_field,
            "matches_short_count": transactions_ext.result.transactions.len()
                == transactions.result.transactions.len(),
            "incomplete": transactions_ext.result.incomplete,
        },
        "consensus": {
            "type": consensus.result.type_field,
            "has_timestamp": consensus.result.timestamp > 0,
        },
        "out_queue": {
            "type": queue.result.type_field,
            "shard_count": queue.result.shards.len(),
        },
        "shards": {
            "type": shards.result.type_field,
            "count": shards.result.shards.len(),
        },
        "lookup_matches_requested_block": lookup.result.seqno == seqno,
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_rest_block_responses.json"),
    );

    node.stop();
}

#[test]
fn json_rpc_returns_typed_errors_for_invalid_requests() {
    let project = ProjectBuilder::new("localnet-v2-json-rpc-errors").build();
    let node = project.localnet().start();

    let (unknown_status, unknown): (u16, responses::TonlibErrorResponse) = node
        .post_v2_json_rpc_with_status(
            "/api/v2",
            StringOrNumber::String("unknown".to_owned()),
            "methodThatDoesNotExist",
            requests::EmptyRequest {},
        );
    let (shards_status, shards): (u16, responses::TonlibErrorResponse) = node
        .post_v2_json_rpc_with_status(
            "/api/v2",
            StringOrNumber::Number(1),
            "getShards",
            requests::SeqnoRequest {
                seqno: StringOrNumber::Number(0),
            },
        );
    let (config_status, config): (u16, responses::TonlibErrorResponse) = node
        .post_v2_json_rpc_with_status(
            "/api/v2",
            StringOrNumber::Unsigned(2),
            "getConfigParam",
            requests::ConfigParamRequest {
                param: None,
                config_id: None,
                seqno: None,
            },
        );
    let (hash_status, hash): (u16, responses::TonlibErrorResponse) = node
        .post_v2_json_rpc_with_status(
            "/api/v2",
            StringOrNumber::String("hash".to_owned()),
            "detectHash",
            requests::DetectHashRequest {
                hash: "not-a-hash".to_owned(),
            },
        );

    let summary = Value::Array(vec![
        json!({
            "case": "unknown method",
            "status": unknown_status,
            "code": unknown.code,
            "error": unknown.error,
            "id": unknown.id,
        }),
        json!({
            "case": "zero shards seqno",
            "status": shards_status,
            "code": shards.code,
            "error": shards.error,
            "id": shards.id,
        }),
        json!({
            "case": "missing config param",
            "status": config_status,
            "code": config.code,
            "error": config.error,
            "id": config.id,
        }),
        json!({
            "case": "invalid hash",
            "status": hash_status,
            "code": hash.code,
            "error": hash.error,
            "id": hash.id,
        }),
    ]);

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_json_rpc_errors.json"),
    );

    node.stop();
}

#[test]
fn json_rpc_matches_upstream_proxy_envelope() {
    let project = ProjectBuilder::new("localnet-v2-json-rpc-envelope").build();
    let node = project.localnet().arg("--mine-empty-blocks").start();
    let _: Value = node.post_json("/acton_mine", &json!({}));
    let requests = [
        ("absent", json!({"method": "getMasterchainInfo"})),
        (
            "empty array",
            json!({"method": "getMasterchainInfo", "params": []}),
        ),
        (
            "null",
            json!({"method": "getMasterchainInfo", "params": null}),
        ),
        (
            "scalar and ignored metadata",
            json!({
                "jsonrpc": 2,
                "id": {"ignored": true},
                "method": "getMasterchainInfo",
                "params": "ignored"
            }),
        ),
    ];
    let mut normalized_params = Vec::new();
    let mut masterchain_seqno = None;
    for (case, request) in requests {
        let (status, json) = node.post_json_with_status("/api/v2", &request);
        let response: responses::TonlibResponse<responses::MasterchainInfo> =
            serde_json::from_value(json.clone()).expect("masterchain response must be typed");
        masterchain_seqno = Some(response.result.last.seqno);
        normalized_params.push(json!({
            "case": case,
            "status": status,
            "ok": response.ok,
            "metadata_omitted": json.get("jsonrpc").is_none() && json.get("id").is_none(),
        }));
    }

    let (shards_status, shards_json) = node.post_json_with_status(
        "/api/v2",
        &json!({
            "method": "getShards",
            "params": {"seqno": masterchain_seqno.expect("masterchain seqno")}
        }),
    );
    let shards: responses::TonlibResponse<responses::Shards> =
        serde_json::from_value(shards_json).expect("getShards response must be typed");
    let (array_status, array_error): (u16, responses::TonlibErrorResponse) = node
        .post_json_with_status_as(
            "/api/v2",
            &json!({"method": "getMasterchainInfo", "params": [{}]}),
        );
    let (alias_status, alias_error): (u16, responses::TonlibErrorResponse) = node
        .post_json_with_status_as(
            "/api/v2",
            &json!({
                "method": "shards",
                "params": {"seqno": masterchain_seqno.expect("masterchain seqno")}
            }),
        );

    let summary = json!({
        "normalized_params": normalized_params,
        "get_shards": {
            "status": shards_status,
            "ok": shards.ok,
            "count": shards.result.shards.len(),
        },
        "nonempty_array": {
            "status": array_status,
            "code": array_error.code,
            "error": array_error.error,
        },
        "non_upstream_alias": {
            "status": alias_status,
            "code": alias_error.code,
            "error": alias_error.error,
        },
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_json_rpc_envelope.json"),
    );

    node.stop();
}

#[test]
fn run_get_method_std_uses_canonical_stack_contract() {
    let project = ProjectBuilder::new("localnet-v2-run-get-method-std").build();
    let node = project.localnet().start();
    let empty_cell = Boc::encode_base64(Cell::default());
    let request_json = json!({
        "address": ZERO_ADDRESS,
        "method": 1,
        "stack": [
            {
                "@type": "tvm.stackEntryNumber",
                "number": {"@type": "tvm.numberDecimal", "number": "7"}
            },
            {
                "@type": "tvm.stackEntryCell",
                "cell": {"@type": "tvm.cell", "bytes": empty_cell}
            },
            {
                "@type": "tvm.stackEntrySlice",
                "slice": {"@type": "tvm.slice", "bytes": empty_cell}
            },
            {
                "@type": "tvm.stackEntryTuple",
                "tuple": {
                    "@type": "tvm.tuple",
                    "elements": [{
                        "@type": "tvm.stackEntryNumber",
                        "number": {"@type": "tvm.numberDecimal", "number": "8"}
                    }]
                }
            },
            {
                "@type": "tvm.stackEntryList",
                "list": {
                    "@type": "tvm.list",
                    "elements": [{
                        "@type": "tvm.stackEntryNumber",
                        "number": {"@type": "tvm.numberDecimal", "number": "9"}
                    }]
                }
            }
        ]
    });
    let request: requests::RunGetMethodStdRequest = serde_json::from_value(request_json.clone())
        .expect("canonical Std request must deserialize");

    let (rest_status, rest_json) =
        node.post_json_with_status("/api/v2/runGetMethodStd", &request_json);
    let rest: responses::TonlibResponse<responses::RunGetMethodStdResult> =
        serde_json::from_value(rest_json.clone()).expect("Std REST response must be typed");
    let rpc: responses::JsonRpcResponse<responses::RunGetMethodStdResult> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("std".to_owned()),
        "runGetMethodStd",
        request,
    );
    let (legacy_status, _) = node.post_json_raw_with_status(
        "/api/v2/runGetMethodStd",
        &json!({
            "address": ZERO_ADDRESS,
            "method": 1,
            "stack": [["num", "7"]]
        }),
    );
    let mut invalid_marker_request = request_json.clone();
    invalid_marker_request["stack"][0]["number"]["@type"] = json!("wrong");
    let (invalid_marker_status, _) =
        node.post_json_raw_with_status("/api/v2/runGetMethodStd", &invalid_marker_request);
    let mut unsupported_request = request_json.clone();
    unsupported_request["stack"] = json!([{"@type": "tvm.stackEntryUnsupported"}]);
    let (unsupported_status, _) =
        node.post_json_raw_with_status("/api/v2/runGetMethodStd", &unsupported_request);
    let mut negative_seqno_request = request_json.clone();
    negative_seqno_request["seqno"] = json!(-1);
    let (negative_seqno_status, _) =
        node.post_json_raw_with_status("/api/v2/runGetMethodStd", &negative_seqno_request);
    let mut unknown_field_request = request_json.clone();
    unknown_field_request["unexpected"] = json!(true);
    let (unknown_field_status, _) =
        node.post_json_raw_with_status("/api/v2/runGetMethodStd", &unknown_field_request);
    let mut out_of_range_method_request = request_json;
    out_of_range_method_request["method"] = json!(i64::from(i32::MAX) + 1);
    let (out_of_range_method_status, _) =
        node.post_json_raw_with_status("/api/v2/runGetMethodStd", &out_of_range_method_request);
    let (rpc_invalid_marker_status, _) = node.post_json_raw_with_status(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": "invalid-marker",
            "method": "runGetMethodStd",
            "params": invalid_marker_request,
        }),
    );

    let summary = json!({
        "rest_status": rest_status,
        "rest_type": rest.result.type_field,
        "rest_exit_code": rest.result.exit_code,
        "rest_stack_len": rest.result.stack.len(),
        "rpc_type": rpc.response.result.type_field,
        "rpc_exit_code": rpc.response.result.exit_code,
        "rpc_stack_len": rpc.response.result.stack.len(),
        "std_shape_omits_legacy_context": rest_json["result"].get("block_id").is_none()
            && rest_json["result"].get("last_transaction_id").is_none()
            && rest_json["result"].get("vm_log").is_none(),
        "legacy_stack_rejected": legacy_status,
        "invalid_nested_marker_rejected": invalid_marker_status >= 400,
        "unsupported_entry_rejected": unsupported_status >= 400,
        "negative_seqno_rejected": negative_seqno_status >= 400,
        "unknown_field_rejected": unknown_field_status >= 400,
        "out_of_range_numeric_method_rejected": out_of_range_method_status >= 400,
        "json_rpc_invalid_marker_rejected": rpc_invalid_marker_status >= 400,
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v2_run_get_method_std.json"),
    );

    node.stop();
}
