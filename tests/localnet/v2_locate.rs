use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::toncenter::{
    find_v2_internal_message, jetton_v1_action_project, run_localnet_action_project,
};
use serde_json::json;
use ton_api::toncenter::v2::{StringOrNumber, requests, responses};

#[test]
fn transaction_lookup_matches_upstream_contract() {
    let project = jetton_v1_action_project("localnet-v2-transaction-lookup");
    let (node, _) = run_localnet_action_project(&project, "scripts/jetton.tolk");
    let (destination_transaction, message) = find_v2_internal_message(&node);
    let source = &message.source.account_address;
    let destination = &message.destination.account_address;
    let created_lt = message
        .created_lt
        .parse::<u64>()
        .expect("message created_lt must fit u64");
    let request = requests::TryLocateTxRequest {
        source: source.clone(),
        destination: destination.clone(),
        created_lt: StringOrNumber::Unsigned(created_lt),
    };

    let alias: responses::TonlibResponse<responses::Transaction> = node.get_json_as(&format!(
        "/api/v2/tryLocateTx?source={source}&destination={destination}&created_lt={created_lt}"
    ));
    let result: responses::JsonRpcResponse<responses::Transaction> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("result".to_owned()),
        "tryLocateResultTx",
        request.clone(),
    );
    let source_rest: responses::TonlibResponse<responses::Transaction> = node.get_json_as(
        &format!(
            "/api/v2/tryLocateSourceTx?source={source}&destination={destination}&created_lt={created_lt}"
        ),
    );
    let source_rpc: responses::JsonRpcResponse<responses::Transaction> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("source".to_owned()),
        "tryLocateSourceTx",
        request.clone(),
    );

    let missing_lt = created_lt + 1;
    let mut errors = Vec::new();
    for method in ["tryLocateTx", "tryLocateResultTx", "tryLocateSourceTx"] {
        let (status, error): (u16, responses::TonlibErrorResponse) =
            node.get_json_with_status_as(&format!(
                "/api/v2/{method}?source={source}&destination={destination}&created_lt={missing_lt}"
            ));
        errors.push(json!({
            "method": method,
            "case": "missing REST transaction",
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    for (case, invalid_request) in [
        (
            "negative created_lt",
            requests::TryLocateTxRequest {
                created_lt: StringOrNumber::Number(-1),
                ..request.clone()
            },
        ),
        (
            "created_lt int64 overflow",
            requests::TryLocateTxRequest {
                created_lt: StringOrNumber::Unsigned(i64::MAX as u64 + 1),
                ..request.clone()
            },
        ),
        (
            "invalid source",
            requests::TryLocateTxRequest {
                source: "not-an-address".to_owned(),
                ..request.clone()
            },
        ),
        (
            "empty destination",
            requests::TryLocateTxRequest {
                destination: String::new(),
                ..request
            },
        ),
    ] {
        let (status, error): (u16, responses::TonlibErrorResponse) = node
            .post_v2_json_rpc_with_status(
                "/api/v2",
                StringOrNumber::String(case.to_owned()),
                "tryLocateResultTx",
                invalid_request,
            );
        errors.push(json!({
            "method": "tryLocateResultTx",
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    let source_contains_message = source_rest.result.out_msgs.iter().any(|candidate| {
        matches!(candidate, responses::Message::Full(full) if full.hash == message.hash)
    });
    let snapshot = json!({
        "fixture": {
            "has_internal_source": !source.is_empty(),
            "has_internal_destination": !destination.is_empty(),
            "created_lt_is_positive": created_lt > 0,
        },
        "destination_lookup": {
            "alias_matches_incoming_transaction": alias.result.transaction_id.hash
                == destination_transaction.transaction_id.hash,
            "result_matches_incoming_transaction": result.response.result.transaction_id.hash
                == destination_transaction.transaction_id.hash,
            "alias_matches_result": alias.result.transaction_id.hash
                == result.response.result.transaction_id.hash,
        },
        "source_lookup": {
            "rest_and_rpc_match": source_rest.result.transaction_id.hash
                == source_rpc.response.result.transaction_id.hash,
            "belongs_to_source": source_rest.result.account == *source,
            "contains_located_message": source_contains_message,
        },
        "errors": errors,
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_transaction_lookup.json"),
    );

    node.stop();
}
