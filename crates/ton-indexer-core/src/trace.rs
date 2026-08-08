//! Incremental, full-message reconstruction of TON transaction traces.
//!
//! A trace is reconstructed by joining a transaction's outgoing message cell
//! hash with the same message cell hash used as another transaction's input.
//! Each edge retains its original immutable [`Cell`], so protocol-specific
//! indexers can decode complete [`Message`] models without re-fetching blocks.
//! A persistent sink can serialize these cells as `BoC`s alongside endpoint
//! upserts.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use thiserror::Error;
use tycho_types::{
    cell::{Cell, Lazy},
    models::{Message, MsgInfo, Transaction, TxInfo},
};

use crate::{Batch, BlockId, Hash256};

/// TON message envelope kind relevant to trace assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    /// Internal message.
    Internal,
    /// Incoming external message.
    ExternalIn,
    /// Outgoing external message.
    ExternalOut,
}

/// Errors raised while extracting or joining trace records.
#[derive(Debug, Error)]
pub enum TraceError {
    /// A nested transaction or message model could not be loaded.
    #[error("invalid trace input: {0}")]
    InvalidModel(String),
    /// One transaction hash was observed with different contents.
    #[error("transaction {0} was observed with conflicting contents")]
    ConflictingTransaction(Hash256),
    /// One message hash was observed with different envelope kinds.
    #[error("message {message} changed kind from {existing:?} to {incoming:?}")]
    ConflictingMessageKind {
        /// Message representation hash.
        message: Hash256,
        /// Previously observed kind.
        existing: MessageKind,
        /// Newly observed kind.
        incoming: MessageKind,
    },
    /// One message hash was observed with different creation logical times.
    #[error("message {message} changed creation logical time from {existing:?} to {incoming:?}")]
    ConflictingMessageLogicalTime {
        /// Message representation hash.
        message: Hash256,
        /// Previously observed creation logical time.
        existing: Option<u64>,
        /// Newly observed creation logical time.
        incoming: Option<u64>,
    },
    /// One message was attributed to two transactions at the same endpoint.
    #[error("message {message} has conflicting {endpoint} transactions: {existing} and {incoming}")]
    ConflictingMessageEndpoint {
        /// Message representation hash.
        message: Hash256,
        /// Either `source` or `destination`.
        endpoint: &'static str,
        /// Previously observed transaction.
        existing: Hash256,
        /// Newly observed transaction.
        incoming: Hash256,
    },
    /// The message kind cannot occur at this transaction endpoint.
    #[error("{kind:?} message {message} cannot be a transaction {endpoint}")]
    InvalidMessageEndpoint {
        /// Message representation hash.
        message: Hash256,
        /// Message envelope kind.
        kind: MessageKind,
        /// Either `input` or `output`.
        endpoint: &'static str,
    },
    /// A message-to-transaction link violates TON logical-time ordering.
    #[error(
        "message {message} has invalid logical time {message_lt} between source tx lt \
         {source_lt:?} and destination tx lt {destination_lt:?}"
    )]
    InvalidLogicalTime {
        /// Message representation hash.
        message: Hash256,
        /// Message creation logical time.
        message_lt: u64,
        /// Source transaction logical time, when observed.
        source_lt: Option<u64>,
        /// Destination transaction logical time, when observed.
        destination_lt: Option<u64>,
    },
    /// The stored transaction/message relation contains a cycle.
    #[error("trace containing transaction {0} has a causal cycle")]
    CausalCycle(Hash256),
}

/// TON transaction kind relevant to trace roots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceTransactionKind {
    /// An ordinary message-processing transaction.
    Ordinary,
    /// A system tick-tock transaction.
    TickTock,
}

/// A storage-neutral transaction node in a trace graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceTransaction {
    /// Transaction cell representation hash.
    pub hash: Hash256,
    /// Block containing this transaction.
    pub block_id: BlockId,
    /// Masterchain batch that made the containing block canonical.
    pub masterchain_seqno: u32,
    /// Account workchain.
    pub account_workchain: i32,
    /// Account address bits.
    pub account: Hash256,
    /// Transaction logical time.
    pub lt: u64,
    /// Transaction Unix timestamp.
    pub now: u32,
    /// Transaction description kind.
    pub kind: TraceTransactionKind,
    /// Whether execution changes were reverted.
    pub aborted: bool,
    /// Message consumed by this transaction.
    pub incoming_message: Option<Hash256>,
    /// Messages produced by this transaction.
    pub outgoing_messages: Vec<Hash256>,
}

