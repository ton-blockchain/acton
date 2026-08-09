use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result};
use serde::Serialize;
use tokio::sync::RwLock;
use ton_indexer_core::{Batch, Hash256};
use tycho_types::{
    cell::CellSlice,
    models::{Message, MsgInfo},
};
use utoipa::ToSchema;

pub(crate) const MAX_TRANSACTION_EXAMPLES: usize = 2;

/// Aggregate statistics for one message opcode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
pub struct OpcodeCount {
    /// The first 32 bits of the message body after an optional bounce prefix.
    pub opcode: u32,
    /// Unique messages that contain this opcode.
    pub messages: u64,
    /// Up to two transactions that contain example messages with this opcode.
    pub example_transaction_hashes: Vec<String>,
}

/// All-time opcode statistics collected by this backend database.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
pub struct OpcodeSnapshot {
    /// First masterchain batch included in these statistics.
    pub first_masterchain_seqno: Option<u32>,
    /// Latest masterchain batch included in these statistics.
    pub latest_masterchain_seqno: Option<u32>,
    /// Unique messages observed since opcode tracking started.
    pub total_messages: u64,
    /// Observed messages included in the opcode counts.
    pub messages_with_opcode: u64,
    /// Distinct opcodes observed since opcode tracking started.
    pub total_opcodes: u64,
    /// Distinct opcodes that satisfy the requested message threshold.
    pub matching_opcodes: u64,
    /// Opcode counts ordered by message count, from highest to lowest.
    pub opcodes: Vec<OpcodeCount>,
}

/// Shared in-memory view of the persisted opcode statistics.
#[derive(Clone, Default)]
pub struct OpcodeStats {
    inner: Arc<RwLock<OpcodeAccumulator>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OpcodeBatchStats {
    pub(crate) masterchain_seqno: u32,
    pub(crate) total_messages: u64,
    pub(crate) counts: BTreeMap<u32, OpcodeAggregate>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OpcodeAggregate {
    pub(crate) messages: u64,
    pub(crate) example_transactions: Vec<Hash256>,
}

impl OpcodeAggregate {
    fn record(&mut self, transaction_hash: Hash256) {
        self.messages = self.messages.saturating_add(1);
        self.add_example(transaction_hash);
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        self.messages = self.messages.saturating_add(other.messages);
        for &transaction_hash in &other.example_transactions {
            self.add_example(transaction_hash);
        }
        if self.messages == 1 {
            self.example_transactions.clear();
        }
    }

    fn add_example(&mut self, transaction_hash: Hash256) {
        if self.example_transactions.len() < MAX_TRANSACTION_EXAMPLES
            && !self.example_transactions.contains(&transaction_hash)
        {
            self.example_transactions.push(transaction_hash);
        }
    }
}

#[derive(Default)]
struct OpcodeAccumulator {
    first_masterchain_seqno: Option<u32>,
    latest_masterchain_seqno: Option<u32>,
    total_messages: u64,
    messages_with_opcode: u64,
    counts: BTreeMap<u32, OpcodeAggregate>,
}

impl OpcodeStats {
    pub(crate) fn from_persisted(
        first_masterchain_seqno: Option<u32>,
        latest_masterchain_seqno: Option<u32>,
        total_messages: u64,
        messages_with_opcode: u64,
        counts: impl IntoIterator<Item = (u32, OpcodeAggregate)>,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(OpcodeAccumulator {
                first_masterchain_seqno,
                latest_masterchain_seqno,
                total_messages,
                messages_with_opcode,
                counts: counts.into_iter().collect(),
            })),
        }
    }

    pub(crate) async fn record_batch(&self, batch: &OpcodeBatchStats) {
        self.inner.write().await.record_batch(batch);
    }

    /// Returns the most frequent opcodes collected by this backend database.
    pub async fn snapshot(&self, limit: usize, min_messages: u64) -> OpcodeSnapshot {
        self.inner.read().await.snapshot(limit, min_messages)
    }
}

impl OpcodeBatchStats {
    pub(crate) fn from_batch(batch: &Batch) -> Result<Self> {
        let mut stats = Self {
            masterchain_seqno: batch.masterchain().id().seqno,
            ..Self::default()
        };

        for block in batch.blocks() {
            for lazy_transaction in block.transactions() {
                let transaction_hash = Hash256::new(lazy_transaction.inner().repr_hash().0);
                let transaction = lazy_transaction
                    .load()
                    .context("failed to decode transaction for opcode statistics")?;

                if let Some(cell) = &transaction.in_msg {
                    let message = cell
                        .parse::<Message<'_>>()
                        .context("failed to decode incoming message for opcode statistics")?;
                    if matches!(&message.info, MsgInfo::ExtIn(_)) {
                        stats.record_message(message, transaction_hash);
                    }
                }

                for entry in transaction.out_msgs.iter() {
                    let (_, cell) = entry.context(
                        "failed to decode outgoing message dictionary for opcode statistics",
                    )?;
                    let message = cell
                        .parse::<Message<'_>>()
                        .context("failed to decode outgoing message for opcode statistics")?;
                    stats.record_message(message, transaction_hash);
                }
            }
        }

        Ok(stats)
    }

    pub(crate) fn messages_with_opcode(&self) -> u64 {
        self.counts.values().map(|entry| entry.messages).sum()
    }

    fn record_message(&mut self, message: Message<'_>, transaction_hash: Hash256) {
        self.total_messages = self.total_messages.saturating_add(1);
        let bounced = match &message.info {
            MsgInfo::ExtIn(_) => return,
            MsgInfo::Int(info) => info.bounced,
            MsgInfo::ExtOut(_) => false,
        };
        let Some(opcode) = opcode_from_body(message.body, bounced) else {
            return;
        };
        self.counts
            .entry(opcode)
            .or_default()
            .record(transaction_hash);
    }
}

