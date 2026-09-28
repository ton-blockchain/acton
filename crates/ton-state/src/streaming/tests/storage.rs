use rston::dict::Dict;
use tolk_source_map::abi::{ABIDeclaration, ABIStorage, ABIStructField};
use tolk_source_map::types_kernel::Ty;

use super::accounts::{event, snapshot};
use super::*;

fn abi() -> ContractABI {
    let fields = |items: &[(&str, usize)]| {
        items
            .iter()
            .map(|(name, ty_idx)| ABIStructField {
                name: (*name).to_owned(),
                ty_idx: *ty_idx,
                client_ty_idx: None,
                default_value: None,
                description: String::new(),
            })
            .collect()
    };
    ContractABI {
        abi_schema_version: "1.0".to_owned(),
        contract_name: "Counter".to_owned(),
        unique_types: vec![
            Ty::UintN { n: 32 },
            Ty::Bool,
            Ty::StructRef {
                struct_name: "Settings".to_owned(),
                type_args_ty_idx: None,
            },
            Ty::CellOf { inner_ty_idx: 2 },
            Ty::MapKV {
                key_ty_idx: 0,
                value_ty_idx: 0,
            },
            Ty::Nullable {
                inner_ty_idx: 2,
                stack_type_id: None,
                stack_width: None,
            },
            Ty::StructRef {
                struct_name: "Storage".to_owned(),
                type_args_ty_idx: None,
            },
        ],
        declarations: vec![
            ABIDeclaration::Struct {
                name: "Settings".to_owned(),
                ty_idx: 2,
                type_params: None,
                prefix: None,
                fields: fields(&[("enabled", 1)]),
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                ty_idx: 6,
                type_params: None,
                prefix: None,
                fields: fields(&[
                    ("seqno", 0),
                    ("settings", 3),
                    ("other", 0),
                    ("ledger", 4),
                    ("maybe", 5),
                ]),
                custom_pack_unpack: None,
                description: String::new(),
            },
        ],
        storage: ABIStorage {
            storage_ty_idx: Some(6),
            storage_at_deployment_ty_idx: None,
        },
        ..Default::default()
    }
}

fn data(seqno: u32, enabled: bool, other: u32, ledger: u32, maybe: Option<bool>) -> Result<Cell> {
    let mut dictionary = Dict::<u32, u32>::new();
    dictionary.set(7, ledger)?;
    let settings = CellBuilder::build_from(enabled)?;
    let mut builder = CellBuilder::new();
    builder.store_u32(seqno)?;
    builder.store_reference(settings)?;
    builder.store_u32(other)?;
    builder.store_slice(CellBuilder::build_from((&dictionary, maybe))?.as_slice()?)?;
    Ok(builder.build()?)
}

fn request() -> Value {
    json!({
        "types": ["storage_fields"],
        "addresses": [account(2, 0).to_string()],
        "abi": abi(),
        "fields": ["seqno", "settings.enabled", "ledger", "maybe.enabled"],
        "include_code_data": false,
    })
}

fn committed(seqno: u32) -> Result<Batch> {
    Batch::try_new(
        block(ShardIdent::MASTERCHAIN, seqno, Vec::new())?,
        vec![block(
            ShardIdent::BASECHAIN,
            seqno,
            vec![transaction(2)?, transaction(3)?],
        )?],
    )
    .map_err(Into::into)
}

fn state(data: Cell) -> AccountState {
    AccountState::Active(StateInit {
        data: Some(data),
        ..Default::default()
    })
}

async fn open(
    hub: &Subscriptions,
    request: Value,
    seqno: u32,
    data: Option<Cell>,
) -> Result<Response> {
    let batch = committed(seqno)?;
    Ok(open_subscription(
        hub.clone(),
        HeaderMap::new(),
        Ok(Json(serde_json::from_value(request)?)),
        move |address| snapshot(&batch, address, data.clone().map(state)),
    )
    .await)
}

async fn idle(body: &mut Body) -> bool {
    tokio::time::timeout(Duration::from_millis(10), body.frame())
        .await
        .is_err()
}