/// A message edge whose source and destination transactions may arrive in
/// different canonical batches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceMessage {
    /// Message cell representation hash.
    pub hash: Hash256,
    /// Complete immutable message cell as stored in the transaction.
    ///
    /// This retains the full envelope, optional `StateInit`, body, and their
    /// original cell layout.
    pub cell: Cell,
    /// TON message envelope kind.
    pub kind: MessageKind,
    /// Logical time at which an internal or external-out message was created.
    pub created_lt: Option<u64>,
    /// Transaction that produced this message, when observed.
    pub source_transaction: Option<Hash256>,
    /// Transaction that consumed this message, when observed.
    pub destination_transaction: Option<Hash256>,
}

impl TraceMessage {
    /// Parses the retained cell as a complete `tycho-types` message.
    ///
    /// The returned message body borrows from [`Self::cell`].
    ///
    /// # Errors
    ///
    /// Returns an error if the retained cell is not a valid TON message.
    pub fn load(&self) -> Result<Message<'_>, tycho_types::error::Error> {
        self.cell.parse()
    }
}

/// Why a reconstructed trace starts inside the indexed dataset.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TraceRoot {
    /// The trace starts with an incoming external message.
    ExternalIn {
        /// Root external message hash.
        message_hash: Hash256,
    },
    /// The trace starts with a transaction that has no incoming message.
    Special {
        /// Root transaction hash.
        transaction_hash: Hash256,
    },
    /// The parent transaction is outside the indexed range or not yet seen.
    OrphanInternal {
        /// Unmatched incoming internal message hash.
        message_hash: Hash256,
    },
}

impl TraceRoot {
    /// Returns the root hash used as a stable trace identifier.
    #[must_use]
    pub const fn id(self) -> Hash256 {
        match self {
            Self::ExternalIn { message_hash } | Self::OrphanInternal { message_hash } => {
                message_hash
            }
            Self::Special { transaction_hash } => transaction_hash,
        }
    }
}

/// Current completeness of an assembled trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceStatus {
    /// The root is known and every internal output has been consumed.
    Complete,
    /// The root is known but at least one internal output is still in flight.
    Pending,
    /// The trace begins before the indexed range but has no open outputs.
    Orphan,
    /// The trace begins before the indexed range and still has open outputs.
    OrphanPending,
}

/// One reconstructed trace DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssembledTrace {
    /// Root classification and identifier.
    pub root: TraceRoot,
    /// Transactions belonging to the trace, ordered by logical time and hash.
    pub transactions: Vec<Hash256>,
    /// Internal outgoing messages with no observed destination transaction.
    pub pending_messages: Vec<Hash256>,
}

impl AssembledTrace {
    /// Returns the current completeness state.
    #[must_use]
    pub const fn status(&self) -> TraceStatus {
        match (
            matches!(self.root, TraceRoot::OrphanInternal { .. }),
            self.pending_messages.is_empty(),
        ) {
            (false, true) => TraceStatus::Complete,
            (false, false) => TraceStatus::Pending,
            (true, true) => TraceStatus::Orphan,
            (true, false) => TraceStatus::OrphanPending,
        }
    }
}

/// Counts produced by one idempotent batch ingest.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TraceIngestStats {
    /// Previously unseen transactions.
    pub new_transactions: usize,
    /// Previously unseen messages.
    pub new_messages: usize,
    /// Messages whose source and destination transactions are now both known.
    pub newly_linked_messages: usize,
}

/// Incrementally joins transaction nodes through message hashes.
#[derive(Clone, Debug, Default)]
pub struct TraceAssembler {
    transactions: BTreeMap<Hash256, TraceTransaction>,
    messages: BTreeMap<Hash256, TraceMessage>,
}

