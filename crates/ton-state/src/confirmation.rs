//! Live matching of submitted messages against fully committed block batches.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use axum::http::StatusCode;
use rston::cell::{HashBytes, Lazy};
use rston::models::{Message, MsgInfo, StdAddr, Transaction};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use ton_indexer_core::{Batch, BlockId, normalized_external_message_hash};

use crate::api::ApiError;

const MAX_WAITERS: usize = 64;
const MAX_PENDING_TRACE_MESSAGES: usize = 16_384;

/// Selects whether an observation ends at the root transaction or after all its
/// internal descendants, including bounces, have committed.
#[derive(Clone, Copy)]
pub(crate) enum WaitFor {
    Transaction,
    Trace,
}

/// A completed observation retains transaction cells only when the caller needs
/// their contents. Trace identifiers are the root transaction's cell hash.
pub(crate) enum Confirmation {
    Transaction(ConfirmedTransaction),
    Trace(HashBytes),
}

/// Bounded, live-only observations of committed transactions. Registrations must
/// precede broadcast; previously published batches are never replayed.
#[derive(Clone)]
pub(crate) struct Confirmations {
    waiters: Arc<Mutex<Vec<Waiter>>>,
    capacity: Arc<Semaphore>,
}

struct Waiter {
    started: Instant,
    destination: StdAddr,
    hash: HashBytes,
    sender: oneshot::Sender<Result<Confirmation, ApiError>>,
    trace: Option<PendingTrace>,
}

#[derive(Default)]
struct PendingTrace {
    root: Option<HashBytes>,
    messages: HashSet<HashBytes>,
}

/// Retains the original transaction cell until the HTTP response is encoded.
/// Block coordinates refer to the batch already committed by the synchronizer.
#[derive(Clone)]
pub(crate) struct ConfirmedTransaction {
    pub(crate) block: BlockId,
    pub(crate) mc_seqno: u32,
    pub(crate) transaction: Lazy<Transaction>,
}

/// Dropping an observation unregisters it immediately, including on cancellation.
/// The capacity permit lives until its buffered result is consumed or dropped.
pub(crate) struct Observation {
    pub(crate) receiver: oneshot::Receiver<Result<Confirmation, ApiError>>,
    confirmations: Confirmations,
    _slot: OwnedSemaphorePermit,
}

impl Default for Confirmations {
    fn default() -> Self {
        Self {
            waiters: Arc::default(),
            capacity: Arc::new(Semaphore::new(MAX_WAITERS)),
        }
    }
}

