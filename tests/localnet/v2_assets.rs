use crate::common::assertion;
use crate::support::localnet::{LocalnetHandle, pretty_json_for_snapshot};
use crate::support::toncenter::{
    extract_canonical_addr_marker, jetton_v1_action_project, nft_v1_action_project,
    run_localnet_action_project,
};
use serde_json::{Value, json};
use ton_api::toncenter::v2::{StringOrNumber, requests, responses};
use ton_api::toncenter::v3;

const NO_STATE_ADDRESS: &str = "0:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn get_token_data_reads_real_historical_jetton_states() {
    let project = jetton_v1_action_project("localnet-v2-real-jetton-token-data");
    let (node, output) = run_localnet_action_project(&project, "scripts/jetton.tolk");
    let owner = extract_canonical_addr_marker(&output, "OWNER=");
    let master = extract_canonical_addr_marker(&output, "JETTON_MASTER=");
    let wallet = extract_canonical_addr_marker(&output, "JETTON_SOURCE_WALLET=");

    let master_seqno = first_transaction_seqno(&node, &master);
    let wallet_seqno = first_transaction_seqno(&node, &wallet);
    let historical_master = get_token_data(&node, &master, Some(master_seqno));
    let historical_master_rpc: responses::JsonRpcResponse<responses::TokenData> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("historical-master".to_owned()),
            "getTokenData",
            token_request(&master, Some(master_seqno)),
        );
    let current_master = get_token_data(&node, &master, None);
    let historical_wallet = get_token_data(&node, &wallet, Some(wallet_seqno));
    let current_wallet = get_token_data(&node, &wallet, None);
    let (non_token_status, non_token) =
        node.get_json_with_status(&format!("/api/v2/getTokenData?address={owner}"));
    let (no_state_status, no_state) =
        node.get_json_with_status(&format!("/api/v2/getTokenData?address={NO_STATE_ADDRESS}"));

    let snapshot = json!({
        "master": {
            "first_transaction_seqno": master_seqno,
            "historical_rest": token_summary(&historical_master.result),
            "historical_json_rpc": token_summary(&historical_master_rpc.response.result),
            "current": token_summary(&current_master.result),
        },
        "wallet": {
            "first_transaction_seqno": wallet_seqno,
            "historical": token_summary(&historical_wallet.result),
            "current": token_summary(&current_wallet.result),
        },
        "non_token": {
            "status": non_token_status,
            "code": non_token["code"],
        },
        "no_state": {
            "status": no_state_status,
            "code": no_state["code"],
        },
    });
    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/v2_real_jetton_token_data.json"),
    );

    node.stop();
}

#[test]
fn get_token_data_detects_real_empty_collection_and_historical_nft_owner() {
    let project = nft_v1_action_project("localnet-v2-real-nft-token-data");
    let (node, output) = run_localnet_action_project(&project, "scripts/nft.tolk");
    let collection = extract_canonical_addr_marker(&output, "NFT_COLLECTION=");
    let item = extract_canonical_addr_marker(&output, "NFT_ITEM=");

    let collection_seqno = first_transaction_seqno(&node, &collection);
    let item_seqno = first_transaction_seqno(&node, &item);
    let historical_collection = get_token_data(&node, &collection, Some(collection_seqno));
    let historical_collection_rpc: responses::JsonRpcResponse<responses::TokenData> = node
        .post_v2_json_rpc(
            "/api/v2",
            StringOrNumber::String("empty-collection".to_owned()),
            "getTokenData",
            token_request(&collection, Some(collection_seqno)),
        );
    let current_collection = get_token_data(&node, &collection, None);
    let historical_item = get_token_data(&node, &item, Some(item_seqno));
    let current_item = get_token_data(&node, &item, None);

    let snapshot = json!({
        "collection": {
            "deploy_seqno": collection_seqno,
            "historical_rest": token_summary(&historical_collection.result),
            "historical_json_rpc": token_summary(&historical_collection_rpc.response.result),
            "current": token_summary(&current_collection.result),
        },
        "item": {
            "deploy_seqno": item_seqno,
            "historical": token_summary(&historical_item.result),
            "current": token_summary(&current_item.result),
        },
    });
    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/v2_real_nft_token_data.json"),
    );

    node.stop();
}

fn get_token_data(
    node: &LocalnetHandle,
    address: &str,
    seqno: Option<u32>,
) -> responses::TonlibResponse<responses::TokenData> {
    let seqno = seqno.map_or_else(String::new, |seqno| format!("&seqno={seqno}"));
    serde_json::from_value(node.get_json(&format!("/api/v2/getTokenData?address={address}{seqno}")))
        .expect("getTokenData response must match the typed V2 contract")
}

fn token_request(address: &str, seqno: Option<u32>) -> requests::AddressInformationRequest {
    requests::AddressInformationRequest {
        address: address.to_owned(),
        seqno: seqno.map(|seqno| StringOrNumber::Unsigned(u64::from(seqno))),
    }
}

fn first_transaction_seqno(node: &LocalnetHandle, address: &str) -> u32 {
    let response: v3::TransactionsResponse = serde_json::from_value(node.get_json(&format!(
        "/api/v3/transactions?account={address}&sort=asc&limit=100"
    )))
    .expect("transactions response must match the typed V3 contract");
    response
        .transactions
        .first()
        .unwrap_or_else(|| panic!("No transactions indexed for {address}"))
        .mc_block_seqno
}

fn token_summary(token: &responses::TokenData) -> Value {
    let token = serde_json::to_value(token).expect("typed token data must serialize");
    let content = token
        .get("content")
        .or_else(|| token.get("collection_content"))
        .or_else(|| token.get("jetton_content"));
    json!({
        "type": token["@type"],
        "contract_type": token["contract_type"],
        "total_supply": token.get("total_supply"),
        "balance": token.get("balance"),
        "next_item_index": token.get("next_item_index"),
        "owner": token.get("owner"),
        "owner_address": token.get("owner_address"),
        "collection_address": token.get("collection_address"),
        "mintless_is_claimed": token.get("mintless_is_claimed"),
        "wallet_code_present": token
            .get("jetton_wallet_code")
            .and_then(Value::as_str)
            .is_some_and(|code| !code.is_empty()),
        "content_type": content.and_then(|content| content.get("type")),
        "domain": content.and_then(|content| content.get("domain")),
    })
}
