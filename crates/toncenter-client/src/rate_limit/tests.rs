use super::*;

#[tokio::test(start_paused = true)]
async fn shared_quota_spaces_requests_without_idle_bursts() {
    let gate = Gate::shared("spacing".to_owned(), Duration::from_secs(1));
    let other = Gate::shared("spacing".to_owned(), Duration::from_secs(2));
    gate.acquire().await;
    let started = Instant::now();
    other.acquire().await;
    assert_eq!(started.elapsed(), Duration::from_secs(2));
    tokio::time::advance(Duration::from_secs(10)).await;
    gate.acquire().await;
    let started = Instant::now();
    other.acquire().await;
    assert_eq!(started.elapsed(), Duration::from_secs(2));
}

#[tokio::test(start_paused = true)]
async fn cancellation_consumes_no_slot_and_cooldown_extends_waiters() {
    let gate = Gate::shared("cancellation".to_owned(), Duration::from_secs(1));
    gate.acquire().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), gate.acquire())
            .await
            .is_err()
    );
    let started = Instant::now();
    let waiting = Arc::clone(&gate);
    let waiter = tokio::spawn(async move { waiting.acquire().await });
    tokio::task::yield_now().await;
    gate.postpone(started + Duration::from_secs(3));
    waiter.await.unwrap();
    assert_eq!(started.elapsed(), Duration::from_secs(3));
}