impl TraceAssembler {
    /// Extracts all transactions and messages from a canonical batch and
    /// applies them atomically to the in-memory graph.
    ///
    /// Re-ingesting an identical batch is a no-op. Conflicting records are
    /// rejected before the assembler is mutated.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed nested TON models, conflicting hashes,
    /// invalid message directions, or invalid causal logical times.
    pub fn ingest_batch(&mut self, batch: &Batch) -> Result<TraceIngestStats, TraceError> {
        let observation = BatchObservation::from_batch(batch)?;
        self.apply_observation(observation)
    }

    /// Returns a transaction by its representation hash.
    #[must_use]
    pub fn transaction(&self, hash: &Hash256) -> Option<&TraceTransaction> {
        self.transactions.get(hash)
    }

    /// Returns a message by its representation hash.
    #[must_use]
    pub fn message(&self, hash: &Hash256) -> Option<&TraceMessage> {
        self.messages.get(hash)
    }

    /// Iterates over all indexed transactions in hash order.
    pub fn transactions(&self) -> impl Iterator<Item = &TraceTransaction> {
        self.transactions.values()
    }

    /// Iterates over all indexed messages in hash order.
    pub fn messages(&self) -> impl Iterator<Item = &TraceMessage> {
        self.messages.values()
    }

    /// Returns the parent transaction linked through this transaction's input.
    #[must_use]
    pub fn parent_transaction(&self, transaction: &Hash256) -> Option<Hash256> {
        let transaction = self.transactions.get(transaction)?;
        let incoming = transaction.incoming_message?;
        self.messages.get(&incoming)?.source_transaction
    }

    /// Reconstructs every trace visible in the current indexed window.
    ///
    /// Transactions and pending messages inside each trace are returned in a
    /// deterministic order.
    ///
    /// # Errors
    ///
    /// Returns an error if a causal cycle is found.
    pub fn traces(&self) -> Result<Vec<AssembledTrace>, TraceError> {
        let mut grouped = BTreeMap::<TraceRoot, Vec<Hash256>>::new();
        for transaction in self.transactions.values() {
            let root = self.resolve_root(transaction.hash)?;
            grouped.entry(root).or_default().push(transaction.hash);
        }

        let mut traces = Vec::with_capacity(grouped.len());
        for (root, mut transactions) in grouped {
            transactions.sort_unstable_by_key(|hash| {
                let transaction = &self.transactions[hash];
                (transaction.lt, *hash)
            });

            let mut pending_messages = BTreeSet::new();
            for transaction_hash in &transactions {
                let transaction = &self.transactions[transaction_hash];
                for message_hash in &transaction.outgoing_messages {
                    let message = &self.messages[message_hash];
                    if message.kind == MessageKind::Internal
                        && message.destination_transaction.is_none()
                    {
                        pending_messages.insert(*message_hash);
                    }
                }
            }

            traces.push(AssembledTrace {
                root,
                transactions,
                pending_messages: pending_messages.into_iter().collect(),
            });
        }
        Ok(traces)
    }

    fn apply_observation(
        &mut self,
        observation: BatchObservation,
    ) -> Result<TraceIngestStats, TraceError> {
        for (hash, transaction) in &observation.transactions {
            if let Some(existing) = self.transactions.get(hash)
                && existing != transaction
            {
                return Err(TraceError::ConflictingTransaction(*hash));
            }
        }

        let mut merged_messages = BTreeMap::new();
        for (hash, message) in &observation.messages {
            let merged = match self.messages.get(hash) {
                Some(existing) => merge_message(existing, message)?,
                None => message.clone(),
            };
            merged_messages.insert(*hash, merged);
        }

        for message in merged_messages.values() {
            validate_logical_time(message, &self.transactions, &observation.transactions)?;
        }

        let linked_before = merged_messages
            .keys()
            .filter(|hash| self.messages.get(hash).is_some_and(message_is_linked))
            .count();
        let linked_after = merged_messages
            .values()
            .filter(|message| message_is_linked(message))
            .count();
        let stats = TraceIngestStats {
            new_transactions: observation
                .transactions
                .keys()
                .filter(|hash| !self.transactions.contains_key(hash))
                .count(),
            new_messages: observation
                .messages
                .keys()
                .filter(|hash| !self.messages.contains_key(hash))
                .count(),
            newly_linked_messages: linked_after - linked_before,
        };

        self.transactions.extend(observation.transactions);
        self.messages.extend(merged_messages);
        Ok(stats)
    }