impl Confirmations {
    /// Reserves a live observation before submission. A full or closed registry
    /// rejects the request before any message is sent to the network.
    pub(crate) fn register(
        &self,
        destination: StdAddr,
        hash: HashBytes,
        wait_for: WaitFor,
    ) -> Result<Observation, ApiError> {
        let mut waiters = self.waiters.lock().expect("confirmation lock poisoned");
        if self.capacity.is_closed() {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "shutting_down",
            ));
        }
        let slot = self.capacity.clone().try_acquire_owned().map_err(|_| {
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "too many pending transaction waits",
            )
        })?;
        let (sender, receiver) = oneshot::channel();
        waiters.push(Waiter {
            started: Instant::now(),
            destination,
            hash,
            sender,
            trace: matches!(wait_for, WaitFor::Trace).then(PendingTrace::default),
        });

        Ok(Observation {
            receiver,
            confirmations: self.clone(),
            _slot: slot,
        })
    }

    /// Observes a batch only after its complete state and history have committed.
    /// Transactions are visited in logical-time order across all shards so a
    /// child in the same batch can close its parent's outgoing message. Trace
    /// completion is checked after the entire batch, including bounce outputs.
    /// Call [`Self::fail`] on error to report a publication gap.
    pub(crate) fn publish(&self, batch: &Batch) -> Result<()> {
        if self
            .waiters
            .lock()
            .expect("confirmation lock poisoned")
            .is_empty()
        {
            return Ok(());
        }

        let mut transactions = Vec::new();
        for block in batch.blocks() {
            for lazy in block.transactions() {
                let tx = lazy.load().with_context(|| {
                    format!(
                        "cannot decode transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block.id()
                    )
                })?;
                transactions.push((block.id(), lazy, tx));
            }
        }
        transactions.sort_unstable_by_key(|(block, _, tx)| (tx.lt, block.workchain, tx.account));

        for (block, lazy, tx) in transactions {
            let Some(cell) = &tx.in_msg else {
                continue;
            };
            let mut waiters = self.waiters.lock().expect("confirmation lock poisoned");
            let root_interested = waiters.iter().any(|waiter| {
                waiter
                    .trace
                    .as_ref()
                    .is_none_or(|trace| trace.root.is_none())
                    && i32::from(waiter.destination.workchain) == block.workchain
                    && waiter.destination.address == tx.account
            });
            let root = if root_interested {
                let message = cell.parse::<Message>().with_context(|| {
                    format!(
                        "cannot decode incoming message in transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block
                    )
                })?;
                match &message.info {
                    MsgInfo::ExtIn(info) => Some((
                        info.dst.clone(),
                        normalized_external_message_hash(&message).with_context(|| {
                            format!(
                                "cannot normalize incoming message in transaction {}",
                                lazy.inner().repr_hash()
                            )
                        })?,
                    )),
                    _ => None,
                }
            } else {
                None
            };
            let interested = waiters
                .iter()
                .enumerate()
                .filter_map(|(index, waiter)| {
                    let matches = match &waiter.trace {
                        Some(trace) if trace.root.is_some() => {
                            trace.messages.contains(cell.repr_hash())
                        }
                        _ => root.as_ref().is_some_and(|(destination, hash)| {
                            *hash == waiter.hash
                                && *destination == waiter.destination.clone().into()
                                && i32::from(waiter.destination.workchain) == block.workchain
                                && waiter.destination.address == tx.account
                        }),
                    };
                    matches.then_some(index)
                })
                .collect::<Vec<_>>();
            let mut outgoing = Vec::new();
            if interested
                .iter()
                .any(|index| waiters[*index].trace.is_some())
            {
                for entry in tx.out_msgs.iter() {
                    let (_, cell) = entry.with_context(|| {
                        format!(
                            "cannot decode outputs in transaction {}",
                            lazy.inner().repr_hash()
                        )
                    })?;
                    // Only emitted internal messages have a receiving transaction.
                    // External outputs are terminal; failed/skipped actions emit none.
                    if matches!(cell.parse::<MsgInfo>()?, MsgInfo::Int(_)) {
                        outgoing.push(*cell.repr_hash());
                    }
                }
            }

            // Removing higher positions first keeps the remaining indices valid.
            for index in interested.into_iter().rev() {
                let started = waiters[index].started;
                let Some(trace) = &mut waiters[index].trace else {
                    let waiter = waiters.swap_remove(index);
                    let _ =
                        waiter
                            .sender
                            .send(Ok(Confirmation::Transaction(ConfirmedTransaction {
                                block,
                                mc_seqno: batch.checkpoint().seqno,
                                transaction: lazy.clone(),
                            })));
                    continue;
                };
                let root = *trace.root.get_or_insert_with(|| *lazy.inner().repr_hash());
                trace.messages.remove(cell.repr_hash());
                trace.messages.extend(outgoing.iter().copied());
                tracing::info!(
                    operation = "send_boc_and_wait_trace",
                    target = %root,
                    mc_block_seqno = batch.checkpoint().seqno,
                    pending_messages = trace.messages.len(),
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "progress",
                    "observed trace transaction and its emitted messages",
                );
                if trace.messages.len() > MAX_PENDING_TRACE_MESSAGES {
                    let waiter = waiters.swap_remove(index);
                    let _ = waiter.sender.send(Err(ApiError::new(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "trace_pending_messages_limit_exceeded",
                    )));
                }
            }
            drop(waiters);
        }

        let mut waiters = self.waiters.lock().expect("confirmation lock poisoned");
        for index in (0..waiters.len()).rev() {
            let complete = waiters[index]
                .trace
                .as_ref()
                .and_then(|trace| trace.root.filter(|_| trace.messages.is_empty()));
            if let Some(hash) = complete {
                let waiter = waiters.swap_remove(index);
                let _ = waiter.sender.send(Ok(Confirmation::Trace(hash)));
            }
        }
        drop(waiters);
        Ok(())
    }

    /// Fails current observations after a publication gap. Later requests can
    /// register again; they cannot recover the missed batch.
    pub(crate) fn fail(&self) {
        self.waiters
            .lock()
            .expect("confirmation lock poisoned")
            .clear();
    }

    /// Rejects new observations and wakes pending requests during shutdown.
    pub(crate) fn close(&self) {
        self.capacity.close();
        self.fail();
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.receiver.close();
        self.confirmations
            .waiters
            .lock()
            .expect("confirmation lock poisoned")
            .retain(|waiter| !waiter.sender.is_closed());
    }
}
