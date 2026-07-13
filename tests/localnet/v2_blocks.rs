use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::toncenter::{
    find_v2_transaction_block, jetton_v1_action_project, run_localnet_action_project,
};
use serde_json::{Value, json};
use ton_api::toncenter::v2::{StringOrNumber, requests, responses};

const SHARD: i64 = i64::MIN;
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn block_transactions_match_upstream_pagination_contract() {
    let project = jetton_v1_action_project("localnet-v2-block-transactions");
    let (node, _) = run_localnet_action_project(&project, "scripts/jetton.tolk");
    let (seqno, all) = find_v2_transaction_block(&node, 3);
    let first = &all.transactions[0];
    let after_hash = account_hash(&first.account);
    let query = block_query(0, seqno);

    let default_page: responses::TonlibResponse<responses::BlockTransactions> =
        node.get_json_as(&format!("/api/v2/getBlockTransactions?{query}&count=2"));
    let zero_cursor: responses::TonlibResponse<responses::BlockTransactions> = node.get_json_as(
        &format!("/api/v2/getBlockTransactions?{query}&count=2&after_lt=0&after_hash={ZERO_HASH}"),
    );
    let exact_cursor: responses::JsonRpcResponse<responses::BlockTransactions> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("exact".to_owned()),
            "getBlockTransactions",
            block_transactions_request(seqno, Some((&first.lt, after_hash))),
        );
    let exact_ext: responses::JsonRpcResponse<responses::BlockTransactionsExt> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("exact-ext".to_owned()),
            "getBlockTransactionsExt",
            block_transactions_request(seqno, Some((&first.lt, after_hash))),
        );
    let unknown_cursor: responses::TonlibResponse<responses::BlockTransactions> =
        node.get_json_as(&format!(
            "/api/v2/getBlockTransactions?{query}&count=2&after_lt={}&after_hash={ZERO_HASH}",
            first.lt
        ));
    let root_only: responses::TonlibResponse<responses::BlockTransactions> = node.get_json_as(
        &format!("/api/v2/getBlockTransactions?{query}&count=1&root_hash={ZERO_HASH}"),
    );
    let file_only: responses::TonlibResponse<responses::BlockTransactions> = node.get_json_as(
        &format!("/api/v2/getBlockTransactions?{query}&count=1&file_hash={ZERO_HASH}"),
    );

    let mut errors = Vec::new();
    for (case, suffix) in [
        (
            "both wrong block hashes",
            format!("root_hash={ZERO_HASH}&file_hash={ZERO_HASH}"),
        ),
        (
            "negative after_lt",
            format!("after_lt=-1&after_hash={ZERO_HASH}"),
        ),
        ("after_lt without hash", "after_lt=0".to_owned()),
        (
            "after_lt i64 overflow",
            format!("after_lt=9223372036854775808&after_hash={ZERO_HASH}"),
        ),
        ("count i32 overflow", "count=2147483648".to_owned()),
        ("seqno i32 overflow", "seqno=2147483648".to_owned()),
    ] {
        let path = if case == "seqno i32 overflow" {
            format!("/api/v2/getBlockTransactions?workchain=0&shard={SHARD}&{suffix}")
        } else {
            format!("/api/v2/getBlockTransactions?{query}&{suffix}")
        };
        let (status, error): (u16, responses::TonlibErrorResponse) =
            node.get_json_with_status_as(&path);
        errors.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }
    let mut invalid_rpc_request = block_transactions_request(seqno, None);
    invalid_rpc_request.count = Some(StringOrNumber::Unsigned(i32::MAX as u64 + 1));
    let (rpc_status, rpc_error): (u16, responses::TonlibErrorResponse) = node
        .post_v2_json_rpc_with_status(
            "/api/v2",
            StringOrNumber::String("invalid-count".to_owned()),
            "getBlockTransactions",
            invalid_rpc_request,
        );
    errors.push(json!({
        "case": "json-rpc count i32 overflow",
        "status": rpc_status,
        "code": rpc_error.code,
        "error": rpc_error.error,
    }));

    let snapshot = json!({
        "fixture": {
            "seqno": seqno,
            "transaction_count": all.transactions.len(),
        },
        "default_page": {
            "count": default_page.result.transactions.len(),
            "incomplete": default_page.result.incomplete,
            "req_count": default_page.result.req_count,
            "modes": default_page.result.transactions.iter().map(|tx| tx.mode).collect::<Vec<_>>(),
        },
        "zero_cursor_starts_at_first": transaction_hashes(&zero_cursor.result)
            == transaction_hashes(&default_page.result),
        "exact_cursor": {
            "count": exact_cursor.response.result.transactions.len(),
            "starts_after_cursor": exact_cursor.response.result.transactions.first()
                .is_some_and(|tx| tx.hash == all.transactions[1].hash),
        },
        "exact_ext_cursor": {
            "count": exact_ext.response.result.transactions.len(),
            "incomplete": exact_ext.response.result.incomplete,
            "req_count": exact_ext.response.result.req_count,
        },
        "unknown_cursor_starts_at_first": transaction_hashes(&unknown_cursor.result)
            == transaction_hashes(&default_page.result),
        "single_block_hash_is_lookup_hint": {
            "root_only_matches": root_only.result.id.root_hash == all.id.root_hash,
            "file_only_matches": file_only.result.id.file_hash == all.id.file_hash,
        },
        "errors": errors,
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_block_transactions.json"),
    );

    node.stop();
}

