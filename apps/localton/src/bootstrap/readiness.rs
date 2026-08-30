//! Readiness checks and termination signals for a running local network.
//!
//! Process creation alone does not mean that TON is usable. The launcher polls
//! the liteserver until two distinct masterchain seqnos are observed while also
//! checking that every managed process remains alive. After startup, the same
//! registry is supervised until Ctrl-C, SIGTERM, or a child failure occurs.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::{signal, time::sleep};
use tracing::{debug, info};

use crate::{
    runtime::ProcessRegistry,
    storage::Layout,
    ton::tools::{
        lite_client::{LiteClient, LiteTarget},
        types::OperationContext,
    },
};

const LITE_QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const READINESS_PROGRESS_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(not(test))]
const READINESS_POLL_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(test)]
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Proves that the liteserver is reachable and the masterchain is advancing.
///
/// A single positive seqno could be stale state from an earlier run. The launcher
/// therefore waits for a later seqno greater than the first observation while
/// checking required child processes on every iteration. Success means both the
/// query path and ongoing block production work.
pub(super) async fn wait_for_blocks(
    layout: &Layout,
    lite_client: &dyn LiteClient,
    target: &LiteTarget,
    processes: &ProcessRegistry,
    timeout: Duration,
) -> Result<()> {
    info!("waiting for liteserver and masterchain block production");
    let started = tokio::time::Instant::now();
    let deadline = started + timeout;
    let mut next_progress = started;
    let mut first_seqno = None;
    let mut last_seqno = None;
    loop {
        processes.ensure_alive().await?;
        let last_error = match lite_client_seqno(lite_client, target).await {
            Ok(seqno) => {
                last_seqno = Some(seqno);
                match first_seqno {
                    None if seqno > 0 => first_seqno = Some(seqno),
                    Some(first) if seqno > first => {
                        info!(
                            first_seqno = first,
                            current_seqno = seqno,
                            "masterchain advanced"
                        );
                        return Ok(());
                    }
                    _ => {}
                }
                None
            }
            Err(error) => Some(format!("{error:#}")),
        };
        let now = tokio::time::Instant::now();
        if now >= next_progress {
            debug!(
                elapsed_ms = now.duration_since(started).as_millis() as u64,
                first_seqno, last_seqno, last_error, "masterchain readiness progress"
            );
            next_progress = now + READINESS_PROGRESS_INTERVAL;
        }
        if now >= deadline {
            let detail = last_error
                .map(|error| format!("; last liteserver error: {error}"))
                .unwrap_or_default();
            bail!(
                "masterchain did not advance within {}s{detail}; inspect {}",
                timeout.as_secs(),
                layout.logs.display()
            );
        }
        sleep(READINESS_POLL_INTERVAL).await;
    }
}

/// Queries the configured liteserver for its latest masterchain block number.
///
/// [`LiteTarget`] binds the query to the same trusted global configuration and
/// liteserver identity external clients use. This function requires structured
/// protocol data: a diagnostic-only official CLI backend cannot accidentally make
/// readiness depend on release-specific display text.
pub(super) async fn lite_client_seqno(
    lite_client: &dyn LiteClient,
    target: &LiteTarget,
) -> Result<u32> {
    let context = OperationContext {
        timeout: LITE_QUERY_TIMEOUT,
        node_name: target.label.clone(),
    };
    let info = tokio::time::timeout(
        LITE_QUERY_TIMEOUT,
        lite_client.masterchain_info(&context, target),
    )
    .await
    .context("masterchain info query timed out")??
    .into_data()
    .context("readiness requires structured masterchain info")?;
    Ok(info.last.seqno)
}

/// Keeps the launcher alive until one required child process exits.
///
/// The registry reports the process name and exit status as an error; the outer
/// pipeline then performs coordinated shutdown instead of leaving a partially
/// functioning network running.
pub(crate) async fn supervise(processes: &ProcessRegistry) -> Result<()> {
    loop {
        sleep(Duration::from_millis(250)).await;
        processes.ensure_alive().await?;
    }
}

