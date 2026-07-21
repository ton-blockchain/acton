use crate::common::assertion;
use crate::support::localnet::{pretty_json_for_snapshot, response_payload};
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{
    TON_CONNECT_WALLETS_CONFIG, bounceable_user_friendly_address,
    build_internal_message_boc_with_currency_and_body, find_v2_internal_message_by_hash,
    summarize_v2_account_state, test_std_addr, v2_extra_currencies,
};
use base64::Engine as _;
use serde_json::json;
use std::fs;
use ton_api::toncenter::v2::{requests as v2_requests, responses as v2_responses};
use ton_api::toncenter::v3::responses as v3_responses;
use tycho_types::cell::{Cell, CellFamily};
use tycho_types::models::{CurrencyCollection, ExtraCurrencyCollection};
use tycho_types::num::VarUint248;

#[test]
fn wallet_and_extended_account_information_match_upstream_shapes() {
    let project = ProjectBuilder::new("localnet-v2-account-information").build();
    fs::write(
        project.path().join("wallets.toml"),
        TON_CONNECT_WALLETS_CONFIG,
    )
    .expect("wallet fixture must be written");
    let node = project
        .localnet()
        .args([
            "--accounts",
            "wallet_v2,wallet_v3_r1,wallet_v3,wallet_v4_r1,wallet_v4,wallet_v5",
        ])
        .start();

    let startup_wallets = node.get_json("/acton_getStartupWallets");
    let mut wallets = response_payload(&startup_wallets)
        .as_array()
        .expect("startup wallets response must contain an array")
        .iter()
        .map(|wallet| {
            (
                wallet["version"]
                    .as_str()
                    .expect("startup wallet must expose its version")
                    .to_owned(),
                wallet["address"]
                    .as_str()
                    .expect("startup wallet must expose its address")
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    wallets.sort_by(|left, right| left.0.cmp(&right.0));

    let mut wallet_matrix = Vec::with_capacity(wallets.len());
    for (version, address) in wallets {
        let wallet_json = node.get_json(&format!("/api/v2/getWalletInformation?address={address}"));
        let wallet: v2_responses::TonlibResponse<v2_responses::WalletInformation> =
            serde_json::from_value(wallet_json.clone())
                .expect("wallet information must match the typed V2 response");
        let extended: v2_responses::TonlibResponse<v2_responses::ExtendedAddressInformation> = node
            .get_json_as(&format!(
                "/api/v2/getExtendedAddressInformation?address={address}"
            ));
        let fields = wallet_json["result"]
            .as_object()
            .expect("wallet result must be an object");
        let present_optional_fields = ["wallet_type", "seqno", "wallet_id", "is_signature_allowed"]
            .iter()
            .filter(|field| fields.contains_key(**field))
            .copied()
            .collect::<Vec<_>>();

        wallet_matrix.push(json!({
            "version": version,
            "wallet": {
                "is_wallet": wallet.result.wallet,
                "wallet_type": wallet.result.wallet_type,
                "seqno": wallet.result.seqno,
                "wallet_id": wallet.result.wallet_id,
                "is_signature_allowed": wallet.result.is_signature_allowed,
                "has_extra_currencies": fields.contains_key("extra_currencies"),
                "present_optional_fields": present_optional_fields,
            },
            "extended": {
                "address_preserves_request_flags":
                    extended.result.address.account_address == address,
                "revision": extended.result.revision,
                "state": summarize_v2_account_state(&extended.result.account_state),
            },
        }));
    }

    let nonexist = format!("0:{}", hex::encode([0x99; 32]));
    let nonexist_wallet_json =
        node.get_json(&format!("/api/v2/getWalletInformation?address={nonexist}"));
    let nonexist_wallet: v2_responses::TonlibResponse<v2_responses::WalletInformation> =
        serde_json::from_value(nonexist_wallet_json.clone())
            .expect("nonexistent wallet response must match the V2 DTO");
    let nonexist_fields = nonexist_wallet_json["result"]
        .as_object()
        .expect("wallet result must be an object");
    let nonexist_extended: v2_responses::TonlibResponse<v2_responses::ExtendedAddressInformation> =
        node.get_json_as(&format!(
            "/api/v2/getExtendedAddressInformation?address={nonexist}"
        ));

    let uninit = format!("0:{}", hex::encode([0x77; 32]));
    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": uninit,
            "state": {"type": "uninit", "balance": "1000000000"},
        }),
    );
    let uninit_extended: v2_responses::TonlibResponse<v2_responses::ExtendedAddressInformation> =
        node.get_json_as(&format!(
            "/api/v2/getExtendedAddressInformation?address={uninit}"
        ));

    let frozen = format!("0:{}", hex::encode([0x88; 32]));
    node.post_json(
        "/acton_changeAccountState",
        &json!({
            "address": frozen,
            "state": {
                "type": "frozen",
                "frozen_hash": hex::encode([0xaa; 32]),
                "balance": "2000000000",
            },
        }),
    );
    let frozen_extended: v2_responses::TonlibResponse<v2_responses::ExtendedAddressInformation> =
        node.get_json_as(&format!(
            "/api/v2/getExtendedAddressInformation?address={frozen}"
        ));
    let nonexist_optional_fields_omitted =
        ["wallet_type", "seqno", "wallet_id", "is_signature_allowed"]
            .iter()
            .all(|field| !nonexist_fields.contains_key(*field));

    let snapshot = json!({
        "wallets": wallet_matrix,
        "nonexist": {
            "wallet": nonexist_wallet.result.wallet,
            "optional_fields_omitted": nonexist_optional_fields_omitted,
            "raw_address_uses_upstream_bounceable_form":
                nonexist_extended.result.address.account_address
                    == bounceable_user_friendly_address(&nonexist),
            "state": summarize_v2_account_state(&nonexist_extended.result.account_state),
        },
        "uninit": summarize_v2_account_state(&uninit_extended.result.account_state),
        "frozen": summarize_v2_account_state(&frozen_extended.result.account_state),
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_account_information.json"),
    );

    node.stop();
}

#[test]
fn account_extra_currencies_survive_v2_and_v3_state_queries() {
    let project = ProjectBuilder::new("localnet-account-extra-currencies").build();
    let node = project.localnet().start();
    let destination = test_std_addr(0x42);
    let destination_raw = format!("0:{}", hex::encode(destination.address.0));
    let mut value = CurrencyCollection::new(50_000_000);
    value.other = ExtraCurrencyCollection::try_from_iter([
        (7, VarUint248::new(123)),
        (u32::MAX, VarUint248::from_words(1, 0)),
    ])
    .expect("extra currencies must encode");

    let sent: v2_responses::TonlibResponse<v2_responses::InternalMessageInfo> = node.post_json_as(
        "/acton_sendInternalMessage",
        &v2_requests::SendBocRequest {
            boc: base64::engine::general_purpose::STANDARD.encode(
                build_internal_message_boc_with_currency_and_body(
                    test_std_addr(0x41),
                    destination.clone(),
                    value,
                    Cell::empty_cell(),
                ),
            ),
        },
    );
    let _ = find_v2_internal_message_by_hash(&node, &sent.result.hash);

    let address: v2_responses::TonlibResponse<v2_responses::AddressInformation> = node.get_json_as(
        &format!("/api/v2/getAddressInformation?address={destination_raw}"),
    );
    let first_seqno = address.result.block_id.seqno;
    node.post_json(
        "/acton_setNextBlockTimestamp",
        &json!({"timestamp": address.result.sync_utime + 3600}),
    );
    let second: v2_responses::TonlibResponse<v2_responses::InternalMessageInfo> = node
        .post_json_as(
            "/acton_sendInternalMessage",
            &v2_requests::SendBocRequest {
                boc: base64::engine::general_purpose::STANDARD.encode(
                    build_internal_message_boc_with_currency_and_body(
                        test_std_addr(0x43),
                        destination,
                        CurrencyCollection::new(1),
                        Cell::empty_cell(),
                    ),
                ),
            },
        );
    let _ = find_v2_internal_message_by_hash(&node, &second.result.hash);
    let latest: v2_responses::TonlibResponse<v2_responses::AddressInformation> = node.get_json_as(
        &format!("/api/v2/getAddressInformation?address={destination_raw}"),
    );
    let historical: v2_responses::TonlibResponse<v2_responses::AddressInformation> = node
        .get_json_as(&format!(
            "/api/v2/getAddressInformation?address={destination_raw}&seqno={first_seqno}"
        ));
    let extended: v2_responses::TonlibResponse<v2_responses::ExtendedAddressInformation> = node
        .get_json_as(&format!(
            "/api/v2/getExtendedAddressInformation?address={destination_raw}"
        ));
    let v3: v3_responses::AccountStatesResponse =
        node.get_json_as(&format!("/api/v3/accountStates?address={destination_raw}"));
    let v3_account = v3
        .accounts
        .first()
        .expect("V3 response must contain the requested account");

    let snapshot = json!({
        "v2_address": v2_extra_currencies(&address.result.extra_currencies),
        "v2_extended": v2_extra_currencies(&extended.result.extra_currencies),
        "v3": v3_account.extra_currencies,
        "v3_status": v3_account.status,
        "large_amount": "340282366920938463463374607431768211456",
        "historical_sync_utime": {
            "matches_first_block": historical.result.sync_utime == address.result.sync_utime,
            "differs_from_latest": historical.result.sync_utime != latest.result.sync_utime,
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/account_extra_currencies.json"),
    );

    node.stop();
}
