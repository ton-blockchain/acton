use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::time::Instant;

#[cfg(test)]
mod tests;

/// Shared pacing and cooldown for callers that consume the same quota.
/// A cancelled waiter consumes no slot; idle time does not accumulate permits.
pub(crate) struct Gate {
    state: Mutex<State>,
}

struct State {
    interval: Duration,
    next: Instant,
    last: Option<Instant>,
}

static GROUPS: LazyLock<Mutex<HashMap<String, Arc<Gate>>>> = LazyLock::new(Mutex::default);

impl Gate {
    pub(crate) fn shared(group: String, interval: Duration) -> Arc<Self> {
        let mut groups = GROUPS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        groups.retain(|_, gate| {
            Arc::strong_count(gate) > 1
                || gate
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .next
                    > now
        });
        let gate = Arc::clone(groups.entry(group).or_insert_with(|| {
            Arc::new(Self {
                state: Mutex::new(State {
                    interval,
                    next: now,
                    last: None,
                }),
            })
        }));
        drop(groups);
        let mut state = gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.interval = state.interval.max(interval);
        if let Some(last) = state.last {
            state.next = state.next.max(last + state.interval);
        }
        drop(state);
        gate
    }

    pub(crate) async fn acquire(&self) {
        loop {
            let next = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let now = Instant::now();
                if now >= state.next {
                    state.next = now + state.interval;
                    state.last = Some(now);
                    return;
                }
                state.next
            };
            tokio::time::sleep_until(next).await;
        }
    }

    pub(crate) fn postpone(&self, until: Instant) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.next = state.next.max(until);
    }
}