/// Waits for the platform's normal interactive or service-manager stop signal.
///
/// Unix handles both Ctrl-C (`SIGINT`) and `SIGTERM`, which is used by Docker and
/// process supervisors. Returning normally sends execution through the same
/// cleanup path as a child-process failure.
pub(crate) async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("failed to install SIGTERM handler")?;
        tokio::select! {
            result = signal::ctrl_c() => {
                result.context("failed to install Ctrl-C handler")?;
                info!("Ctrl-C received, stopping all TON processes");
            }
            _ = terminate.recv() => {
                info!("SIGTERM received, stopping all TON processes");
            }
        }
    }
    #[cfg(not(unix))]
    {
        signal::ctrl_c()
            .await
            .context("failed to install Ctrl-C handler")?;
        info!("Ctrl-C received, stopping all TON processes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use anyhow::anyhow;
    use async_trait::async_trait;

    use crate::{
        ton::lite::{AccountInfo, BlockRef},
        ton::tools::lite_client::{
            AccountStateRequest, BlockData, BlockTransactions, BlockTransactionsRequest, Boc,
            ElectionStatus, LiteResponse, LookupBlock, MasterchainInfo, RunMethodRequest,
            RunMethodResult, SendBocResult,
        },
    };

    use super::*;

    struct ReadinessLiteClient {
        seqnos: Mutex<VecDeque<u32>>,
    }

    impl ReadinessLiteClient {
        fn new(seqnos: impl IntoIterator<Item = u32>) -> Self {
            Self {
                seqnos: Mutex::new(seqnos.into_iter().collect()),
            }
        }

        fn unexpected<T>() -> Result<T> {
            Err(anyhow!(
                "readiness invoked an unrelated liteserver operation"
            ))
        }
    }

    #[async_trait]
    impl LiteClient for ReadinessLiteClient {
        async fn masterchain_info(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
        ) -> Result<LiteResponse<MasterchainInfo>> {
            let seqno = self
                .seqnos
                .lock()
                .unwrap()
                .pop_front()
                .context("readiness requested more seqnos than expected")?;
            Ok(LiteResponse::Data(MasterchainInfo {
                last: block_ref(seqno),
            }))
        }

        async fn account_state(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _request: AccountStateRequest,
        ) -> Result<LiteResponse<AccountInfo>> {
            Self::unexpected()
        }

        async fn lookup_block(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _request: LookupBlock,
        ) -> Result<LiteResponse<BlockRef>> {
            Self::unexpected()
        }

        async fn block(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _request: LookupBlock,
        ) -> Result<LiteResponse<BlockData>> {
            Self::unexpected()
        }

        async fn download_block(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _id: BlockRef,
        ) -> Result<LiteResponse<BlockData>> {
            Self::unexpected()
        }

        async fn block_transactions(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _request: BlockTransactionsRequest,
        ) -> Result<LiteResponse<BlockTransactions>> {
            Self::unexpected()
        }

        async fn send_boc(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _message: Boc,
        ) -> Result<LiteResponse<SendBocResult>> {
            Self::unexpected()
        }

        async fn run_method(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
            _request: RunMethodRequest,
        ) -> Result<LiteResponse<RunMethodResult>> {
            Self::unexpected()
        }

        async fn election_status(
            &self,
            _context: &OperationContext,
            _target: &LiteTarget,
        ) -> Result<LiteResponse<ElectionStatus>> {
            Self::unexpected()
        }
    }

    fn block_ref(seqno: u32) -> BlockRef {
        BlockRef {
            workchain: -1,
            shard: "8000000000000000".to_owned(),
            seqno,
            root_hash: "11".repeat(32),
            file_hash: "22".repeat(32),
        }
    }

    #[tokio::test]
    async fn waits_for_a_second_distinct_masterchain_seqno() {
        let state = tempfile::tempdir().unwrap();
        let layout = Layout::new(state.path().join("state"));
        let target = LiteTarget::new(layout.global_config.clone()).with_label("genesis");
        let processes = ProcessRegistry::default();
        let client = ReadinessLiteClient::new([17, 17, 18]);

        wait_for_blocks(
            &layout,
            &client,
            &target,
            &processes,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert!(client.seqnos.lock().unwrap().is_empty());
    }
}
