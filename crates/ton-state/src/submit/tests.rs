mod trace;

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::Request;
use expect_test::{expect, expect_file};
use http_body_util::BodyExt;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, Lazy};
use rston::models::{MessageLayout, MsgInfo, OwnedMessage, ShardIdent, Transaction, TxInfo};
use rston::num::Tokens;
use serde_json::{Value, json};
use tokio::sync::Notify;
use ton_indexer_core::Batch;
use tower::ServiceExt;

use super::*;
use crate::streaming::tests::{block, transaction};

#[tokio::test]
async fn malformed_messages_use_v2_errors() {
    let mut outcomes = Vec::new();
    for boc in [
        "?".to_owned(),
        STANDARD.encode([0]),
        STANDARD.encode(vec![0; 65_536]),
    ] {
        let response = parse_message(SendBocRequest { boc })
            .err()
            .unwrap()
            .into_response();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        outcomes.push(format!("{status}: {}", String::from_utf8_lossy(&body)));
    }

    expect![[r#"
        400 Bad Request: {"ok":false,"error":"boc must contain valid base64","code":400}
        400 Bad Request: {"ok":false,"error":"invalid inbound external message","code":400}
        413 Payload Too Large: {"ok":false,"error":"boc exceeds 65535 bytes","code":413}"#]]
    .assert_eq(&outcomes.join("\n"));
}

fn service(confirmations: Confirmations, broadcast: Broadcast) -> Submission {
    Submission {
        broadcast,
        confirmations,
        capacity: Arc::new(Semaphore::new(16)),
    }
}

fn request(body: Value) -> Request<Body> {
    Request::post("/api/sendAndWaitTransaction")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn response(response: Response) -> Value {
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    json!({ "status": status, "body": serde_json::from_slice::<Value>(&bytes).unwrap() })
}

fn committed(tx: Transaction, shard: ShardIdent) -> anyhow::Result<Batch> {
    if shard.is_masterchain() {
        Ok(Batch::try_new(block(shard, 42, vec![tx])?, vec![])?)
    } else {
        Ok(Batch::try_new(
            block(ShardIdent::MASTERCHAIN, 42, vec![])?,
            vec![block(shard, 17, vec![tx])?],
        )?)
    }
}

fn message() -> Cell {
    transaction(2).unwrap().in_msg.unwrap()
}

fn available_observations(confirmations: &Confirmations) -> usize {
    let mut observations = Vec::new();
    while let Ok(observation) = confirmations.register(
        StdAddr::new(0, HashBytes::ZERO),
        HashBytes::ZERO,
        WaitFor::Transaction,
    ) {
        observations.push(observation);
    }
    observations.len()
}

#[tokio::test]
async fn normalizes_before_broadcast_and_returns_the_committed_transaction() -> anyhow::Result<()> {
    let confirmations = Confirmations::default();
    let tx = transaction(2)?;
    let original = tx.in_msg.clone().unwrap();
    let mut submitted = original.parse::<OwnedMessage>()?;
    submitted.init = None;
    submitted.layout = Some(MessageLayout {
        init_to_cell: false,
        body_to_cell: true,
    });
    let MsgInfo::ExtIn(info) = &mut submitted.info else {
        unreachable!()
    };
    info.import_fee = Tokens::new(9);
    info.src = rston::models::ExtAddr::new(8, [7]);
    let submitted = CellBuilder::build_from(submitted)?;
    let expected_hash = *submitted.repr_hash();
    let batch = committed(tx, ShardIdent::BASECHAIN)?;
    let broadcasts = Arc::new(AtomicUsize::new(0));
    let seen = broadcasts.clone();
    let observer = confirmations.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |message| {
            let attempt = seen.fetch_add(1, Ordering::Relaxed);
            expect![["true"]].assert_eq(&(message.hash() == expected_hash).to_string());
            // Commit during the send call, before the send future can complete.
            observer.publish(&batch).unwrap();
            if attempt == 0 {
                Box::pin(std::future::pending())
            } else {
                Box::pin(async { anyhow::bail!("later peer send failed") })
            }
        }),
    );
    let capacity = app.capacity.clone();
    let router = app.router();
    let reply = tokio::time::timeout(
        Duration::from_secs(3),
        router.clone().oneshot(request(json!({
            "boc": Boc::encode_base64(&submitted),
        }))),
    )
    .await??;
    let reply = response(reply).await;
    let partial_send_failure = response(
        router
            .oneshot(request(json!({
                "boc": Boc::encode_base64(&submitted),
            })))
            .await?,
    )
    .await;
    let result = json!({
        "response": reply,
        "partial_send_failure_still_returns_transaction": partial_send_failure == reply,
        "original_hash_differs": original.repr_hash() != submitted.repr_hash(),
        "broadcasts": broadcasts.load(Ordering::Relaxed),
        "submission_slots": capacity.available_permits(),
        "observation_slots": available_observations(&confirmations),
    });
    expect_file!["snapshots/confirmed.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&result)?));
    Ok(())
}

#[tokio::test]
async fn concurrent_waiters_ignore_unrelated_messages_and_return_aborted_masterchain_transaction()
-> anyhow::Result<()> {
    let confirmations = Confirmations::default();
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            signal.add_permits(1);
            Box::pin(async { Ok(()) })
        }),
    );
    let capacity = app.capacity.clone();
    let mut tx = transaction(2)?;
    let mut incoming = tx.in_msg.as_ref().unwrap().parse::<OwnedMessage>()?;
    let MsgInfo::ExtIn(info) = &mut incoming.info else {
        unreachable!()
    };
    info.dst = StdAddr::new(-1, tx.account).into();
    tx.in_msg = Some(CellBuilder::build_from(incoming)?);
    let mut info = tx.load_info()?;
    let TxInfo::Ordinary(ordinary) = &mut info else {
        unreachable!()
    };
    ordinary.aborted = true;
    tx.info = Lazy::new(&info)?;
    let boc = Boc::encode_base64(tx.in_msg.as_ref().unwrap());
    let router = app.router();
    let first = tokio::spawn(router.clone().oneshot(request(json!({"boc": boc}))));
    let second = tokio::spawn(router.oneshot(request(json!({"boc": boc}))));
    started.acquire_many(2).await?.forget();

    // Same account bytes in the wrong workchain and another body must not match.
    confirmations.publish(&committed(transaction(2)?, ShardIdent::BASECHAIN)?)?;
    let mut unrelated = tx.clone();
    let mut input = unrelated.in_msg.as_ref().unwrap().parse::<OwnedMessage>()?;
    input.body = CellBuilder::build_from(7_u32)?.into();
    unrelated.in_msg = Some(CellBuilder::build_from(input)?);
    confirmations.publish(&committed(unrelated, ShardIdent::MASTERCHAIN)?)?;
    let still_waiting = !first.is_finished() && !second.is_finished();
    confirmations.publish(&committed(tx.clone(), ShardIdent::MASTERCHAIN)?)?;
    let first = response(first.await??).await;
    let second = response(second.await??).await;
    let returned = Boc::decode_base64(
        first["body"]["result"]["transaction"]["data"]
            .as_str()
            .unwrap(),
    )?
    .parse::<Transaction>()?;
    let TxInfo::Ordinary(info) = returned.load_info()? else {
        unreachable!()
    };
    let summary = json!({
        "still_waiting_after_unrelated": still_waiting,
        "responses_equal": first == second,
        "status": first["status"],
        "aborted": info.aborted,
        "block": first["body"]["result"]["block_id"],
        "checkpoint": first["body"]["result"]["mc_block_seqno"],
        "original_transaction": CellBuilder::build_from(returned)?.repr_hash() == CellBuilder::build_from(tx)?.repr_hash(),
        "submission_slots": capacity.available_permits(),
        "observation_slots": available_observations(&confirmations),
    });
    expect_file!["snapshots/concurrent.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&summary)?));
    Ok(())
}

