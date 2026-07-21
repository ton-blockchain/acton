use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{
    build_internal_message_boc_with_currency_and_body, build_text_comment_body,
    find_v2_internal_message_by_hash, test_std_addr,
};
use base64::Engine as _;
use serde_json::{Value, json};
use ton_api::toncenter::v2::{requests as v2_requests, responses as v2_responses};
use ton_api::toncenter::v3::responses as v3_responses;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;
use tycho_types::models::{CurrencyCollection, ExtraCurrencyCollection};
use tycho_types::num::VarUint248;

const TEXT_COMMENT: &str = "typed message with extra currencies";
const LARGE_EXTRA_CURRENCY: &str = "340282366920938463463374607431768211456";

#[test]
fn transaction_messages_match_decoded_and_raw_upstream_dtos() {
    let project = ProjectBuilder::new("localnet-v2-transaction-messages").build();
    let node = project.localnet().start();
    let source = test_std_addr(0x11);
    let destination = test_std_addr(0x22);
    let destination_raw = format!("0:{}", hex::encode([0x22; 32]));

    let text_body = build_text_comment_body(&["typed message ", "with extra currencies"]);
    let text_body_b64 = base64::engine::general_purpose::STANDARD.encode(Boc::encode(&text_body));
    let mut text_value = CurrencyCollection::new(50_000_000);
    text_value.other = ExtraCurrencyCollection::try_from_iter([
        (7, VarUint248::new(123)),
        (u32::MAX, VarUint248::from_words(1, 0)),
    ])
    .expect("extra currencies must encode");
    let text_send: v2_responses::TonlibResponse<v2_responses::InternalMessageInfo> = node
        .post_json_as(
            "/acton_sendInternalMessage",
            &v2_requests::SendBocRequest {
                boc: base64::engine::general_purpose::STANDARD.encode(
                    build_internal_message_boc_with_currency_and_body(
                        source,
                        destination.clone(),
                        text_value,
                        text_body,
                    ),
                ),
            },
        );

    let mut binary_body = CellBuilder::new();
    binary_body
        .store_u32(0xdead_beef)
        .expect("binary opcode must store");
    binary_body
        .store_uint(0b10101, 5)
        .expect("partial binary byte must store");
    let binary_body = binary_body.build().expect("binary body must build");
    let binary_body_b64 =
        base64::engine::general_purpose::STANDARD.encode(Boc::encode(&binary_body));
    let binary_send: v2_responses::TonlibResponse<v2_responses::InternalMessageInfo> = node
        .post_json_as(
            "/acton_sendInternalMessage",
            &v2_requests::SendBocRequest {
                boc: base64::engine::general_purpose::STANDARD.encode(
                    build_internal_message_boc_with_currency_and_body(
                        test_std_addr(0x33),
                        destination,
                        CurrencyCollection::new(50_000_000),
                        binary_body,
                    ),
                ),
            },
        );

    let (text_block_tx, text_block_message) =
        find_v2_internal_message_by_hash(&node, &text_send.result.hash);
    let (binary_block_tx, binary_block_message) =
        find_v2_internal_message_by_hash(&node, &binary_send.result.hash);
    let decoded: v2_responses::TonlibResponse<Vec<v2_responses::Transaction>> = node.get_json_as(
        &format!("/api/v2/getTransactions?address={destination_raw}&limit=100"),
    );
    let raw: v2_responses::TonlibResponse<v2_responses::RawTransactions> = node.get_json_as(
        &format!("/api/v2/getTransactionsStd?address={destination_raw}&limit=100"),
    );

    let text_decoded_tx = decoded_transaction(&decoded.result, &text_block_tx.transaction_id.hash);
    let binary_decoded_tx =
        decoded_transaction(&decoded.result, &binary_block_tx.transaction_id.hash);
    let text_decoded = full_in_message(text_decoded_tx);
    let binary_decoded = full_in_message(binary_decoded_tx);
    let text_raw = raw_in_message(&raw.result, &text_block_tx.transaction_id.hash);
    let binary_raw = raw_in_message(&raw.result, &binary_block_tx.transaction_id.hash);

    let text_located: v2_responses::TonlibResponse<v2_responses::Transaction> =
        node.get_json_as(&format!(
            "/api/v2/tryLocateTx?source={}&destination={}&created_lt={}",
            text_block_message.source.account_address,
            text_block_message.destination.account_address,
            text_block_message.created_lt,
        ));
    let text_located = full_in_message(&text_located.result);

    let v3: v3_responses::TransactionsResponse = node.get_json_as(&format!(
        "/api/v3/transactions?account={destination_raw}&limit=100"
    ));
    let text_v3 = v3
        .transactions
        .iter()
        .find(|transaction| transaction.hash == text_block_tx.transaction_id.hash)
        .and_then(|transaction| transaction.in_msg.as_ref())
        .expect("v3 text transaction must contain its incoming message");

    let snapshot = json!({
        "send": {
            "text_ok": text_send.ok,
            "text_type": text_send.result.type_field,
            "binary_ok": binary_send.ok,
            "binary_type": binary_send.result.type_field,
        },
        "text": {
            "decoded_history": decoded_message_snapshot(text_decoded),
            "decoded_locate": decoded_message_snapshot(text_located),
            "std": raw_message_snapshot(text_raw, &text_body_b64),
            "block_ext": raw_message_snapshot(&text_block_message, &text_body_b64),
            "v3": {
                "body_matches": text_v3.message_content.as_ref()
                    .and_then(|content| content.body.as_deref()) == Some(text_body_b64.as_str()),
                "small_extra_currency": text_v3.value_extra_currencies.as_ref()
                    .and_then(|currencies| currencies.get("7")).map(String::as_str),
                "large_extra_currency": text_v3.value_extra_currencies.as_ref()
                    .and_then(|currencies| currencies.get(&u32::MAX.to_string())).map(String::as_str),
            },
        },
        "binary": {
            "decoded_history": decoded_message_snapshot(binary_decoded),
            "std": raw_message_snapshot(binary_raw, &binary_body_b64),
            "block_ext": raw_message_snapshot(&binary_block_message, &binary_body_b64),
        },
        "fixture": {
            "decoded_count": decoded.result.len(),
            "raw_count": raw.result.transactions.len(),
            "v3_count": v3.transactions.len(),
            "large_extra_currency": LARGE_EXTRA_CURRENCY,
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_transaction_messages.json"),
    );

    node.stop();
}

fn decoded_transaction<'a>(
    transactions: &'a [v2_responses::Transaction],
    hash: &str,
) -> &'a v2_responses::Transaction {
    transactions
        .iter()
        .find(|transaction| transaction.transaction_id.hash == hash)
        .expect("decoded transaction must be present")
}