    fn resolve_root(&self, transaction: Hash256) -> Result<TraceRoot, TraceError> {
        let mut current = transaction;
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(current) {
                return Err(TraceError::CausalCycle(transaction));
            }

            let transaction = &self.transactions[&current];
            let Some(incoming_hash) = transaction.incoming_message else {
                return Ok(TraceRoot::Special {
                    transaction_hash: current,
                });
            };
            let incoming = &self.messages[&incoming_hash];
            match incoming.kind {
                MessageKind::ExternalIn => {
                    return Ok(TraceRoot::ExternalIn {
                        message_hash: incoming_hash,
                    });
                }
                MessageKind::Internal => match incoming.source_transaction {
                    Some(parent) => current = parent,
                    None => {
                        return Ok(TraceRoot::OrphanInternal {
                            message_hash: incoming_hash,
                        });
                    }
                },
                MessageKind::ExternalOut => {
                    return Err(TraceError::InvalidMessageEndpoint {
                        message: incoming_hash,
                        kind: MessageKind::ExternalOut,
                        endpoint: "input",
                    });
                }
            }
        }
    }
}

#[derive(Clone, Default)]
struct BatchObservation {
    transactions: BTreeMap<Hash256, TraceTransaction>,
    messages: BTreeMap<Hash256, TraceMessage>,
}

impl BatchObservation {
    fn from_batch(batch: &Batch) -> Result<Self, TraceError> {
        let mut observation = Self::default();
        let masterchain_seqno = batch.masterchain().id().seqno;

        for block in batch.blocks() {
            for lazy_transaction in block.transactions() {
                observation.observe_transaction(block.id(), masterchain_seqno, lazy_transaction)?;
            }
        }
        Ok(observation)
    }

    fn observe_transaction(
        &mut self,
        block_id: BlockId,
        masterchain_seqno: u32,
        lazy_transaction: &Lazy<Transaction>,
    ) -> Result<(), TraceError> {
        let hash = hash256(*lazy_transaction.inner().repr_hash());
        let transaction = lazy_transaction
            .load()
            .map_err(invalid_trace_model("transaction"))?;
        let (kind, aborted) = match transaction
            .load_info()
            .map_err(invalid_trace_model("transaction info"))?
        {
            TxInfo::Ordinary(info) => (TraceTransactionKind::Ordinary, info.aborted),
            TxInfo::TickTock(info) => (TraceTransactionKind::TickTock, info.aborted),
        };

        let incoming_message = match &transaction.in_msg {
            Some(cell) => {
                let message = message_record(cell, None, Some(hash))?;
                if message.kind == MessageKind::ExternalOut {
                    return Err(TraceError::InvalidMessageEndpoint {
                        message: message.hash,
                        kind: message.kind,
                        endpoint: "input",
                    });
                }
                let message_hash = message.hash;
                self.observe_message(message)?;
                Some(message_hash)
            }
            None => None,
        };

        let mut outgoing_messages =
            Vec::with_capacity(usize::from(transaction.out_msg_count.into_inner()));
        for entry in transaction.out_msgs.iter() {
            let (_, cell) = entry.map_err(invalid_trace_model("out message dictionary"))?;
            let message = message_record(&cell, Some(hash), None)?;
            if message.kind == MessageKind::ExternalIn {
                return Err(TraceError::InvalidMessageEndpoint {
                    message: message.hash,
                    kind: message.kind,
                    endpoint: "output",
                });
            }
            outgoing_messages.push(message.hash);
            self.observe_message(message)?;
        }

        let record = TraceTransaction {
            hash,
            block_id,
            masterchain_seqno,
            account_workchain: block_id.workchain,
            account: hash256(transaction.account),
            lt: transaction.lt,
            now: transaction.now,
            kind,
            aborted,
            incoming_message,
            outgoing_messages,
        };
        match self.transactions.get(&hash) {
            Some(existing) if existing != &record => Err(TraceError::ConflictingTransaction(hash)),
            Some(_) => Ok(()),
            None => {
                self.transactions.insert(hash, record);
                Ok(())
            }
        }
    }