#[tokio::test]
async fn failed_submission_timeout_shutdown_and_publication_gap_release_capacity()
-> anyhow::Result<()> {
    let mut outcomes = Vec::new();
    for mode in [
        "send_failure",
        "timeout_during_send",
        "timeout_after_send",
        "shutdown",
        "publication_gap",
    ] {
        let confirmations = Confirmations::default();
        let started = Arc::new(Notify::new());
        let signal = started.clone();
        let app = service(
            confirmations.clone(),
            Arc::new(move |_| {
                signal.notify_one();
                Box::pin(async move {
                    match mode {
                        "send_failure" => anyhow::bail!("test transport failure"),
                        "timeout_during_send" | "shutdown" | "publication_gap" => {
                            std::future::pending().await
                        }
                        _ => {
                            tokio::time::sleep(Duration::from_millis(750)).await;
                            Ok(())
                        }
                    }
                })
            }),
        );
        let capacity = app.capacity.clone();
        let task = tokio::spawn(app.router().oneshot(request(json!({
            "boc": Boc::encode_base64(message()), "timeout_ms": 1000,
        }))));
        started.notified().await;
        match mode {
            "timeout_during_send" | "timeout_after_send" => {
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(1)).await;
            }
            "shutdown" => confirmations.close(),
            "publication_gap" => confirmations.fail(),
            _ => {}
        }
        let response = response(task.await??).await;
        if mode.starts_with("timeout") {
            tokio::time::resume();
        }
        outcomes.push(json!({
            "mode": mode, "response": response,
            "submission_slots": capacity.available_permits(),
            "observation_slots": available_observations(&confirmations),
        }));
    }
    expect_file!["snapshots/failures.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&outcomes)?));
    Ok(())
}

