//! Validator-engine initialization, configuration, and console operations.
//!
//! Each local validator has an engine database, an ADNL identity, a control
//! console, and optionally a liteserver. This module supplies typed engine and
//! console operations in workflow order, installs service keys into the generated
//! config, and owns retries and temporary-process restarts needed to register
//! permanent validator identities.

use std::{
    future::Future,
    net::Ipv4Addr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use tokio::{net::TcpStream, time::sleep};
use tracing::{info, warn};

use crate::{
    runtime::ServiceHandle,
    storage::Layout,
    storage::NodeSettings,
    ton::tools::{
        types::{AdnlEndpoint, GeneratedKey, OperationContext},
        validator_console::{
            AddAdnl, AddPermanentKey, AddTemporaryKey, AddValidatorAddress, ChangeFullNodeAddress,
            ImportPrivateKey, ValidatorConsole, ValidatorConsoleEndpoint,
        },
        validator_engine::{
            ValidatorBootstrapRequest, ValidatorDatabase, ValidatorEngine, ValidatorLogPaths,
            ValidatorRetentionPolicy, ValidatorStartRequest,
        },
    },
};

const YEAR_SECONDS: u64 = 365 * 24 * 60 * 60;
const CONSOLE_OPERATION_TIMEOUT: Duration = Duration::from_secs(15);
const CONSOLE_READINESS_TIMEOUT: Duration = Duration::from_secs(2);
const CONSOLE_ENDPOINT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CONSOLE_RETRY_LIMIT: usize = 5;
const IMPORT_RETRY_LIMIT: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);
const READINESS_LOG_INTERVAL: Duration = Duration::from_secs(5);

