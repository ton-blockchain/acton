use rston::models::{Account, AccountState, OptionalAccount, ShardAccount};
use ton_node_db::ReadStats;

use super::*;

pub(super) fn snapshot(
    batch: &Batch,
    address: &StdAddr,
    state: Option<AccountState>,
) -> Result<AccountSnapshot> {
    let account = state
        .map(|state| {
            let mut balance = CurrencyCollection::new(1_234_567_890);
            balance
                .other
                .as_dict_mut()
                .set(7, VarUint248::from(44_u32))?;
            anyhow::Ok(ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(Account {
                    address: address.clone().into(),
                    storage_stat: Default::default(),
                    last_trans_lt: 900,
                    balance,
                    state,
                })))?,
                last_trans_hash: HashBytes([4; 32]),
                last_trans_lt: 899,
            })
        })
        .transpose()?;
    let shard_block = batch
        .blocks()
        .filter(|block| block.id().workchain == i32::from(address.workchain))
        .last()
        .context("missing account block")?
        .id()
        .try_into()?;
    Ok(AccountSnapshot {
        masterchain_block: batch.checkpoint().try_into()?,
        shard_block,
        gen_utime: 1_700_000_000,
        account,
        reads: ReadStats {
            records: 0,
            bytes: 0,
            cache_hits: 0,
        },
    })
}

pub(super) async fn event(body: &mut Body) -> Result<Value> {
    let text = frame(body).await?;
    Ok(serde_json::from_str(
        text.trim()
            .strip_prefix("data: ")
            .context("missing SSE data")?,
    )?)
}