fn full_in_message(transaction: &v2_responses::Transaction) -> &v2_responses::MessageFull {
    match transaction.in_msg.as_ref() {
        Some(v2_responses::Message::Full(message)) => message,
        Some(v2_responses::Message::Empty) | None => {
            panic!("transaction must contain a full incoming message")
        }
    }
}

fn raw_in_message<'a>(
    transactions: &'a v2_responses::RawTransactions,
    hash: &str,
) -> &'a v2_responses::MessageStd {
    transactions
        .transactions
        .iter()
        .find(|transaction| transaction.transaction_id.hash == hash)
        .and_then(|transaction| transaction.in_msg.as_ref())
        .expect("raw transaction must contain its incoming message")
}

fn decoded_message_snapshot(message: &v2_responses::MessageFull) -> Value {
    let (data, legacy_message) = match &message.msg_data {
        v2_responses::MessageData::Text { text } => (
            json!({
                "type": "text",
                "decoded": base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok()),
            }),
            json!({"text_matches": message.message.as_deref() == Some(TEXT_COMMENT)}),
        ),
        v2_responses::MessageData::Raw { body, init_state } => (
            json!({
                "type": "raw",
                "body_is_boc": Boc::decode_base64(body).is_ok(),
                "init_state_is_empty": init_state.is_empty(),
            }),
            json!({"binary_matches": message.message.as_deref() == Some("3q2+76g=\n")}),
        ),
        v2_responses::MessageData::DecryptedText { .. } => (
            json!({"type": "decrypted"}),
            json!({"classified": "decrypted"}),
        ),
        v2_responses::MessageData::EncryptedText { .. } => (
            json!({"type": "encrypted"}),
            json!({"classified": "encrypted"}),
        ),
    };
    json!({
        "data": data,
        "legacy_message": legacy_message,
        "decode_error": message.message_decode_error,
        "extra_currencies": message.extra_currencies,
    })
}

fn raw_message_snapshot(message: &v2_responses::MessageStd, expected_body: &str) -> Value {
    let (body_matches, init_state_is_empty) = match &message.msg_data {
        v2_responses::MessageData::Raw { body, init_state } => {
            (body == expected_body, init_state.is_empty())
        }
        _ => (false, false),
    };
    json!({
        "type": message.type_field,
        "body_matches": body_matches,
        "init_state_is_empty": init_state_is_empty,
        "extra_currencies": message.extra_currencies,
    })
}
