//! Structured progress reporting for multi-step Localton workflows
//!
//! Tool adapters already report subprocess-level timing. This module adds the
//! workflow boundary above them so an operator can distinguish a slow external
//! program from time spent preparing artifacts, waiting for readiness, or
//! coordinating several tools

use std::{future::Future, time::Instant};

use anyhow::Result;
use tracing::{Instrument, field::Empty, info, info_span, warn};

/// Runs one named workflow stage with stable start, duration, and outcome events
///
/// Names are required to be static strings so telemetry cardinality stays bounded
/// even when hundreds of nodes are active. The returned error remains unchanged,
/// but it is not copied into tracing: lower-level tool diagnostics can contain
/// message payloads or release output that must not become searchable log fields
pub async fn run_stage<T, F>(
    workflow: &'static str,
    stage: &'static str,
    target: &str,
    future: F,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    let started = Instant::now();
    let span = info_span!(
        "workflow_stage",
        workflow,
        stage,
        target,
        duration_ms = Empty,
        outcome = Empty,
    );
    span.in_scope(|| info!(milestone = "started", "workflow stage started"));
    let result = future.instrument(span.clone()).await;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    span.in_scope(|| {
        if result.is_ok() {
            info!(
                milestone = "completed",
                duration_ms, outcome, "workflow stage completed"
            );
        } else {
            warn!(
                milestone = "failed",
                duration_ms, outcome, "workflow stage failed"
            );
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::run_stage;

    #[tokio::test]
    async fn preserves_success_and_error_values() {
        let value = run_stage("test", "success", "node", async {
            Ok::<_, anyhow::Error>(7)
        })
        .await
        .unwrap();
        assert_eq!(value, 7);

        let error = run_stage("test", "failure", "node", async {
            Err::<(), _>(anyhow!("diagnostic remains on the returned error"))
        })
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "diagnostic remains on the returned error"
        );
    }
}