#[test]
fn block_lookup_and_headers_match_upstream_contract() {
    let project = jetton_v1_action_project("localnet-v2-block-lookup");
    let (node, _) = run_localnet_action_project(&project, "scripts/jetton.tolk");
    let (seqno, transactions) = find_v2_transaction_block(&node, 1);
    let query = block_query(0, seqno);
    let header: responses::TonlibResponse<responses::BlockHeader> =
        node.get_json_as(&format!("/api/v2/getBlockHeader?{query}"));
    let root_only: responses::TonlibResponse<responses::BlockHeader> = node.get_json_as(&format!(
        "/api/v2/getBlockHeader?{query}&root_hash={ZERO_HASH}"
    ));
    let by_seqno: responses::JsonRpcResponse<responses::TonBlockIdExt> = node.post_v2_json_rpc(
        "/api/v2",
        StringOrNumber::String("seqno".to_owned()),
        "lookupBlock",
        lookup_request(Some(seqno.into()), None, None),
    );
    let by_lt: responses::TonlibResponse<responses::TonBlockIdExt> = node.get_json_as(&format!(
        "/api/v2/lookupBlock?workchain=0&shard={SHARD}&lt={}",
        transactions.transactions[0].lt
    ));
    let by_time: responses::TonlibResponse<responses::TonBlockIdExt> = node.get_json_as(&format!(
        "/api/v2/lookupBlock?workchain=0&shard={SHARD}&unixtime={}",
        header.result.gen_utime
    ));

    let masterchain_query = block_query(-1, seqno);
    let masterchain_header: responses::TonlibResponse<responses::BlockHeader> =
        node.get_json_as(&format!("/api/v2/getBlockHeader?{masterchain_query}"));
    let masterchain_transactions: responses::TonlibResponse<responses::BlockTransactions> = node
        .get_json_as(&format!(
            "/api/v2/getBlockTransactions?{masterchain_query}&count=2"
        ));

    let invalid_lookup_requests = [
        ("missing selector", lookup_request(None, None, None)),
        (
            "multiple selectors",
            lookup_request(Some(seqno.into()), Some(0.into()), None),
        ),
        ("zero seqno", lookup_request(Some(0.into()), None, None)),
        ("negative lt", lookup_request(None, Some((-1).into()), None)),
        (
            "lt i64 overflow",
            lookup_request(
                None,
                Some(StringOrNumber::Unsigned(i64::MAX as u64 + 1)),
                None,
            ),
        ),
        (
            "negative unixtime",
            lookup_request(None, None, Some((-1).into())),
        ),
        (
            "unixtime i32 overflow",
            lookup_request(
                None,
                None,
                Some(StringOrNumber::Unsigned(i32::MAX as u64 + 1)),
            ),
        ),
    ];
    let mut validation = Vec::new();
    for (case, request) in invalid_lookup_requests {
        let (status, error): (u16, responses::TonlibErrorResponse) = node
            .post_v2_json_rpc_with_status(
                "/api/v2",
                StringOrNumber::String(case.to_owned()),
                "lookupBlock",
                request,
            );
        validation.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }
    for (case, selector) in [("zero lt", "lt=0"), ("zero unixtime", "unixtime=0")] {
        let (status, error): (u16, responses::TonlibErrorResponse) = node.get_json_with_status_as(
            &format!("/api/v2/lookupBlock?workchain=0&shard={SHARD}&{selector}"),
        );
        validation.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    let (both_hashes_status, both_hashes_error): (u16, responses::TonlibErrorResponse) = node
        .get_json_with_status_as(&format!(
            "/api/v2/getBlockHeader?{query}&root_hash={ZERO_HASH}&file_hash={ZERO_HASH}"
        ));
    let snapshot = json!({
        "header": header_summary(&header.result),
        "masterchain_header": header_summary(&masterchain_header.result),
        "single_hash_is_lookup_hint": root_only.result.id.root_hash == header.result.id.root_hash,
        "both_wrong_hashes": {
            "status": both_hashes_status,
            "code": both_hashes_error.code,
            "error": both_hashes_error.error,
        },
        "lookup": {
            "by_seqno": by_seqno.response.result.seqno == u64::from(seqno),
            "by_lt": by_lt.result.seqno == u64::from(seqno),
            "by_time_is_at_or_after_fixture": by_time.result.seqno >= u64::from(seqno),
        },
        "masterchain_transactions": {
            "count": masterchain_transactions.result.transactions.len(),
            "incomplete": masterchain_transactions.result.incomplete,
            "req_count": masterchain_transactions.result.req_count,
        },
        "validation": validation,
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_block_lookup_and_headers.json"),
    );

    node.stop();
}

fn block_query(workchain: i32, seqno: u32) -> String {
    format!("workchain={workchain}&shard={SHARD}&seqno={seqno}")
}

fn account_hash(account: &str) -> &str {
    account.split_once(':').map_or(account, |(_, hash)| hash)
}

fn block_transactions_request(
    seqno: u32,
    cursor: Option<(&str, &str)>,
) -> requests::BlockTransactionsRequest {
    requests::BlockTransactionsRequest {
        workchain: 0.into(),
        shard: StringOrNumber::String(SHARD.to_string()),
        seqno: seqno.into(),
        root_hash: None,
        file_hash: None,
        after_lt: cursor.map(|(lt, _)| StringOrNumber::String(lt.to_owned())),
        after_hash: cursor.map(|(_, hash)| hash.to_owned()),
        count: Some(2.into()),
    }
}

fn lookup_request(
    seqno: Option<StringOrNumber>,
    lt: Option<StringOrNumber>,
    unixtime: Option<StringOrNumber>,
) -> requests::LookupBlockRequest {
    requests::LookupBlockRequest {
        workchain: 0.into(),
        shard: StringOrNumber::String(SHARD.to_string()),
        seqno,
        lt,
        unixtime,
    }
}

fn transaction_hashes(block: &responses::BlockTransactions) -> Vec<&str> {
    block
        .transactions
        .iter()
        .map(|tx| tx.hash.as_str())
        .collect()
}

fn header_summary(header: &responses::BlockHeader) -> Value {
    json!({
        "workchain": header.id.workchain,
        "global_id": header.global_id,
        "version": header.version,
        "after_merge": header.after_merge,
        "after_split": header.after_split,
        "before_split": header.before_split,
        "want_merge": header.want_merge,
        "want_split": header.want_split,
        "validator_list_hash_short": header.validator_list_hash_short,
        "catchain_seqno": header.catchain_seqno,
        "min_ref_mc_seqno": header.min_ref_mc_seqno,
        "is_key_block": header.is_key_block,
        "prev_key_block_seqno": header.prev_key_block_seqno,
        "has_lt_range": header.start_lt.parse::<u64>().is_ok()
            && header.end_lt.parse::<u64>().is_ok(),
        "has_gen_utime": header.gen_utime > 0,
        "previous_seqnos": header.prev_blocks.iter().map(|block| block.seqno).collect::<Vec<_>>(),
    })
}