#[tokio::test]
async fn snapshots_and_selected_changes_preserve_the_complete_data_boc() -> Result<()> {
    let hub = Subscriptions::default();
    let initial = data(1, true, 10, 100, None)?;
    let mut request = request();
    request["addresses"] = json!([
        account(2, 0).to_string(),
        account(2, 0).display_base64(true).to_string(),
    ]);
    let response = open(&hub, request.clone(), 42, Some(initial.clone())).await?;
    let status = response.status().as_u16();
    let mut body = response.into_body();
    let mut events = vec![event(&mut body).await?, event(&mut body).await?];
    let mut quiet = Vec::new();
    let mut matching_bocs = vec![events[1]["data"] == Boc::encode_base64(&initial)];

    for (seqno, value, emits) in [
        (41, data(998, false, 20, 999, Some(false))?, false),
        (42, data(999, false, 20, 999, Some(false))?, false),
        (43, initial, false),
        (44, data(1, true, 20, 100, None)?, false),
        (45, data(2, false, 20, 100, None)?, true),
        (46, data(2, false, 20, 101, Some(true))?, true),
        (47, data(2, false, 20, 101, Some(false))?, true),
    ] {
        let batch = committed(seqno)?;
        hub.publish(&batch, |address| {
            snapshot(&batch, address, Some(state(value.clone())))
        })?;
        if emits {
            let update = event(&mut body).await?;
            let http =
                crate::api::account_info(snapshot(&batch, &account(2, 0), Some(state(value)))?)?;
            matching_bocs.push(update["data"] == serde_json::to_value(http)?["data"]);
            events.push(update);
        }
        quiet.push(idle(&mut body).await);
    }
    let final_data = data(2, false, 20, 101, Some(false))?;
    let mut late = open(&hub, request, 47, Some(final_data)).await?.into_body();
    event(&mut late).await?;
    events.push(event(&mut late).await?);
    expect_file!["../snapshots/storage-updates.json"].assert_eq(&serde_json::to_string_pretty(
        &json!({
            "status": status, "events": events, "quiet_after_publications": quiet,
            "full_data_matches_http": matching_bocs,
        }),
    )?);
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn missing_storage_clears_fields_and_redeployment_restores_them() -> Result<()> {
    let hub = Subscriptions::default();
    let mut body = open(&hub, request(), 42, None).await?.into_body();
    event(&mut body).await?;
    let mut events = vec![event(&mut body).await?];
    for (seqno, next) in [
        (43, Some(state(data(0, false, 0, 0, None)?))),
        (44, Some(AccountState::Frozen(HashBytes([9; 32])))),
        (45, Some(state(data(0, false, 0, 0, None)?))),
    ] {
        let batch = committed(seqno)?;
        hub.publish(&batch, |address| snapshot(&batch, address, next.clone()))?;
        events.push(event(&mut body).await?);
    }
    expect_file!["../snapshots/storage-lifecycle.json"]
        .assert_eq(&serde_json::to_string_pretty(&events)?);
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn mixed_streams_coalesce_accounts_and_isolate_abi_failures() -> Result<()> {
    let hub = Subscriptions::default();
    let mut combined_request = request();
    combined_request["types"] = json!(["transactions", "account_states", "storage_fields"]);
    combined_request["addresses"] = json!([account(2, 0).to_string(), account(3, 0).to_string()]);
    let initial = data(1, true, 0, 1, None)?;
    let mut combined = open(&hub, combined_request, 42, Some(initial))
        .await?
        .into_body();
    let mut plain = subscribe_to(
        &hub,
        json!({
            "types": ["account_states"], "addresses": [account(2, 0).to_string()],
        }),
    )
    .await?
    .into_body();
    event(&mut combined).await?;
    let initial_addresses = vec![
        event(&mut combined).await?["address"].clone(),
        event(&mut combined).await?["address"].clone(),
    ];
    event(&mut plain).await?;
    let batch = Batch::try_new(
        block(ShardIdent::MASTERCHAIN, 43, Vec::new())?,
        vec![
            block(ShardIdent::BASECHAIN, 17, vec![transaction(2)?])?,
            block(ShardIdent::BASECHAIN, 18, vec![transaction(2)?])?,
        ],
    )?;
    hub.publish(&batch, |address| {
        snapshot(&batch, address, Some(state(data(2, true, 0, 1, None)?)))
    })?;
    let mut types = Vec::new();
    for _ in 0..4 {
        types.push(event(&mut combined).await?["type"].clone());
    }
    event(&mut plain).await?;
    let coalesced = idle(&mut combined).await;

    // A schema failure must not close an unrelated account-state subscription.
    let batch = committed(44)?;
    hub.publish(&batch, |address| {
        snapshot(&batch, address, Some(state(CellBuilder::build_from(0_u8)?)))
    })?;
    let transaction = event(&mut combined).await?["type"].clone();
    let other_transaction = event(&mut combined).await?["type"].clone();
    let error = event(&mut combined).await?;
    let ended = combined.frame().await.is_none();
    let unaffected = event(&mut plain).await?["type"].clone();
    expect_file!["../snapshots/storage-mixed.json"].assert_eq(&serde_json::to_string_pretty(
        &json!({
            "initial_addresses": initial_addresses, "types": types, "coalesced": coalesced,
            "transactions_before_error": [transaction, other_transaction],
            "error": error, "ended": ended, "other_subscription": unaffected,
        }),
    )?);
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn invalid_schemas_paths_and_inputs_are_rejected_before_registration() -> Result<()> {
    let hub = Subscriptions::default();
    let initial = data(1, true, 10, 100, None)?;
    let mut cases = Vec::new();
    for fields in [
        json!([]),
        json!(["missing"]),
        json!(["seqno", "seqno"]),
        json!(["settings..enabled"]),
        json!(["ledger.7"]),
    ] {
        let mut request = request();
        request["fields"] = fields;
        cases.push(request);
    }
    let mut missing_abi = request();
    missing_abi.as_object_mut().unwrap().remove("abi");
    cases.push(missing_abi);
    let mut wrong_type = request();
    wrong_type["types"] = json!(["account_states"]);
    cases.push(wrong_type);
    let mut missing_storage = request();
    missing_storage["abi"]["storage"] = json!({});
    cases.push(missing_storage);
    let mut custom = request();
    custom["abi"]["declarations"][1]["custom_pack_unpack"] = json!({"unpack_from_slice": true});
    cases.push(custom);
    let mut recursive = request();
    recursive["abi"]["declarations"][0]["fields"][0]["ty_idx"] = json!(3);
    cases.push(recursive);
    let mut unknown_type = request();
    unknown_type["abi"]["declarations"][0]["fields"][0]["ty_idx"] = json!(9999);
    cases.push(unknown_type);
    let mut rows = Vec::new();
    for request in cases {
        let response = open(&hub, request, 42, Some(initial.clone())).await?;
        let status = response.status().as_u16();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        rows.push(json!({"status": status, "body": body}));
    }
    let response = open(&hub, request(), 42, Some(CellBuilder::build_from(0_u8)?)).await?;
    rows.push(json!({
        "malformed_data_status": response.status().as_u16(),
        "active_subscribers": hub.subscribers.lock().unwrap().len(),
        "available_slots": hub.connections.available_permits(),
    }));
    expect_file!["../snapshots/storage-invalid.json"]
        .assert_eq(&serde_json::to_string_pretty(&rows)?);
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn slow_storage_consumers_disconnect_and_release_their_slot() -> Result<()> {
    let hub = Subscriptions::default();
    let initial = data(1, true, 0, 0, None)?;
    let mut slow = open(&hub, request(), 42, Some(initial.clone()))
        .await?
        .into_body();
    let mut fast = open(&hub, request(), 42, Some(initial)).await?.into_body();
    for body in [&mut slow, &mut fast] {
        event(body).await?;
        event(body).await?;
    }
    for seqno in 43..=43 + QUEUE_EVENTS as u32 {
        let batch = committed(seqno)?;
        hub.publish(&batch, |address| {
            snapshot(&batch, address, Some(state(data(seqno, true, 0, 0, None)?)))
        })?;
        event(&mut fast).await?;
    }
    let error = event(&mut slow).await?;
    let ended = slow.frame().await.is_none();
    drop(slow);
    expect![[r#"{"available_slots":63,"ended":true,"error":{"error":"slow_consumer","type":"error"}}"#]]
        .assert_eq(&json!({"error": error, "ended": ended, "available_slots": hub.connections.available_permits()}).to_string());
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn registration_and_publication_have_no_snapshot_gap() -> Result<()> {
    let hub = Subscriptions::default();
    let initial_batch = committed(42)?;
    let (pinned, ready) = tokio::sync::oneshot::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let subscriber_hub = hub.clone();
    let subscription = tokio::spawn(async move {
        let mut pinned = Some(pinned);
        open_subscription(
            subscriber_hub,
            HeaderMap::new(),
            Ok(Json(serde_json::from_value(request()).unwrap())),
            move |address| {
                pinned.take().unwrap().send(()).unwrap();
                resume.recv()?;
                snapshot(
                    &initial_batch,
                    address,
                    Some(state(data(1, true, 0, 0, None)?)),
                )
            },
        )
        .await
    });
    ready.await?;
    let publisher = hub.clone();
    let mut publication = tokio::task::spawn_blocking(move || {
        let batch = committed(43)?;
        publisher.publish(&batch, |address| {
            snapshot(&batch, address, Some(state(data(2, true, 0, 0, None)?)))
        })
    });
    let waited_for_snapshot = tokio::time::timeout(Duration::from_millis(20), &mut publication)
        .await
        .is_err();
    release.send(())?;
    let response = subscription.await?;
    publication.await??;
    let mut body = response.into_body();
    event(&mut body).await?;
    let initial = event(&mut body).await?;
    let update = event(&mut body).await?;
    expect![[
        r#"{"initial":[true,42,[]],"update":[false,43,["seqno"]],"waited_for_snapshot":true}"#
    ]]
    .assert_eq(
        &json!({
            "initial": [initial["initial"], initial["mc_seqno"], initial["changed_fields"]],
            "update": [update["initial"], update["mc_seqno"], update["changed_fields"]],
            "waited_for_snapshot": waited_for_snapshot,
        })
        .to_string(),
    );
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn zero_width_collections_cannot_hold_the_stream_decoder_forever() -> Result<()> {
    let hub = Subscriptions::default();
    let mut request = request();
    request["fields"] = json!(["seqno"]);
    request["abi"]["unique_types"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"kind": "void"}),
            json!({"kind": "arrayOf", "inner_ty_idx": 7}),
        ]);
    request["abi"]["declarations"][1]["fields"] = json!([{"name": "seqno", "ty_idx": 8}]);
    // The chunk has an unused bit and no continuation reference. A void element
    // consumes nothing, so only the decoder budget can terminate this input.
    let chunk = CellBuilder::build_from((false, true))?;
    let data = CellBuilder::build_from((0_u8, Some(chunk)))?;
    let response = open(&hub, request, 42, Some(data)).await?;
    let status = response.status().as_u16();
    let body: Value = serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    expect![[r#"{"body":{"error":"storage_initialization_failed","message":"failed to decode field Storage.seqno: ABI value limit exceeded"},"status":400}"#]]
        .assert_eq(&json!({"status": status, "body": body}).to_string());
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn wallet_v5_compiler_abi_filters_a_real_mainnet_storage_snapshot() -> Result<()> {
    let abi: ContractABI = serde_json::from_str(include_str!("../fixtures/wallet-v5r1.abi.json"))?;
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/wallet-v5r1-state.json"))?;
    let (address, _) =
        StdAddr::from_str_ext(fixture["address"].as_str().unwrap(), StdAddrFormat::any())?;
    let initial = Boc::decode_base64(fixture["data"].as_str().unwrap())?;
    let checkpoint = fixture["mc_seqno"].as_u64().unwrap() as u32;
    let make_batch = |seqno| -> Result<Batch> {
        let mut tx = transaction(2)?;
        tx.account = address.address;
        Ok(Batch::try_new(
            block(ShardIdent::MASTERCHAIN, seqno, Vec::new())?,
            vec![block(ShardIdent::BASECHAIN, seqno, vec![tx])?],
        )?)
    };
    let batch = make_batch(checkpoint)?;
    let hub = Subscriptions::default();
    let request = json!({
        "types": ["storage_fields"], "addresses": [fixture["address"]], "abi": abi,
        "fields": ["seqno", "isSignatureAllowed", "extensions"],
    });
    let seed = initial.clone();
    let mut body = open_subscription(
        hub.clone(),
        HeaderMap::new(),
        Ok(Json(serde_json::from_value(request)?)),
        move |address| snapshot(&batch, address, Some(state(seed.clone()))),
    )
    .await
    .into_body();
    event(&mut body).await?;
    let first = event(&mut body).await?;

    // Simulate only a seqno change in the captured storage. No signature, wallet
    // secret, or network transaction is needed to exercise the ABI subscription.
    let mut tail = initial.as_slice()?;
    let enabled = tail.load_bit()?;
    let seqno = tail.load_u32()?;
    let mut builder = CellBuilder::new();
    builder.store_bit(enabled)?;
    builder.store_u32(seqno + 1)?;
    builder.store_slice(tail)?;
    let updated = builder.build()?;
    let batch = make_batch(checkpoint + 1)?;
    hub.publish(&batch, |address| {
        snapshot(&batch, address, Some(state(updated.clone())))
    })?;
    let update = event(&mut body).await?;
    let quiet = idle(&mut body).await;
    expect_file!["../snapshots/storage-wallet-v5.json"].assert_eq(&serde_json::to_string_pretty(
        &json!({
            "address": address.to_string(), "seqno_before": seqno, "seqno_after": seqno + 1,
            "initial": first, "update": update,
            "original_boc_preserved": first["data"] == fixture["data"],
            "no_extra_events": quiet,
        }),
    )?);
    drop(hub);
    Ok(())
}
