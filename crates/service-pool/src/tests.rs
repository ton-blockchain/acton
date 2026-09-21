use std::sync::atomic::{AtomicUsize, Ordering};

use expect_test::expect;

use super::*;

struct Server(&'static str);

impl Endpoint for Server {
    fn id(&self) -> &str {
        self.0
    }

    fn address(&self) -> String {
        self.0.to_owned()
    }
}

fn pool(options: Options) -> Pool<Server> {
    let pool = Pool::new("test".into(), options).unwrap();
    for id in ["a", "b", "c"] {
        pool.upsert(Server(id));
    }
    pool
}

fn outcomes(pool: &Pool<Server>) -> String {
    let mut output = String::new();
    for endpoint in pool.snapshot().endpoints {
        for (class, metrics) in endpoint.classes {
            use std::fmt::Write;
            writeln!(
                &mut output,
                "{} {class}: success={} failed={} unavailable={} cancelled={} active={}",
                endpoint.id,
                metrics.successes,
                metrics.failures,
                metrics.unavailable,
                metrics.cancelled,
                endpoint.in_flight
            )
            .unwrap();
        }
    }
    output
}

#[tokio::test(start_paused = true)]
async fn unavailable_reply_does_not_win_or_reward_the_endpoint() {
    let pool = pool(Options::default());
    let winner = pool
        .execute("block", |server| async move {
            match server.id() {
                "a" => tokio::time::sleep(Duration::from_secs(10)).await,
                "b" => return Err(Failure::Unavailable("block is not applied yet".into())),
                _ => tokio::time::sleep(Duration::from_millis(20)).await,
            }
            Ok(server.id().to_owned())
        })
        .await
        .unwrap();

    // The subsequent operation reuses the measured winner without speculative work.
    let next = pool
        .execute("block", |server| async move { Ok(server.id().to_owned()) })
        .await
        .unwrap();
    expect![[r"
        c c
        a block: success=0 failed=0 unavailable=0 cancelled=1 active=0
        b block: success=0 failed=0 unavailable=1 cancelled=0 active=0
        c block: success=2 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!("{winner} {next}\n{}", outcomes(&pool)));
}

#[tokio::test(start_paused = true)]
async fn importing_profiles_keeps_newer_measurements_and_rejects_other_networks() {
    let original = pool(Options::default());
    original
        .execute_where("block", |server| server.id() == "a", |_| async { Ok(()) })
        .await
        .unwrap();
    let mut newer = original.snapshot();
    let metric = newer.endpoints[0].classes.get_mut("block").unwrap();
    metric.updated_at = 200;
    metric.latency_ms = Some(20.0);
    let mut older = original.snapshot();
    let metric = older.endpoints[0].classes.get_mut("block").unwrap();
    metric.updated_at = 100;
    metric.latency_ms = Some(700.0);

    let restored = pool(Options::default());
    restored.restore_snapshot(newer).unwrap();
    restored.restore_snapshot(older).unwrap();
    let mut foreign = original.snapshot();
    foreign.namespace = "another network".into();
    let error = restored.restore_snapshot(foreign).unwrap_err();
    expect![["mean=Some(20.0); operation failed: endpoint profile belongs to another pool or schema version"]].assert_eq(&format!(
        "mean={:?}; {error}", restored.snapshot().endpoints[0].classes["block"].latency_ms
    ));
}

#[tokio::test(start_paused = true)]
async fn calibration_visits_every_endpoint_and_excludes_connection_warmup() {
    let pool = pool(Options {
        max_in_flight: 2,
        attempt_timeout: Duration::from_secs(1),
        request_timeout: Duration::from_secs(2),
        ..Options::default()
    });
    pool.upsert(Server("d"));
    let calls: [AtomicUsize; 4] = std::array::from_fn(|_| AtomicUsize::new(0));

    pool.measure_all("block", 3, |server| {
        let index = match server.id() {
            "a" => 0,
            "b" => 1,
            "c" => 2,
            _ => 3,
        };
        let sample = calls[index].fetch_add(1, Ordering::Relaxed);
        async move {
            let delay = match (index, sample) {
                (3, _) => 10_000,
                (0, 0) => 900,
                (1, 0) => 40,
                (2, 0) => 800,
                (0, sample) => 30 + sample as u64 * 10,
                (1, sample) => 50 + sample as u64 * 10,
                (_, sample) => sample as u64 * 10,
            };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(())
        }
    })
    .await
    .unwrap();

    let mut report = String::new();
    for endpoint in pool.snapshot().endpoints {
        use std::fmt::Write;
        let metrics = &endpoint.classes["block"];
        writeln!(
            &mut report,
            "{} mean={:?} successes={} failures={}",
            endpoint.id, metrics.latency_ms, metrics.successes, metrics.failures
        )
        .unwrap();
    }
    let winner = pool
        .execute(
            "block",
            |endpoint| async move { Ok(endpoint.id().to_owned()) },
        )
        .await
        .unwrap();
    expect![[r"
        a mean=Some(50.0) successes=4 failures=0
        b mean=Some(70.0) successes=4 failures=0
        c mean=Some(20.0) successes=4 failures=0
        d mean=None successes=0 failures=1
        winner=c
    "]]
    .assert_eq(&format!("{report}winner={winner}\n"));
}

#[tokio::test(start_paused = true)]
async fn latency_averages_completed_responses_and_tracks_cancellation_separately() {
    let pool = Pool::new("test".into(), Options::default()).unwrap();
    pool.upsert(Server("a"));

    for delay in [100, 700] {
        pool.execute("block", |_| async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(())
        })
        .await
        .unwrap();
    }

    let after_spike = pool.snapshot().endpoints[0].classes["block"].clone();
    let cancelled = tokio::time::timeout(
        Duration::from_millis(400),
        pool.execute("block", |_| std::future::pending::<Result<(), Failure>>()),
    )
    .await;
    let after_cancellation = pool.snapshot().endpoints[0].classes["block"].clone();

    pool.execute("block", |_| async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    })
    .await
    .unwrap();
    let recovered = pool.snapshot().endpoints[0].classes["block"].clone();

    expect![[r"
        spike: mean=220 bound=None
        cancelled=true mean=220 bound=Some(400.0)
        recovered: mean=196 bound=None successes=3 cancelled=1
    "]]
    .assert_eq(&format!(
        "spike: mean={:.0} bound={:?}\n\
         cancelled={} mean={:.0} bound={:?}\n\
         recovered: mean={:.0} bound={:?} successes={} cancelled={}\n",
        after_spike.latency_ms.unwrap(),
        after_spike.latency_lower_bound_ms,
        cancelled.is_err(),
        after_cancellation.latency_ms.unwrap(),
        after_cancellation.latency_lower_bound_ms,
        recovered.latency_ms.unwrap(),
        recovered.latency_lower_bound_ms,
        recovered.successes,
        recovered.cancelled,
    ));
}

#[tokio::test(start_paused = true)]
async fn cancelled_fast_endpoints_recover_without_blocking_discovery() {
    let pool = pool(Options::default());
    let mut saved = pool.snapshot();
    for endpoint in &mut saved.endpoints {
        let latency = match endpoint.id.as_str() {
            "a" => 75.0,
            "c" => 5.0,
            _ => continue,
        };
        endpoint.classes.insert(
            "block".into(),
            Metrics {
                successes: 4,
                latency_ms: Some(latency),
                latency_lower_bound_ms: (endpoint.id == "c").then_some(600.0),
                updated_at: unix_seconds(),
                ..Metrics::default()
            },
        );
    }
    pool.restore_snapshot(saved).unwrap();

    let request = |server: Arc<Server>| async move {
        let delay = match server.id() {
            "a" => 75,
            "b" => 30,
            _ => 5,
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        Ok(server.id().to_owned())
    };

    let first = pool.execute_with_probes("block", request).await.unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(10)).await;
    tokio::task::yield_now().await;
    let recovered = pool.execute_with_probes("block", request).await.unwrap();

    // Discovery runs independently of recovery and can continue its sweep.
    tokio::time::advance(Duration::from_secs(1)).await;
    pool.execute_with_probes("block", request).await.unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(40)).await;
    tokio::task::yield_now().await;

    expect![[r"
        a c
        a block: success=5 failed=0 unavailable=0 cancelled=0 active=1
        b block: success=1 failed=0 unavailable=0 cancelled=0 active=0
        c block: success=7 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!("{first} {recovered}\n{}", outcomes(&pool)));
}

#[tokio::test(start_paused = true)]
async fn slow_discovery_does_not_delay_recovery_of_a_cancelled_endpoint() {
    let pool = pool(Options::default());
    for (id, delay) in [("a", 5), ("c", 75)] {
        pool.execute_where(
            "block",
            |server| server.id() == id,
            |_| async {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                Ok(())
            },
        )
        .await
        .unwrap();
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let request = move |server: Arc<Server>| {
        let delay = match server.id() {
            "a" if calls.fetch_add(1, Ordering::Relaxed) == 1 => 10_000,
            "a" => 5,
            "b" => 3_000,
            _ => 75,
        };
        async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(server.id().to_owned())
        }
    };

    let first = pool
        .execute_with_probes("block", request.clone())
        .await
        .unwrap();
    let hedged = pool
        .execute_with_probes("block", request.clone())
        .await
        .unwrap();
    tokio::time::advance(Duration::from_secs(1)).await;
    pool.execute_with_probes("block", request.clone())
        .await
        .unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(10)).await;
    tokio::task::yield_now().await;
    let recovered = pool.execute_with_probes("block", request).await.unwrap();

    expect![[r"a c a; discovery_active=1; recovery_bound=None"]].assert_eq(&format!(
        "{first} {hedged} {recovered}; discovery_active={}; recovery_bound={:?}",
        pool.snapshot().endpoints[1].in_flight,
        pool.snapshot().endpoints[0].classes["block"].latency_lower_bound_ms,
    ));
}

#[tokio::test(start_paused = true)]
async fn another_class_can_recover_a_stale_successful_latency_estimate() {
    let pool = pool(Options::default());
    let mut saved = pool.snapshot();
    for (index, class, latency) in [
        (0, "block", 100.0),
        (0, "metadata", 5.0),
        (1, "block", 20.0),
    ] {
        saved.endpoints[index].classes.insert(
            class.into(),
            Metrics {
                latency_ms: Some(latency),
                ..Metrics::default()
            },
        );
    }
    pool.restore_snapshot(saved).unwrap();

    let mut winners = Vec::new();
    for _ in 0..12 {
        let winner = pool
            .execute_with_probes("block", |server| async move {
                let delay = match server.id() {
                    "a" => 5,
                    "b" => 20,
                    _ => 3_000,
                };
                tokio::time::sleep(Duration::from_millis(delay)).await;
                Ok(server.id().to_owned())
            })
            .await
            .unwrap();
        winners.push(winner);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    expect![["b b b b b b b b b a a a"]].assert_eq(&winners.join(" "));
}

#[tokio::test(start_paused = true)]
async fn pacing_rotates_fast_endpoints_and_shares_admission_across_clones_and_classes() {
    let pool = pool(Options {
        min_request_interval: Duration::from_millis(40),
        request_timeout: Duration::from_millis(100),
        ..Options::default()
    });
    let mut saved = pool.snapshot();
    for (endpoint, latency) in saved.endpoints.iter_mut().zip([5.0, 6.0, 75.0]) {
        endpoint.classes.insert(
            "block".into(),
            Metrics {
                latency_ms: Some(latency),
                ..Metrics::default()
            },
        );
    }
    pool.restore_snapshot(saved).unwrap();

    let started = Instant::now();
    let mut dispatches = Vec::new();
    for _ in 0..6 {
        let entry = pool
            .clone()
            .execute("block", |server| async move {
                let dispatched = started.elapsed().as_millis();
                let delay = if server.id() == "a" { 5 } else { 6 };
                tokio::time::sleep(Duration::from_millis(delay)).await;
                Ok(format!("{}@{dispatched}", server.id()))
            })
            .await
            .unwrap();
        dispatches.push(entry);
    }

    let other_class = pool
        .execute_where(
            "shard",
            |server| server.id() == "a",
            |_| async { Ok(started.elapsed().as_millis()) },
        )
        .await
        .unwrap();
    let timeout = tokio::time::timeout(
        Duration::from_millis(10),
        pool.execute_where("block", |server| server.id() == "a", |_| async { Ok(()) }),
    )
    .await;

    expect![["a@0 b@5 a@40 b@45 a@80 b@85; shard@120; admission_timed_out=true; active=0"]]
        .assert_eq(&format!(
            "{}; shard@{other_class}; admission_timed_out={}; active={}",
            dispatches.join(" "),
            timeout.is_err(),
            pool.snapshot().endpoints[0].in_flight
        ));
}

#[tokio::test(start_paused = true)]
async fn a_formerly_fast_endpoint_loses_its_rank_when_it_slows_down() {
    let pool = pool(Options::default());
    pool.execute("block", |_| async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(())
    })
    .await
    .unwrap();

    let winner = pool
        .execute("block", |server| async move {
            let delay = if server.id() == "a" { 10_000 } else { 20 };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(server.id().to_owned())
        })
        .await
        .unwrap();
    let next = pool
        .execute("block", |server| async move { Ok(server.id().to_owned()) })
        .await
        .unwrap();
    expect![["b b"]].assert_eq(&format!("{winner} {next}"));
}

#[tokio::test(start_paused = true)]
async fn unknown_endpoints_do_not_outrank_a_measured_slow_service() {
    let pool = pool(Options::default());
    pool.execute("block", |_| async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    })
    .await
    .unwrap();

    let started = Instant::now();
    let winner = pool
        .execute("block", |server| async move {
            let delay = if server.id() == "a" { 300 } else { 10_000 };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok(server.id().to_owned())
        })
        .await
        .unwrap();
    expect![["a 300ms"]].assert_eq(&format!("{winner} {:?}", started.elapsed()));
}

#[tokio::test(start_paused = true)]
async fn hedged_requests_keep_exploring_each_request_class() {
    let pool = pool(Options::default());
    pool.remove("c");

    for class in ["masterchain", "shard"] {
        for id in ["a", "b"] {
            pool.execute_where(
                class,
                |server| server.id() == id,
                |_| async {
                    tokio::time::sleep(Duration::from_millis(180)).await;
                    Ok(())
                },
            )
            .await
            .unwrap();
        }
    }

    // Both measured servers respond after the hedge delay. The new server is
    // faster, but cannot win a backup attempt with a 100 ms head start to overcome.
    pool.upsert(Server("c"));
    let mut final_requests = String::new();

    for round in 0..16 {
        for class in ["masterchain", "shard"] {
            let started = Instant::now();
            let winner = pool
                .execute_with_probes(class, |server| async move {
                    let delay = if server.id() == "c" { 120 } else { 180 };
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    Ok(server.id().to_owned())
                })
                .await
                .unwrap();

            if round == 15 {
                use std::fmt::Write;
                writeln!(
                    &mut final_requests,
                    "{class}: {winner} {:?}",
                    started.elapsed()
                )
                .unwrap();
            }
        }
    }

    expect![[r"
        masterchain: c 120ms
        shard: c 120ms
    "]]
    .assert_eq(&final_requests);
}

#[tokio::test(start_paused = true)]
async fn background_measurements_survive_foreground_winners_but_not_pool_shutdown() {
    let pool = pool(Options::default());
    let request = |server: Arc<Server>| async move {
        let delay = match server.id() {
            "a" => 10,
            "b" => 2_000,
            _ => 60_000,
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        Ok(server.id().to_owned())
    };

    let started = Instant::now();
    let first = pool.execute_with_probes("block", request).await.unwrap();
    let second = pool.execute_with_probes("block", request).await.unwrap();
    let foreground_time = started.elapsed();
    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;

    expect![[r"
        a a foreground=20ms
        a block: success=2 failed=0 unavailable=0 cancelled=0 active=0
        b block: success=1 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!(
        "{first} {second} foreground={foreground_time:?}\n{}",
        outcomes(&pool)
    ));

    pool.execute_with_probes("block", request).await.unwrap();
    tokio::task::yield_now().await;
    let state = Arc::downgrade(&pool.inner);
    drop(pool);
    tokio::task::yield_now().await;

    expect![["false"]].assert_eq(&state.upgrade().is_some().to_string());
}

#[tokio::test(start_paused = true)]
async fn a_new_request_class_uses_reachable_endpoints_until_it_has_its_own_measurements() {
    let pool = pool(Options::default());
    pool.execute_where(
        "metadata",
        |server| server.id() == "c",
        |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(())
        },
    )
    .await
    .unwrap();

    let first = pool
        .execute("block", |server| async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok(server.id().to_owned())
        })
        .await
        .unwrap();
    pool.execute_where(
        "metadata",
        |server| server.id() == "a",
        |_| async { Ok(()) },
    )
    .await
    .unwrap();
    let second = pool
        .execute("block", |server| async move { Ok(server.id().to_owned()) })
        .await
        .unwrap();

    expect![["c c"]].assert_eq(&format!("{first} {second}"));
}

#[tokio::test(start_paused = true)]
async fn invalid_response_is_retried_and_fatal_errors_stop_retries() {
    let pool = pool(Options::default());
    let winner = pool
        .execute("block", |server| async move {
            if server.id() == "a" {
                Err(Failure::Invalid("hash mismatch".into()))
            } else {
                Ok(server.id().to_owned())
            }
        })
        .await
        .unwrap();

    let fatal = pool
        .execute("block", |_| async {
            Err::<(), _>(Failure::Fatal("disk full".into()))
        })
        .await
        .unwrap_err();
    expect![[r"
        b operation failed: disk full
        a block: success=0 failed=1 unavailable=0 cancelled=0 active=0
        b block: success=1 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!("{winner} {fatal}\n{}", outcomes(&pool)));
}

#[tokio::test(start_paused = true)]
async fn a_concurrent_success_does_not_cancel_an_invalid_response_suspension() {
    let pool = pool(Options {
        request_timeout: Duration::from_secs(90),
        ..Options::default()
    });
    let started = Instant::now();
    let bad = pool.execute_where(
        "block",
        |server| server.id() == "a",
        |_| async {
            tokio::time::sleep(Duration::from_millis(1)).await;
            Err::<(), _>(Failure::Invalid("hash mismatch".into()))
        },
    );
    let good = pool.execute_where(
        "metadata",
        |server| server.id() == "a",
        |_| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        },
    );
    let (bad, good) = tokio::join!(bad, good);
    good.unwrap();
    let error = bad.unwrap_err();
    pool.execute_where("block", |server| server.id() == "a", |_| async { Ok(()) })
        .await
        .unwrap();
    expect![["invalid response: hash mismatch; recovered after 60.001s"]]
        .assert_eq(&format!("{error}; recovered after {:?}", started.elapsed()));
}

#[tokio::test(start_paused = true)]
async fn cloned_pools_share_admission_and_cancellation_releases_it() {
    let pool = pool(Options {
        max_in_flight: 2,
        max_in_flight_per_endpoint: 1,
        ..Options::default()
    });
    let started = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for class in ["metadata", "block", "proof"] {
        let pool = pool.clone();
        let started = Arc::clone(&started);
        tasks.push(tokio::spawn(async move {
            pool.execute(class, |_| {
                started.fetch_add(1, Ordering::Relaxed);
                std::future::pending::<Result<(), Failure>>()
            })
            .await
        }));
    }
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(300)).await;
    tokio::task::yield_now().await;
    let active: Vec<_> = pool
        .snapshot()
        .endpoints
        .iter()
        .map(|server| server.in_flight)
        .collect();
    expect![["2 [1, 1, 0]"]].assert_eq(&format!("{} {active:?}", started.load(Ordering::Relaxed)));

    for task in &tasks {
        task.abort();
    }
    for task in tasks {
        let _ = task.await;
    }
    let active: usize = pool
        .snapshot()
        .endpoints
        .iter()
        .map(|server| server.in_flight)
        .sum();
    let cancelled: u64 = pool
        .snapshot()
        .endpoints
        .iter()
        .flat_map(|server| server.classes.values())
        .map(|metrics| metrics.cancelled)
        .sum();
    let result = pool
        .execute("after_cancel", |_| async { Ok(42) })
        .await
        .unwrap();
    expect![["active=0 cancelled=2 result=42"]].assert_eq(&format!(
        "active={active} cancelled={cancelled} result={result}"
    ));
}

#[tokio::test(start_paused = true)]
async fn admission_waits_share_the_request_deadline() {
    let pool = pool(Options {
        max_in_flight: 1,
        max_parallel_attempts: 1,
        request_timeout: Duration::from_secs(1),
        ..Options::default()
    });
    let first = pool.execute("block", |_| std::future::pending::<Result<(), Failure>>());
    let second = pool.execute("metadata", |_| {
        std::future::pending::<Result<(), Failure>>()
    });
    let (first, second) = tokio::join!(first, second);
    expect![[r"
        request failed: block deadline exceeded after 1 attempts; last result: data unavailable: no eligible endpoints
        request failed: metadata deadline exceeded after 0 attempts; last result: data unavailable: no eligible endpoints
    "]].assert_eq(&format!("{}\n{}\n", first.unwrap_err(), second.unwrap_err()));
}

#[tokio::test(start_paused = true)]
async fn cooldown_recovers_and_eligibility_is_respected() {
    let pool = pool(Options::default());
    let failure = pool
        .execute_where(
            "block",
            |server| server.id() == "a",
            |_| async { Err::<(), _>(Failure::Retryable("connection closed".into())) },
        )
        .await
        .unwrap_err();

    let started = Instant::now();
    let recovered = pool
        .execute_where(
            "block",
            |server| server.id() == "a",
            |server| async move { Ok(server.id().to_owned()) },
        )
        .await
        .unwrap();
    expect![["request failed: connection closed; recovered=a after=2s"]].assert_eq(&format!(
        "{failure}; recovered={recovered} after={:?}",
        started.elapsed()
    ));
}

#[tokio::test]
async fn restart_restores_ranking_but_not_admission_or_other_namespaces() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("peers.json");
    let mut original = pool(Options::default());
    original.persist_to(&path).unwrap();
    original
        .execute_where("block", |server| server.id() == "c", |_| async { Ok(()) })
        .await
        .unwrap();
    original.flush().await.unwrap();

    // A long shutdown must not erase the only measured route and force a cold search.
    let mut saved: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for endpoint in &mut saved.endpoints {
        for metrics in endpoint.classes.values_mut() {
            metrics.updated_at = unix_seconds().saturating_sub(86_400);
        }
    }
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut restored = pool(Options::default());
    restored.persist_to(&path).unwrap();
    let winner = restored
        .execute("block", |server| async move { Ok(server.id().to_owned()) })
        .await
        .unwrap();
    let mut different = Pool::<Server>::new("other network".into(), Options::default()).unwrap();
    different.persist_to(&path).unwrap();
    let ignored = different.snapshot().endpoints.len();
    expect![[r"
        c other_pool_entries=0
        c block: success=2 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!(
        "{winner} other_pool_entries={ignored}\n{}",
        outcomes(&restored)
    ));
}

#[tokio::test(start_paused = true)]
async fn retries_are_bounded() {
    let pool = pool(Options {
        max_attempts: 2,
        ..Options::default()
    });
    let error = pool
        .execute("missing", |_| async {
            Err::<(), _>(Failure::Unavailable("missing".into()))
        })
        .await
        .unwrap_err();
    let winner = pool
        .execute("block", |server| async move { Ok(server.id().to_owned()) })
        .await
        .unwrap();
    expect![[r"
        data unavailable: missing winner=c
        a missing: success=0 failed=0 unavailable=1 cancelled=0 active=0
        b missing: success=0 failed=0 unavailable=1 cancelled=0 active=0
        c block: success=1 failed=0 unavailable=0 cancelled=0 active=0
    "]]
    .assert_eq(&format!("{error} winner={winner}\n{}", outcomes(&pool)));
}