#[tokio::test]
async fn cancellation_and_capacity_limits_do_not_block_send_boc() -> anyhow::Result<()> {
    let confirmations = Confirmations::default();
    let broadcasts = Arc::new(AtomicUsize::new(0));
    let seen = broadcasts.clone();
    let started = Arc::new(Notify::new());
    let signal = started.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            seen.fetch_add(1, Ordering::Relaxed);
            signal.notify_one();
            Box::pin(async { Ok(()) })
        }),
    );
    let capacity = app.capacity.clone();
    let router = app.router();
    let task = tokio::spawn(
        router
            .clone()
            .oneshot(request(json!({"boc": Boc::encode_base64(message())}))),
    );
    started.notified().await;
    task.abort();
    let cancelled = task.await.unwrap_err().is_cancelled();
    let after_cancellation = available_observations(&confirmations);

    let observations = (0..64)
        .map(|_| {
            confirmations.register(
                StdAddr::new(0, HashBytes::ZERO),
                HashBytes::ZERO,
                WaitFor::Transaction,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let full_wait = response(
        router
            .clone()
            .oneshot(request(json!({"boc": Boc::encode_base64(message())})))
            .await?,
    )
    .await;
    let sent_while_full = broadcasts.load(Ordering::Relaxed);
    let ordinary = response(
        router
            .clone()
            .oneshot(
                Request::post("/api/send")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"boc": Boc::encode_base64(message())}).to_string(),
                    ))?,
            )
            .await?,
    )
    .await;
    drop(observations);
    let permit = capacity.clone().acquire_many_owned(16).await?;
    let full_send = response(
        router
            .clone()
            .oneshot(request(json!({"boc": Boc::encode_base64(message())})))
            .await?,
    )
    .await;
    drop(permit);
    confirmations.close();
    let closed = response(
        router
            .oneshot(request(json!({"boc": Boc::encode_base64(message())})))
            .await?,
    )
    .await;
    let summary = json!({
        "cancelled": cancelled,
        "observation_slots_after_cancellation": after_cancellation,
        "full_wait": full_wait, "sent_while_full": sent_while_full,
        "ordinary_send": ordinary, "full_send": full_send, "closed": closed,
        "broadcasts": broadcasts.load(Ordering::Relaxed),
        "submission_slots": capacity.available_permits(),
    });
    expect_file!["snapshots/capacity.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&summary)?));
    Ok(())
}

#[tokio::test]
async fn wait_route_rejects_invalid_inputs_before_broadcast() -> anyhow::Result<()> {
    let app = service(
        Confirmations::default(),
        Arc::new(|_| panic!("invalid input was broadcast")),
    )
    .router();
    let mut outcomes = Vec::new();
    for body in [
        json!({"boc":"?"}),
        json!({"boc":STANDARD.encode([0])}),
        json!({"boc": STANDARD.encode(vec![0; 65536])}),
        json!({"boc": Boc::encode_base64(message()), "timeout_ms": 999}),
        json!({"boc": Boc::encode_base64(message()), "timeout_ms": 120001}),
        json!({"boc": Boc::encode_base64(message()), "timeout_ms": "30000"}),
        json!({"boc": Boc::encode_base64(message()), "unexpected": true}),
        json!({}),
    ] {
        outcomes.push(response(app.clone().oneshot(request(body)).await?).await);
    }
    let oversized = Request::post("/api/sendAndWaitTransaction")
        .header("content-type", "application/json")
        .body(Body::from(" ".repeat(96 * 1024 + 1)))?;
    outcomes.push(response(app.oneshot(oversized).await?).await);
    expect_file!["snapshots/invalid.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&outcomes)?));
    Ok(())
}

#[tokio::test]
async fn past_batches_are_not_replayed_and_sse_failure_does_not_interrupt_waiting()
-> anyhow::Result<()> {
    let confirmations = Confirmations::default();
    let batch = committed(transaction(2)?, ShardIdent::BASECHAIN)?;
    confirmations.publish(&batch)?;
    let started = Arc::new(Notify::new());
    let signal = started.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            signal.notify_one();
            Box::pin(async { Ok(()) })
        }),
    )
    .router();
    let task = tokio::spawn(app.oneshot(request(json!({"boc": Boc::encode_base64(message())}))));
    tokio::time::timeout(Duration::from_secs(3), started.notified()).await?;
    let no_replay = !task.is_finished();
    crate::streaming::Subscriptions::default().fail();
    tokio::task::yield_now().await;
    let unaffected_by_sse = !task.is_finished();
    confirmations.publish(&batch)?;
    let reply = response(task.await??).await;
    expect![[r#"{
  "no_replay": true,
  "observation_slots": 64,
  "status": 200,
  "unaffected_by_sse": true
}"#]]
    .assert_eq(&serde_json::to_string_pretty(&json!({
        "no_replay": no_replay,
        "unaffected_by_sse": unaffected_by_sse,
        "status": reply["status"],
        "observation_slots": available_observations(&confirmations),
    }))?);
    Ok(())
}