#[tokio::test]
async fn state_subscriptions_filter_and_coalesce_at_the_committed_frontier() -> Result<()> {
    let hub = Subscriptions::default();
    let base = account(2, 0);
    let master = account(2, -1);
    let mut states = subscribe_to(&hub, json!({
        "types": ["account_states"],
        "addresses": [base.to_string(), base.display_base64(true).to_string(), master.to_string(), account(9, 0).to_string()],
    })).await?.into_body();
    let mut combined = subscribe_to(
        &hub,
        json!({
            "types": ["transactions", "account_states", "account_states"],
            "addresses": [base.to_string()],
        }),
    )
    .await?
    .into_body();
    let mut transactions = subscribe_to(
        &hub,
        json!({
            "types": null, "min_finality": null, "addresses": [base.to_string()],
        }),
    )
    .await?
    .into_body();
    for body in [&mut states, &mut combined, &mut transactions] {
        expect![[r#"{"status":"subscribed"}"#]].assert_eq(&event(body).await?.to_string());
    }

    let batch = Batch::try_new(
        block(ShardIdent::MASTERCHAIN, 42, vec![transaction(2)?])?,
        vec![
            block(
                ShardIdent::BASECHAIN,
                17,
                vec![transaction(2)?, transaction(3)?],
            )?,
            block(ShardIdent::BASECHAIN, 18, vec![transaction(2)?])?,
        ],
    )?;
    let active = AccountState::Active(StateInit {
        code: Some(CellBuilder::build_from(123_u32)?),
        data: Some(CellBuilder::build_from(456_u32)?),
        ..Default::default()
    });
    let mut reads = Vec::new();
    hub.publish(&batch, |address| {
        reads.push(address.to_string());
        snapshot(&batch, address, Some(active.clone()))
    })?;
    let master_event = event(&mut states).await?;
    let base_event = event(&mut states).await?;
    let mut combined_types = Vec::new();
    for _ in 0..2 {
        combined_types.push(event(&mut combined).await?["type"].clone());
    }
    let combined_state = event(&mut combined).await?;
    combined_types.push(combined_state["type"].clone());
    let transaction_types = vec![
        event(&mut transactions).await?["type"].clone(),
        event(&mut transactions).await?["type"].clone(),
    ];
    let http_state = serde_json::to_value(crate::api::account_info(snapshot(
        &batch,
        &base,
        Some(active),
    )?)?)?;

    // A new stream receives no initial state, and an empty batch reads no accounts.
    let mut late = subscribe_to(
        &hub,
        json!({
            "types": ["account_states"], "addresses": [base.to_string()],
        }),
    )
    .await?
    .into_body();
    frame(&mut late).await?;
    hub.publish(
        &Batch::try_new(block(ShardIdent::MASTERCHAIN, 43, Vec::new())?, Vec::new())?,
        no_account_reads,
    )?;
    let mut idle = Vec::new();
    for body in [&mut states, &mut combined, &mut transactions, &mut late] {
        idle.push(
            tokio::time::timeout(Duration::from_millis(10), body.frame())
                .await
                .is_err(),
        );
    }
    drop(hub);
    expect_file!["../snapshots/account-state.json"].assert_eq(&serde_json::to_string_pretty(
        &json!({
            "reads": reads,
            "masterchain_event": master_event,
            "basechain_event": base_event,
            "combined_types": combined_types,
            "transaction_types": transaction_types,
            "same_event_for_combined_subscription": combined_state == base_event,
            "matches_http_result": base_event["account_state"] == http_state,
            "idle_after_batch": idle,
        }),
    )?);
    Ok(())
}

#[tokio::test]
async fn frozen_uninitialized_and_deleted_accounts_use_the_http_representation() -> Result<()> {
    let hub = Subscriptions::default();
    let batch = batch()?;
    let mut body = subscribe_to(
        &hub,
        json!({
            "types": ["account_states"], "addresses": [account(2, 0).to_string()],
        }),
    )
    .await?
    .into_body();
    frame(&mut body).await?;
    let mut events = Vec::new();
    for state in [
        Some(AccountState::Frozen(HashBytes([3; 32]))),
        Some(AccountState::Uninit),
        None,
    ] {
        hub.publish(&batch, |address| snapshot(&batch, address, state.clone()))?;
        events.push(event(&mut body).await?);
    }
    drop(hub);
    expect_file!["../snapshots/account-lifecycle.json"]
        .assert_eq(&serde_json::to_string_pretty(&events)?);
    Ok(())
}

#[tokio::test]
async fn code_and_data_are_independent_per_account_connection_baselines() -> Result<()> {
    let hub = Subscriptions::default();
    let request = json!({"types": ["account_states"], "addresses": [account(2, 0).to_string()]});
    let mut body = subscribe_to(&hub, request.clone()).await?.into_body();
    frame(&mut body).await?;
    let mut late = None;
    let mut reconstructed = serde_json::Map::new();
    let mut rows = Vec::new();

    for (index, (code, data)) in [
        (Some(1), Some(10)),
        (Some(1), Some(10)),
        (Some(1), Some(11)),
        (Some(2), Some(11)),
        (None, Some(11)),
        (None, None),
        (None, None),
        (Some(2), Some(11)),
    ]
    .into_iter()
    .enumerate()
    {
        let batch = Batch::try_new(
            block(ShardIdent::MASTERCHAIN, 42 + index as u32, Vec::new())?,
            vec![block(
                ShardIdent::BASECHAIN,
                17 + index as u32,
                vec![transaction(2)?],
            )?],
        )?;
        let state = AccountState::Active(StateInit {
            code: code
                .map(|value: u32| CellBuilder::build_from(value))
                .transpose()?,
            data: data
                .map(|value: u32| CellBuilder::build_from(value))
                .transpose()?,
            ..Default::default()
        });
        // Joining after the first publication must still deliver both BoCs,
        // regardless of the earlier connection's code/data baseline.
        if index == 1 {
            let mut connected = subscribe_to(&hub, request.clone()).await?.into_body();
            frame(&mut connected).await?;
            late = Some(connected);
        }
        hub.publish(&batch, |address| {
            snapshot(&batch, address, Some(state.clone()))
        })?;
        let update = event(&mut body).await?;
        let fields = update["account_state"]
            .as_object()
            .context("missing state fields")?;
        let mut expected: serde_json::Map<String, Value> =
            serde_json::from_value(serde_json::to_value(crate::api::account_info(snapshot(
                &batch,
                &account(2, 0),
                Some(state),
            )?)?)?)?;
        reconstructed.extend(fields.clone());
        let matches_http = reconstructed == expected;
        expected.remove("code");
        expected.remove("data");
        let mut small_fields = fields.clone();
        small_fields.remove("code");
        small_fields.remove("data");
        let late_fields = if let Some(late) = &mut late {
            let update = event(late).await?;
            let fields = update["account_state"]
                .as_object()
                .context("missing late state fields")?;
            Some(json!({"code": fields.get("code"), "data": fields.get("data")}))
        } else {
            None
        };
        rows.push(json!({
            "seqno": batch.checkpoint().seqno,
            "code": fields.get("code"),
            "data": fields.get("data"),
            "small_fields_complete": small_fields == expected,
            "reconstructed_matches_http": matches_http,
            "late_connection": late_fields,
        }));
    }
    drop(hub);
    expect_file!["../snapshots/account-code-data.json"]
        .assert_eq(&serde_json::to_string_pretty(&rows)?);
    Ok(())
}

#[tokio::test]
async fn code_data_flag_keeps_small_fields_and_transaction_events_complete() -> Result<()> {
    let hub = Subscriptions::default();
    let batch = batch()?;
    let mut connections = Vec::new();
    for flag in [json!(false), json!(true), Value::Null] {
        let mut body = subscribe_to(
            &hub,
            json!({
                "types": ["transactions", "account_states"],
                "addresses": [account(2, 0).to_string()],
                "include_code_data": flag,
            }),
        )
        .await?
        .into_body();
        frame(&mut body).await?;
        connections.push((flag, body));
    }
    let mut rows = Vec::new();
    for value in [Some(1_u32), Some(2_u32), None] {
        let state = value
            .map(|value| {
                anyhow::Ok(AccountState::Active(StateInit {
                    code: Some(CellBuilder::build_from(value)?),
                    data: Some(CellBuilder::build_from(value + 10)?),
                    ..Default::default()
                }))
            })
            .transpose()?;
        hub.publish(&batch, |address| snapshot(&batch, address, state.clone()))?;
        let mut expected = serde_json::to_value(crate::api::account_info(snapshot(
            &batch,
            &account(2, 0),
            state,
        )?)?)?;
        let expected = expected.as_object_mut().context("missing HTTP state")?;
        expected.remove("code");
        expected.remove("data");
        let mut transaction = None;
        for (flag, body) in &mut connections {
            let tx = event(body).await?;
            let transaction_matches = transaction.get_or_insert_with(|| tx.clone()) == &tx;
            let state = event(body).await?;
            let mut fields = state["account_state"]
                .as_object()
                .context("missing state")?
                .clone();
            let code = fields.remove("code");
            let data = fields.remove("data");
            rows.push(json!({
                "include_code_data": flag,
                "code": code,
                "data": data,
                "small_fields_match_http": fields == *expected,
                "transaction_type": tx["type"],
                "transaction_matches_other_subscriptions": transaction_matches,
            }));
        }
    }
    drop(hub);
    expect_file!["../snapshots/account-code-data-flag.json"]
        .assert_eq(&serde_json::to_string_pretty(&rows)?);
    Ok(())
}

#[tokio::test]
async fn state_read_failures_and_wrong_checkpoints_report_a_gap() -> Result<()> {
    let hub = Subscriptions::default();
    let batch = batch()?;
    let mut outcomes = Vec::new();
    for wrong_checkpoint in [false, true] {
        let mut body = subscribe_to(
            &hub,
            json!({
                "types": ["account_states"], "addresses": [account(2, 0).to_string()],
            }),
        )
        .await?
        .into_body();
        frame(&mut body).await?;
        let result = hub.publish(&batch, |address| {
            if !wrong_checkpoint {
                anyhow::bail!("state unavailable");
            }
            let mut state = snapshot(&batch, address, None)?;
            state.masterchain_block.seqno += 1;
            Ok(state)
        });
        let error = format!("{:#}", result.unwrap_err());
        hub.fail();
        let error_event = event(&mut body).await?;
        outcomes.push(
            json!({"error": error, "event": error_event, "closed": body.frame().await.is_none()}),
        );
    }
    drop(hub);
    expect_file!["../snapshots/account-failures.json"]
        .assert_eq(&serde_json::to_string_pretty(&outcomes)?);
    Ok(())
}

#[tokio::test]
async fn account_events_share_queue_limits_and_release_slots_on_disconnect() -> Result<()> {
    let hub = Subscriptions::default();
    let batch = batch()?;
    let request = json!({"types": ["account_states"], "addresses": [account(2, 0).to_string()]});
    let mut slow = subscribe_to(&hub, request.clone()).await?.into_body();
    let mut fast = subscribe_to(&hub, request).await?.into_body();
    frame(&mut slow).await?;
    frame(&mut fast).await?;
    for _ in 0..=QUEUE_EVENTS {
        hub.publish(&batch, |address| snapshot(&batch, address, None))?;
        event(&mut fast).await?;
    }
    let error = event(&mut slow).await?;
    let ended = slow.frame().await.is_none();
    drop(slow);
    hub.publish(&batch, |address| snapshot(&batch, address, None))?;
    let next = event(&mut fast).await?;
    drop(fast);
    // Closed subscribers are removed before any state read.
    hub.publish(&batch, no_account_reads)?;
    expect![[r#"{"closed":true,"error":{"error":"slow_consumer","type":"error"},"next":"account_state","slots":64}"#]].assert_eq(&json!({
        "error": error, "closed": ended, "next": next["type"], "slots": hub.connections.available_permits(),
    }).to_string());
    drop(hub);
    Ok(())
}