impl OpcodeAccumulator {
    fn record_batch(&mut self, batch: &OpcodeBatchStats) {
        if self
            .latest_masterchain_seqno
            .is_some_and(|latest| batch.masterchain_seqno <= latest)
        {
            return;
        }

        self.first_masterchain_seqno
            .get_or_insert(batch.masterchain_seqno);
        self.latest_masterchain_seqno = Some(batch.masterchain_seqno);
        self.total_messages = self.total_messages.saturating_add(batch.total_messages);
        self.messages_with_opcode = self
            .messages_with_opcode
            .saturating_add(batch.messages_with_opcode());
        for (&opcode, batch_entry) in &batch.counts {
            self.counts.entry(opcode).or_default().merge(batch_entry);
        }
    }

    fn snapshot(&self, limit: usize, min_messages: u64) -> OpcodeSnapshot {
        let mut matching = self
            .counts
            .iter()
            .filter(|(_, entry)| entry.messages >= min_messages)
            .map(|(&opcode, entry)| (opcode, entry))
            .collect::<Vec<_>>();
        let matching_opcodes = u64::try_from(matching.len()).unwrap_or(u64::MAX);

        let compare = |left: &(u32, &OpcodeAggregate), right: &(u32, &OpcodeAggregate)| {
            right
                .1
                .messages
                .cmp(&left.1.messages)
                .then_with(|| left.0.cmp(&right.0))
        };

        if limit < matching.len() {
            matching.select_nth_unstable_by(limit, compare);
            matching.truncate(limit);
        }
        matching.sort_unstable_by(compare);
        let opcodes = matching
            .into_iter()
            .map(|(opcode, entry)| OpcodeCount {
                opcode,
                messages: entry.messages,
                example_transaction_hashes: entry
                    .example_transactions
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            })
            .collect();

        OpcodeSnapshot {
            first_masterchain_seqno: self.first_masterchain_seqno,
            latest_masterchain_seqno: self.latest_masterchain_seqno,
            total_messages: self.total_messages,
            messages_with_opcode: self.messages_with_opcode,
            total_opcodes: u64::try_from(self.counts.len()).unwrap_or(u64::MAX),
            matching_opcodes,
            opcodes,
        }
    }
}

fn opcode_from_body(mut body: CellSlice<'_>, bounced: bool) -> Option<u32> {
    if bounced {
        body.load_u32().ok()?;
    }
    body.load_u32().ok()
}

#[cfg(test)]
mod tests {
    use tycho_types::cell::CellBuilder;

    use super::*;

    #[test]
    fn reads_regular_and_bounced_opcodes() {
        let mut regular = CellBuilder::new();
        regular.store_u32(0x1234_5678).unwrap();
        let regular = regular.build().unwrap();

        let mut bounced = CellBuilder::new();
        bounced.store_u32(0xffff_ffff).unwrap();
        bounced.store_u32(0x1234_5678).unwrap();
        let bounced = bounced.build().unwrap();

        assert_eq!(
            opcode_from_body(regular.as_slice().unwrap(), false),
            Some(0x1234_5678)
        );
        assert_eq!(
            opcode_from_body(bounced.as_slice().unwrap(), true),
            Some(0x1234_5678)
        );
    }

    #[test]
    fn excludes_incoming_external_messages_from_opcode_counts() {
        let mut body = CellBuilder::new();
        body.store_u32(0x1234_5678).unwrap();
        let body = body.build().unwrap();
        let message = Message {
            info: MsgInfo::ExtIn(Default::default()),
            init: None,
            body: body.as_slice().unwrap(),
            layout: None,
        };
        let mut stats = OpcodeBatchStats::default();

        stats.record_message(message, Hash256::new([1; 32]));

        assert_eq!(stats.total_messages, 1);
        assert!(stats.counts.is_empty());
    }

    #[tokio::test]
    async fn aggregates_batches_once_and_orders_opcodes_by_count() {
        let stats = OpcodeStats::default();
        let batch = OpcodeBatchStats {
            masterchain_seqno: 42,
            total_messages: 4,
            counts: BTreeMap::from([
                (
                    1,
                    OpcodeAggregate {
                        messages: 1,
                        example_transactions: vec![Hash256::new([1; 32])],
                    },
                ),
                (
                    2,
                    OpcodeAggregate {
                        messages: 2,
                        example_transactions: vec![Hash256::new([2; 32]), Hash256::new([3; 32])],
                    },
                ),
            ]),
        };

        stats.record_batch(&batch).await;
        stats.record_batch(&batch).await;

        assert!(
            stats
                .snapshot(usize::MAX, 1)
                .await
                .opcodes
                .iter()
                .find(|entry| entry.opcode == 1)
                .unwrap()
                .example_transaction_hashes
                .is_empty()
        );

        assert_eq!(
            stats.snapshot(1, 2).await,
            OpcodeSnapshot {
                first_masterchain_seqno: Some(42),
                latest_masterchain_seqno: Some(42),
                total_messages: 4,
                messages_with_opcode: 3,
                total_opcodes: 2,
                matching_opcodes: 1,
                opcodes: vec![OpcodeCount {
                    opcode: 2,
                    messages: 2,
                    example_transaction_hashes: vec![
                        Hash256::new([2; 32]).to_string(),
                        Hash256::new([3; 32]).to_string(),
                    ],
                },],
            }
        );
    }
}