    fn observe_message(&mut self, message: TraceMessage) -> Result<(), TraceError> {
        match self.messages.get(&message.hash) {
            Some(existing) => {
                let merged = merge_message(existing, &message)?;
                self.messages.insert(message.hash, merged);
            }
            None => {
                self.messages.insert(message.hash, message);
            }
        }
        Ok(())
    }
}

fn message_record(
    cell: &Cell,
    source_transaction: Option<Hash256>,
    destination_transaction: Option<Hash256>,
) -> Result<TraceMessage, TraceError> {
    let hash = hash256(*cell.repr_hash());
    let message = cell
        .parse::<Message<'_>>()
        .map_err(invalid_trace_model("message"))?;
    let (kind, created_lt) = match message.info {
        MsgInfo::Int(info) => (MessageKind::Internal, Some(info.created_lt)),
        MsgInfo::ExtIn(_) => (MessageKind::ExternalIn, None),
        MsgInfo::ExtOut(info) => (MessageKind::ExternalOut, Some(info.created_lt)),
    };
    Ok(TraceMessage {
        hash,
        cell: cell.clone(),
        kind,
        created_lt,
        source_transaction,
        destination_transaction,
    })
}

fn merge_message(
    existing: &TraceMessage,
    incoming: &TraceMessage,
) -> Result<TraceMessage, TraceError> {
    if existing.kind != incoming.kind {
        return Err(TraceError::ConflictingMessageKind {
            message: existing.hash,
            existing: existing.kind,
            incoming: incoming.kind,
        });
    }
    if existing.created_lt != incoming.created_lt {
        return Err(TraceError::ConflictingMessageLogicalTime {
            message: existing.hash,
            existing: existing.created_lt,
            incoming: incoming.created_lt,
        });
    }

    Ok(TraceMessage {
        hash: existing.hash,
        cell: existing.cell.clone(),
        kind: existing.kind,
        created_lt: existing.created_lt,
        source_transaction: merge_endpoint(
            existing.hash,
            "source",
            existing.source_transaction,
            incoming.source_transaction,
        )?,
        destination_transaction: merge_endpoint(
            existing.hash,
            "destination",
            existing.destination_transaction,
            incoming.destination_transaction,
        )?,
    })
}

fn merge_endpoint(
    message: Hash256,
    endpoint: &'static str,
    existing: Option<Hash256>,
    incoming: Option<Hash256>,
) -> Result<Option<Hash256>, TraceError> {
    match (existing, incoming) {
        (Some(existing), Some(incoming)) if existing != incoming => {
            Err(TraceError::ConflictingMessageEndpoint {
                message,
                endpoint,
                existing,
                incoming,
            })
        }
        (Some(existing), _) => Ok(Some(existing)),
        (_, Some(incoming)) => Ok(Some(incoming)),
        (None, None) => Ok(None),
    }
}

fn validate_logical_time(
    message: &TraceMessage,
    existing_transactions: &BTreeMap<Hash256, TraceTransaction>,
    new_transactions: &BTreeMap<Hash256, TraceTransaction>,
) -> Result<(), TraceError> {
    let transaction = |hash: &Hash256| {
        new_transactions
            .get(hash)
            .or_else(|| existing_transactions.get(hash))
    };
    let source_lt = message
        .source_transaction
        .and_then(|hash| transaction(&hash))
        .map(|transaction| transaction.lt);
    let destination_lt = message
        .destination_transaction
        .and_then(|hash| transaction(&hash))
        .map(|transaction| transaction.lt);

    let Some(message_lt) = message.created_lt else {
        return Ok(());
    };
    let source_is_valid = source_lt.is_none_or(|lt| lt < message_lt);
    let destination_is_valid = destination_lt.is_none_or(|lt| message_lt < lt);
    if source_is_valid && destination_is_valid {
        Ok(())
    } else {
        Err(TraceError::InvalidLogicalTime {
            message: message.hash,
            message_lt,
            source_lt,
            destination_lt,
        })
    }
}

const fn message_is_linked(message: &TraceMessage) -> bool {
    message.source_transaction.is_some() && message.destination_transaction.is_some()
}