/// Registers the identities that let the genesis validator produce blocks.
///
/// The workflow owns ordering, bounded retries, and temporary-engine lifecycle.
/// Each individual key or address mutation crosses the typed console boundary
/// exactly once per attempt. This avoids retrying non-idempotent `new_key` while
/// retaining recovery for mutations that can race a keyring reload.
pub(super) async fn configure_genesis_identity(
    layout: &Layout,
    engine: &dyn ValidatorEngine,
    console: &dyn ValidatorConsole,
    node: &NodeSettings,
    validator: &GeneratedKey,
    context: &OperationContext,
) -> Result<()> {
    let started = std::time::Instant::now();
    workflow_stage(node, "configure_genesis_identity", "starting", "pending");
    let endpoint = console_endpoint(layout, node);
    let console_context = operation_context(context, node, CONSOLE_OPERATION_TIMEOUT);
    let validator_key = validator.id;
    let mut temporary = start_bootstrap(layout, engine, node).await?;
    let result = async {
        wait_for_console(layout, console, node, &mut temporary, context).await?;

        // Key creation is non-idempotent: a failed response must be surfaced rather
        // than hidden by a retry that silently creates a different identity.
        workflow_stage(node, "create_full_node_key", "executing", "pending");
        let node_key = console.new_key(&console_context, &endpoint).await?;
        let _public = console
            .export_public(&console_context, &endpoint, &node_key)
            .await?;

        workflow_stage(node, "create_validator_adnl", "executing", "pending");
        let validator_adnl = console.new_key(&console_context, &endpoint).await?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let end = u32::try_from(now.saturating_add(YEAR_SECONDS))
            .context("validator bootstrap key expiry exceeds TON uint32 time")?;

        retry_console_mutation(node, "add_permanent_key", || {
            console.add_permanent_key(
                &console_context,
                &endpoint,
                AddPermanentKey {
                    key: validator_key,
                    election_id: 0,
                    expire_at: end,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "add_temporary_key", || {
            console.add_temporary_key(
                &console_context,
                &endpoint,
                AddTemporaryKey {
                    permanent_key: validator_key,
                    temporary_key: validator_key,
                    expire_at: end,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "add_validator_adnl", || {
            console.add_adnl(
                &console_context,
                &endpoint,
                AddAdnl {
                    key: validator_adnl,
                    category: 0,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "add_permanent_key_adnl", || {
            console.add_adnl(
                &console_context,
                &endpoint,
                AddAdnl {
                    key: validator_key,
                    category: 0,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "bind_validator_address", || {
            console.add_validator_address(
                &console_context,
                &endpoint,
                AddValidatorAddress {
                    validator_key,
                    adnl_key: validator_adnl,
                    expire_at: end,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "add_full_node_adnl", || {
            console.add_adnl(
                &console_context,
                &endpoint,
                AddAdnl {
                    key: node_key,
                    category: 0,
                },
            )
        })
        .await?;
        retry_console_mutation(node, "change_full_node_address", || {
            console.change_full_node_address(
                &console_context,
                &endpoint,
                ChangeFullNodeAddress { adnl_key: node_key },
            )
        })
        .await?;

        // The official adapter normalizes TON v2026.06's successful disconnect
        // after `changefullnodeaddr`. The process itself may still terminate with
        // the associated ADNL unsubscribe error, so the workflow probes it and
        // owns the required restart before importing the permanent private key.
        if wait_for_console(layout, console, node, &mut temporary, context)
            .await
            .is_err()
        {
            warn!(
                "ton.tool" = "validator-bootstrap",
                operation = "configure_genesis_identity",
                node = %node.name,
                stage = "post_full_node_identity",
                retry_attempt = 1,
                progress = "restarting_engine",
                outcome = "retrying",
                "temporary validator stopped after changing its full-node identity"
            );
            temporary.stop().await?;
            temporary = start_bootstrap(layout, engine, node).await?;
            wait_for_console(layout, console, node, &mut temporary, context).await?;
        }

        import_validator_key(
            layout,
            engine,
            console,
            node,
            validator,
            context,
            &mut temporary,
        )
        .await
    }
    .await;
    temporary.stop().await?;
    workflow_result(node, "configure_genesis_identity", started, &result);
    result
}

/// Starts one initialized node with its persisted synchronization and retention
/// policy.
///
/// Success means only that the service was spawned. The bootstrap workflow remains
/// responsible for registry ownership, console readiness, synchronization, and
/// shutdown.
pub(super) async fn start_persistent(
    layout: &Layout,
    engine: &dyn ValidatorEngine,
    node: &NodeSettings,
    database: ValidatorDatabase,
) -> Result<ServiceHandle> {
    let node_layout = layout.node(node);
    engine
        .start_persistent(ValidatorStartRequest {
            node_name: node.name.clone(),
            global_config: node_layout.global_config,
            database,
            logs: ValidatorLogPaths {
                engine: node_layout.logs.join("validator-engine"),
                stdout: node_layout.logs.join("validator.stdout.log"),
                stderr: node_layout.logs.join("validator.stderr.log"),
            },
            endpoint: AdnlEndpoint::new(node.public_ip, node.adnl_port),
            threads: node.threads,
            verbosity: node.verbosity,
            retention: ValidatorRetentionPolicy {
                sync_before_seconds: node.sync_before_seconds,
                state_ttl_seconds: node.state_ttl_seconds,
                block_ttl_seconds: node.block_ttl_seconds,
                archive_ttl_seconds: node.archive_ttl_seconds,
                key_proof_ttl_seconds: node.key_proof_ttl_seconds,
            },
            initial_sync_delay: Duration::ZERO,
        })
        .await
}

/// Imports the generated validator private key with bounded engine restarts.
///
/// `importf` is not reliably accepted by a freshly mutated bootstrap engine on
/// every supported TON build. Each failed attempt restarts the temporary service,
/// waits for authenticated readiness, and then retries the same intended import.
/// Neither the key path nor any command text is emitted in workflow telemetry.
async fn import_validator_key(
    layout: &Layout,
    engine: &dyn ValidatorEngine,
    console: &dyn ValidatorConsole,
    node: &NodeSettings,
    validator: &GeneratedKey,
    context: &OperationContext,
    temporary: &mut ServiceHandle,
) -> Result<()> {
    let endpoint = console_endpoint(layout, node);
    let console_context = operation_context(context, node, CONSOLE_OPERATION_TIMEOUT);
    let private_key = layout
        .validator_keyring
        .join(validator.id.to_keyring_filename());
    let mut last_error = None;
    for attempt in 1..=IMPORT_RETRY_LIMIT {
        workflow_retry(
            node,
            "import_private_key",
            attempt,
            IMPORT_RETRY_LIMIT,
            "attempting",
            "pending",
            false,
        );
        match console
            .import_private_key(
                &console_context,
                &endpoint,
                ImportPrivateKey {
                    private_key: private_key.clone(),
                },
            )
            .await
        {
            Ok(()) => {
                workflow_retry(
                    node,
                    "import_private_key",
                    attempt,
                    IMPORT_RETRY_LIMIT,
                    "complete",
                    "success",
                    false,
                );
                return Ok(());
            }
            Err(error) => {
                workflow_retry(
                    node,
                    "import_private_key",
                    attempt,
                    IMPORT_RETRY_LIMIT,
                    "restarting_engine",
                    "retrying",
                    true,
                );
                last_error = Some(error);
                temporary.stop().await?;
                *temporary = start_bootstrap(layout, engine, node).await?;
                wait_for_console(layout, console, node, temporary, context).await?;
            }
        }
    }
    Err(last_error.context("validator key import failed without an error")?)
}

/// Waits until a live service answers a typed, authenticated health request.
///
/// A TCP listener check precedes the official console executable because that
/// executable waits for its complete transport timeout after an initial
/// connection refusal; it does not reconnect when validator-engine opens the
/// port moments later. The cheap poll prevents each normal engine start from
/// adding ten seconds while the authenticated health request still proves that
/// the expected control key, not merely an unrelated listener, is available.
pub(super) async fn wait_for_console(
    layout: &Layout,
    console: &dyn ValidatorConsole,
    node: &NodeSettings,
    process: &mut ServiceHandle,
    context: &OperationContext,
) -> Result<()> {
    let endpoint = console_endpoint(layout, node);
    let deadline = tokio::time::Instant::now() + context.timeout;
    let mut next_progress_log = tokio::time::Instant::now();
    let mut last_probe_error = None;
    let mut endpoint_attempt = 0_usize;

    loop {
        endpoint_attempt += 1;
        if let Some(status) = process.try_status()? {
            bail!("temporary validator-engine exited early with {status}");
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let message = format!(
                "validator-engine console endpoint was not ready within {}s",
                context.timeout.as_secs()
            );
            return match last_probe_error {
                Some(error) => Err(error).context(message),
                None => Err(anyhow::anyhow!(message)),
            };
        }
        if endpoint_attempt == 1 || now >= next_progress_log {
            workflow_retry(
                node,
                "console_endpoint",
                endpoint_attempt,
                0,
                "waiting",
                "pending",
                false,
            );
            next_progress_log = now + READINESS_LOG_INTERVAL;
        }
        match tokio::time::timeout(
            CONSOLE_ENDPOINT_POLL_INTERVAL,
            TcpStream::connect(endpoint.address),
        )
        .await
        {
            Ok(Ok(stream)) => {
                drop(stream);
                workflow_retry(
                    node,
                    "console_endpoint",
                    endpoint_attempt,
                    0,
                    "ready",
                    "success",
                    false,
                );
                break;
            }
            Ok(Err(error)) => last_probe_error = Some(error.into()),
            Err(error) => last_probe_error = Some(error.into()),
        }
        sleep(CONSOLE_ENDPOINT_POLL_INTERVAL.min(deadline - now)).await;
    }

    let mut attempt = 0_usize;
    loop {
        attempt += 1;
        if let Some(status) = process.try_status()? {
            bail!("temporary validator-engine exited early with {status}");
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let message = format!(
                "validator-engine-console was not ready within {}s",
                context.timeout.as_secs()
            );
            return match last_probe_error {
                Some(error) => Err(error).context(message),
                None => Err(anyhow::anyhow!(message)),
            };
        }
        let probe_context = operation_context(
            context,
            node,
            (deadline - now).min(CONSOLE_READINESS_TIMEOUT),
        );
        if attempt == 1 || now >= next_progress_log {
            workflow_retry(
                node,
                "console_readiness",
                attempt,
                0,
                "probing",
                "pending",
                false,
            );
            next_progress_log = now + READINESS_LOG_INTERVAL;
        }
        match console.health(&probe_context, &endpoint).await {
            Ok(_) => {
                workflow_retry(
                    node,
                    "console_readiness",
                    attempt,
                    0,
                    "ready",
                    "success",
                    false,
                );
                return Ok(());
            }
            Err(error) if tokio::time::Instant::now() < deadline => {
                last_probe_error = Some(error);
                sleep(RETRY_DELAY).await;
            }
            Err(error) => {
                workflow_retry(
                    node,
                    "console_readiness",
                    attempt,
                    0,
                    "timed_out",
                    "failure",
                    true,
                );
                return Err(error).with_context(|| {
                    format!(
                        "validator-engine-console was not ready within {}s",
                        context.timeout.as_secs()
                    )
                });
            }
        }
    }
}

/// Starts a temporary engine without taking ownership of its readiness or restart
/// policy away from this workflow.
async fn start_bootstrap(
    layout: &Layout,
    engine: &dyn ValidatorEngine,
    node: &NodeSettings,
) -> Result<ServiceHandle> {
    let node_layout = layout.node(node);
    engine
        .start_bootstrap(ValidatorBootstrapRequest {
            node_name: node.name.clone(),
            global_config: node_layout.global_config,
            database: ValidatorDatabase::at(node_layout.db),
            logs: ValidatorLogPaths {
                engine: node_layout.logs.join("validator-init"),
                stdout: node_layout.logs.join("validator-bootstrap.stdout.log"),
                stderr: node_layout.logs.join("validator-bootstrap.stderr.log"),
            },
            endpoint: AdnlEndpoint::new(node.public_ip, node.adnl_port),
            threads: node.threads,
            verbosity: node.verbosity,
        })
        .await
}

/// Derives the authenticated loopback endpoint from Localton-owned node paths.
///
/// The console never advertises the node's public address: bootstrap control is a
/// local administrative channel authenticated by the generated client/server key
/// pair.
fn console_endpoint(layout: &Layout, node: &NodeSettings) -> ValidatorConsoleEndpoint {
    let node_layout = layout.node(node);
    ValidatorConsoleEndpoint {
        address: (Ipv4Addr::LOCALHOST, node.console_port).into(),
        client_private_key: node_layout.client_private_key(),
        server_public_key: node_layout.server_public_key(),
    }
}

/// Narrows an operation deadline while preserving the workflow's tracing identity.
fn operation_context(
    parent: &OperationContext,
    node: &NodeSettings,
    timeout: Duration,
) -> OperationContext {
    OperationContext {
        timeout: parent.timeout.min(timeout),
        node_name: parent.node_name.clone().or_else(|| Some(node.name.clone())),
    }
}

/// Retries one workflow-approved console mutation without exposing its command or
/// arguments.
///
/// The helper is used only for mutations whose desired key/address tuple remains
/// stable across attempts. Non-idempotent key creation and private-key import use
/// dedicated flows with different recovery rules.
async fn retry_console_mutation<F, Fut>(
    node: &NodeSettings,
    stage: &'static str,
    mut mutation: F,
) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let mut last_error = None;
    for attempt in 1..=CONSOLE_RETRY_LIMIT {
        workflow_retry(
            node,
            stage,
            attempt,
            CONSOLE_RETRY_LIMIT,
            "attempting",
            "pending",
            false,
        );
        match mutation().await {
            Ok(()) => {
                workflow_retry(
                    node,
                    stage,
                    attempt,
                    CONSOLE_RETRY_LIMIT,
                    "complete",
                    "success",
                    false,
                );
                return Ok(());
            }
            Err(error) => {
                workflow_retry(
                    node,
                    stage,
                    attempt,
                    CONSOLE_RETRY_LIMIT,
                    if attempt == CONSOLE_RETRY_LIMIT {
                        "exhausted"
                    } else {
                        "waiting"
                    },
                    if attempt == CONSOLE_RETRY_LIMIT {
                        "failure"
                    } else {
                        "retrying"
                    },
                    true,
                );
                last_error = Some(error);
                if attempt < CONSOLE_RETRY_LIMIT {
                    sleep(RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_error.context("validator console mutation failed without an error")?)
}

/// Emits a workflow stage without tool argv, key identifiers, or payloads.
fn workflow_stage(
    node: &NodeSettings,
    stage: &'static str,
    progress: &'static str,
    outcome: &'static str,
) {
    info!(
        "ton.tool" = "validator-bootstrap",
        operation = "configure_node",
        node = %node.name,
        stage,
        progress,
        outcome,
        "validator workflow stage"
    );
}

/// Emits a retry observation using low-cardinality semantic fields only.
///
/// `retry_limit = 0` denotes readiness polling bounded by a wall-clock deadline
/// instead of an attempt count.
fn workflow_retry(
    node: &NodeSettings,
    stage: &'static str,
    attempt: usize,
    retry_limit: usize,
    progress: &'static str,
    outcome: &'static str,
    failed: bool,
) {
    if failed {
        warn!(
            "ton.tool" = "validator-bootstrap",
            operation = "configure_node",
            node = %node.name,
            stage,
            retry_attempt = attempt,
            retry_limit,
            progress,
            outcome,
            "validator workflow retry"
        );
    } else {
        info!(
            "ton.tool" = "validator-bootstrap",
            operation = "configure_node",
            node = %node.name,
            stage,
            retry_attempt = attempt,
            retry_limit,
            progress,
            outcome,
            "validator workflow retry"
        );
    }
}

/// Emits the terminal result and total duration of a validator workflow.
fn workflow_result<T>(
    node: &NodeSettings,
    stage: &'static str,
    started: std::time::Instant,
    result: &Result<T>,
) {
    match result {
        Ok(_) => info!(
            "ton.tool" = "validator-bootstrap",
            operation = "configure_node",
            node = %node.name,
            stage,
            duration_ms = started.elapsed().as_millis(),
            progress = "complete",
            outcome = "success",
            "validator workflow completed"
        ),
        Err(_) => warn!(
            "ton.tool" = "validator-bootstrap",
            operation = "configure_node",
            node = %node.name,
            stage,
            duration_ms = started.elapsed().as_millis(),
            progress = "complete",
            outcome = "failure",
            "validator workflow failed"
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, time::Duration};

    use anyhow::bail;

    use super::*;

    #[test]
    fn operation_context_never_extends_the_parent_deadline() {
        let node = NodeSettings::default();
        let parent = OperationContext::for_node(Duration::from_secs(4), "genesis");

        let narrowed = operation_context(&parent, &node, Duration::from_secs(15));

        assert_eq!(narrowed.timeout, Duration::from_secs(4));
        assert_eq!(narrowed.node_name.as_deref(), Some("genesis"));
    }

    #[tokio::test]
    async fn approved_console_mutation_retries_without_changing_intent() {
        let node = NodeSettings::default();
        let attempts = Cell::new(0_usize);

        retry_console_mutation(&node, "test_mutation", || {
            let attempt = attempts.get() + 1;
            attempts.set(attempt);
            async move {
                if attempt < 3 {
                    bail!("controlled transient failure")
                }
                Ok(())
            }
        })
        .await
        .unwrap();

        assert_eq!(attempts.get(), 3);
    }
}