const fn hash256(hash: tycho_types::cell::HashBytes) -> Hash256 {
    Hash256::new(hash.0)
}

fn invalid_trace_model(
    context: &'static str,
) -> impl FnOnce(tycho_types::error::Error) -> TraceError {
    move |error| TraceError::InvalidModel(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tycho_types::{cell::CellBuilder, models::IntMsgInfo};

    #[test]
    fn retains_and_loads_the_complete_tycho_message() {
        let mut body_builder = CellBuilder::new();
        body_builder.store_u32(0x0f8a_7ea5).unwrap();
        let body = body_builder.build().unwrap();
        let cell = CellBuilder::build_from(Message {
            info: MsgInfo::Int(IntMsgInfo {
                bounce: true,
                bounced: true,
                created_lt: 11,
                created_at: 42,
                ..Default::default()
            }),
            init: None,
            body: body.as_slice().unwrap(),
            layout: None,
        })
        .unwrap();

        let source = hash(1);
        let destination = hash(2);
        let outgoing = message_record(&cell, Some(source), None).unwrap();
        let incoming = message_record(&cell, None, Some(destination)).unwrap();
        let record = merge_message(&outgoing, &incoming).unwrap();

        assert_eq!(record.hash, hash256(*cell.repr_hash()));
        assert_eq!(record.cell, cell);
        assert_eq!(record.source_transaction, Some(source));
        assert_eq!(record.destination_transaction, Some(destination));

        let mut message = record.load().unwrap();
        let MsgInfo::Int(info) = message.info else {
            panic!("expected an internal message");
        };
        assert!(info.bounce);
        assert!(info.bounced);
        assert_eq!(info.created_lt, 11);
        assert_eq!(info.created_at, 42);
        assert_eq!(message.body.load_u32().unwrap(), 0x0f8a_7ea5);
        assert!(message.init.is_none());
    }

    #[test]
    fn joins_an_external_trace_across_observations() {
        let external = hash(1);
        let internal = hash(2);
        let root_tx = hash(3);
        let child_tx = hash(4);
        let mut assembler = TraceAssembler::default();

        let first = observation(
            vec![transaction(root_tx, 10, Some(external), vec![internal])],
            vec![
                message(external, MessageKind::ExternalIn, None, None, Some(root_tx)),
                message(
                    internal,
                    MessageKind::Internal,
                    Some(11),
                    Some(root_tx),
                    None,
                ),
            ],
        );
        assert_eq!(
            assembler.apply_observation(first).unwrap(),
            TraceIngestStats {
                new_transactions: 1,
                new_messages: 2,
                newly_linked_messages: 0,
            }
        );
        let pending = assembler.traces().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status(), TraceStatus::Pending);
        assert_eq!(pending[0].pending_messages, vec![internal]);

        let second = observation(
            vec![transaction(child_tx, 13, Some(internal), Vec::new())],
            vec![message(
                internal,
                MessageKind::Internal,
                Some(11),
                None,
                Some(child_tx),
            )],
        );
        assert_eq!(
            assembler.apply_observation(second.clone()).unwrap(),
            TraceIngestStats {
                new_transactions: 1,
                new_messages: 0,
                newly_linked_messages: 1,
            }
        );
        assert_eq!(
            assembler.apply_observation(second).unwrap(),
            TraceIngestStats::default()
        );

        let complete = assembler.traces().unwrap();
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0].status(), TraceStatus::Complete);
        assert_eq!(complete[0].transactions, vec![root_tx, child_tx]);
        assert_eq!(assembler.parent_transaction(&child_tx), Some(root_tx));
    }

    #[test]
    fn marks_an_unmatched_internal_input_as_orphan() {
        let internal = hash(1);
        let child_tx = hash(2);
        let mut assembler = TraceAssembler::default();
        assembler
            .apply_observation(observation(
                vec![transaction(child_tx, 20, Some(internal), Vec::new())],
                vec![message(
                    internal,
                    MessageKind::Internal,
                    Some(19),
                    None,
                    Some(child_tx),
                )],
            ))
            .unwrap();

        let traces = assembler.traces().unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(
            traces[0].root,
            TraceRoot::OrphanInternal {
                message_hash: internal
            }
        );
        assert_eq!(traces[0].status(), TraceStatus::Orphan);
    }

    #[test]
    fn heals_an_orphan_when_its_parent_arrives_later() {
        let external = hash(1);
        let internal = hash(2);
        let root_tx = hash(3);
        let child_tx = hash(4);
        let mut assembler = TraceAssembler::default();

        assembler
            .apply_observation(observation(
                vec![transaction(child_tx, 13, Some(internal), Vec::new())],
                vec![message(
                    internal,
                    MessageKind::Internal,
                    Some(11),
                    None,
                    Some(child_tx),
                )],
            ))
            .unwrap();
        assert_eq!(assembler.traces().unwrap()[0].status(), TraceStatus::Orphan);

        assembler
            .apply_observation(observation(
                vec![transaction(root_tx, 10, Some(external), vec![internal])],
                vec![
                    message(external, MessageKind::ExternalIn, None, None, Some(root_tx)),
                    message(
                        internal,
                        MessageKind::Internal,
                        Some(11),
                        Some(root_tx),
                        None,
                    ),
                ],
            ))
            .unwrap();

        let traces = assembler.traces().unwrap();
        assert_eq!(traces.len(), 1);
        assert_eq!(
            traces[0].root,
            TraceRoot::ExternalIn {
                message_hash: external
            }
        );
        assert_eq!(traces[0].status(), TraceStatus::Complete);
        assert_eq!(traces[0].transactions, vec![root_tx, child_tx]);
    }

    #[test]
    fn rejects_conflicting_message_endpoints_without_mutating_state() {
        let internal = hash(1);
        let first_source = hash(2);
        let second_source = hash(3);
        let mut assembler = TraceAssembler::default();
        let first = observation(
            vec![transaction(first_source, 10, None, vec![internal])],
            vec![message(
                internal,
                MessageKind::Internal,
                Some(11),
                Some(first_source),
                None,
            )],
        );
        assembler.apply_observation(first).unwrap();

        let conflicting = observation(
            vec![transaction(second_source, 9, None, vec![internal])],
            vec![message(
                internal,
                MessageKind::Internal,
                Some(11),
                Some(second_source),
                None,
            )],
        );
        assert!(matches!(
            assembler.apply_observation(conflicting),
            Err(TraceError::ConflictingMessageEndpoint {
                endpoint: "source",
                ..
            })
        ));
        assert!(assembler.transaction(&second_source).is_none());
        assert_eq!(
            assembler.message(&internal).unwrap().source_transaction,
            Some(first_source)
        );
    }

    fn observation(
        transactions: Vec<TraceTransaction>,
        messages: Vec<TraceMessage>,
    ) -> BatchObservation {
        BatchObservation {
            transactions: transactions
                .into_iter()
                .map(|transaction| (transaction.hash, transaction))
                .collect(),
            messages: messages
                .into_iter()
                .map(|message| (message.hash, message))
                .collect(),
        }
    }

    fn transaction(
        hash: Hash256,
        lt: u64,
        incoming_message: Option<Hash256>,
        outgoing_messages: Vec<Hash256>,
    ) -> TraceTransaction {
        TraceTransaction {
            hash,
            block_id: BlockId {
                workchain: 0,
                shard: 1_u64 << 63,
                seqno: 1,
                root_hash: Hash256::ZERO,
                file_hash: Hash256::ZERO,
            },
            masterchain_seqno: 1,
            account_workchain: 0,
            account: hash,
            lt,
            now: 0,
            kind: TraceTransactionKind::Ordinary,
            aborted: false,
            incoming_message,
            outgoing_messages,
        }
    }

    fn message(
        hash: Hash256,
        kind: MessageKind,
        created_lt: Option<u64>,
        source_transaction: Option<Hash256>,
        destination_transaction: Option<Hash256>,
    ) -> TraceMessage {
        TraceMessage {
            hash,
            cell: Cell::default(),
            kind,
            created_lt,
            source_transaction,
            destination_transaction,
        }
    }

    const fn hash(marker: u8) -> Hash256 {
        Hash256::new([marker; 32])
    }
}
