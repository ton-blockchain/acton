use std::collections::HashMap;

use super::*;
use crate::Hash256;

#[derive(Default)]
struct FakeGraph {
    tip: Option<BlockId>,
    latest_calls: usize,
    blocks: HashMap<BlockIdShort, RawBlock>,
    load_calls: HashMap<BlockIdShort, usize>,
    exact_load_calls: HashMap<BlockId, usize>,
    exact_load_batches: Vec<Vec<BlockId>>,
    frontiers: HashMap<BlockId, Vec<BlockId>>,
    predecessors: HashMap<BlockId, Vec<BlockId>>,
}

impl FakeGraph {
    fn insert(&mut self, id: BlockId) {
        self.blocks.insert(id.into(), RawBlock::new(id, []));
    }
}

#[async_trait]
impl BlockGraphClient for FakeGraph {
    async fn latest_masterchain_block(&mut self) -> Result<BlockId, SourceError> {
        self.latest_calls += 1;
        self.tip
            .ok_or_else(|| SourceError::Transport("fake tip is missing".into()))
    }

    async fn load_block(&mut self, id: BlockIdShort) -> Result<RawBlock, SourceError> {
        *self.load_calls.entry(id).or_default() += 1;
        self.blocks
            .get(&id)
            .cloned()
            .ok_or_else(|| SourceError::Transport(format!("fake block {id:?} is missing")))
    }

    async fn load_block_exact(&mut self, id: BlockId) -> Result<RawBlock, SourceError> {
        *self.exact_load_calls.entry(id).or_default() += 1;
        self.load_block(id.into()).await
    }

    async fn load_blocks_exact(&mut self, ids: &[BlockId]) -> Result<Vec<RawBlock>, SourceError> {
        self.exact_load_batches.push(ids.to_vec());
        let mut blocks = Vec::with_capacity(ids.len());
        for &id in ids {
            blocks.push(self.load_block_exact(id).await?);
        }
        Ok(blocks)
    }

    async fn shard_frontier(&mut self, mc_block: &RawBlock) -> Result<Vec<BlockId>, SourceError> {
        Ok(self
            .frontiers
            .get(&mc_block.id)
            .cloned()
            .unwrap_or_default())
    }

    async fn predecessors(&mut self, block: &RawBlock) -> Result<Vec<BlockId>, SourceError> {
        Ok(self
            .predecessors
            .get(&block.id)
            .cloned()
            .unwrap_or_default())
    }
}

fn id(workchain: i32, shard: u64, seqno: u32, marker: u8) -> BlockId {
    BlockId {
        workchain,
        shard,
        seqno,
        root_hash: Hash256::new([marker; 32]),
        file_hash: Hash256::new([marker.wrapping_add(1); 32]),
    }
}

fn graph_with_masterchain() -> (FakeGraph, BlockId, BlockId) {
    let previous_mc = id(-1, BlockId::FULL_SHARD, 9, 90);
    let current_mc = id(-1, BlockId::FULL_SHARD, 10, 100);
    let mut graph = FakeGraph {
        tip: Some(current_mc),
        ..FakeGraph::default()
    };
    graph.insert(previous_mc);
    graph.insert(current_mc);
    graph.predecessors.insert(current_mc, vec![previous_mc]);
    (graph, previous_mc, current_mc)
}

#[tokio::test]
async fn reuses_tip_and_checkpoint_confirmed_previous_frontier() {
    let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
    let next_mc = id(-1, BlockId::FULL_SHARD, 11, 110);
    graph.tip = Some(next_mc);
    graph.insert(next_mc);
    graph.predecessors.insert(next_mc, vec![current_mc]);

    let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
    let current = source.next_raw_batch(None).await.unwrap().unwrap();
    let next = source
        .next_raw_batch(Some(&current.masterchain.id))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(next.masterchain.id, next_mc);
    assert_eq!(source.client().latest_calls, 1);
    assert_eq!(
        source.client().load_calls,
        HashMap::from([
            (previous_mc.into(), 1),
            (current_mc.into(), 1),
            (next_mc.into(), 1),
        ])
    );
}

#[tokio::test]
async fn does_not_reuse_cached_frontier_for_mismatching_checkpoint() {
    let (mut graph, _, current_mc) = graph_with_masterchain();
    let next_mc = id(-1, BlockId::FULL_SHARD, 11, 110);
    graph.tip = Some(next_mc);
    graph.insert(next_mc);
    graph.predecessors.insert(next_mc, vec![current_mc]);

    let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
    source.next_raw_batch(None).await.unwrap().unwrap();
    let invalid_checkpoint = BlockId {
        workchain: current_mc.workchain,
        shard: current_mc.shard,
        seqno: current_mc.seqno,
        root_hash: Hash256::new([0xee; 32]),
        file_hash: current_mc.file_hash,
    };

    let error = source
        .next_raw_batch(Some(&invalid_checkpoint))
        .await
        .unwrap_err();
    assert!(matches!(error, SourceError::UnexpectedBlock { .. }));
    assert_eq!(source.client().load_calls.get(&current_mc.into()), Some(&2));
}

#[tokio::test]
async fn refreshes_tip_after_reaching_cached_boundary() {
    let (graph, _, current_mc) = graph_with_masterchain();
    let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
    let current = source.next_raw_batch(None).await.unwrap().unwrap();

    assert!(
        source
            .next_raw_batch(Some(&current.masterchain.id))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(source.client().latest_calls, 2);
}

#[tokio::test]
async fn walks_normal_chain_predecessor_first() {
    let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
    let one = id(0, BlockId::FULL_SHARD, 1, 1);
    let two = id(0, BlockId::FULL_SHARD, 2, 2);
    let three = id(0, BlockId::FULL_SHARD, 3, 3);
    for block in [one, two, three] {
        graph.insert(block);
    }
    graph.frontiers.insert(previous_mc, vec![one]);
    graph.frontiers.insert(current_mc, vec![three]);
    graph.predecessors.insert(two, vec![one]);
    graph.predecessors.insert(three, vec![two]);

    let mut source = CanonicalBlockSource::new(graph, 10);
    let batch = source.next_raw_batch(None).await.unwrap().unwrap();
    assert_eq!(
        batch
            .shards
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![two, three]
    );
    assert_eq!(
        source.client().exact_load_calls,
        HashMap::from([(two, 1), (three, 1)])
    );
    assert_eq!(
        source.client().exact_load_batches,
        vec![vec![three], vec![two]]
    );
}

#[tokio::test]
async fn handles_split_without_duplicating_parent() {
    let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
    let parent = id(0, BlockId::FULL_SHARD, 1, 1);
    let (left_shard, right_shard) = rston::models::ShardIdent::new(0, BlockId::FULL_SHARD)
        .unwrap()
        .split()
        .unwrap();
    let left = id(0, left_shard.prefix(), 2, 2);
    let right = id(0, right_shard.prefix(), 2, 3);
    for block in [parent, left, right] {
        graph.insert(block);
    }
    graph.frontiers.insert(previous_mc, vec![parent]);
    graph.frontiers.insert(current_mc, vec![right, left]);
    graph.predecessors.insert(left, vec![parent]);
    graph.predecessors.insert(right, vec![parent]);

    let mut source = CanonicalBlockSource::new(graph, 10);
    let batch = source.next_raw_batch(None).await.unwrap().unwrap();
    let ids = batch
        .shards
        .iter()
        .map(|block| block.id)
        .collect::<HashSet<_>>();
    assert_eq!(ids, HashSet::from([left, right]));
    assert_eq!(source.client().exact_load_batches, vec![vec![left, right]]);
}

#[tokio::test]
async fn handles_merge_from_two_frontier_blocks() {
    let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
    let parent_shard = rston::models::ShardIdent::new(0, BlockId::FULL_SHARD).unwrap();
    let (left_shard, right_shard) = parent_shard.split().unwrap();
    let left = id(0, left_shard.prefix(), 2, 2);
    let right = id(0, right_shard.prefix(), 2, 3);
    let merged = id(0, BlockId::FULL_SHARD, 3, 4);
    for block in [left, right, merged] {
        graph.insert(block);
    }
    graph.frontiers.insert(previous_mc, vec![left, right]);
    graph.frontiers.insert(current_mc, vec![merged]);
    graph.predecessors.insert(merged, vec![right, left]);

    let batch = CanonicalBlockSource::new(graph, 10)
        .next_raw_batch(None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        batch
            .shards
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![merged]
    );
}

#[tokio::test]
async fn rejects_disconnected_midchain_bootstrap() {
    let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
    let unrelated = id(-1, BlockId::FULL_SHARD, 9, 91);
    graph.predecessors.insert(current_mc, vec![unrelated]);

    let error = CanonicalBlockSource::new(graph, 10)
        .next_raw_batch(None)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        SourceError::MasterchainDiscontinuity { previous, next, .. }
            if *previous == previous_mc && *next == current_mc
    ));
}

#[tokio::test]
async fn rejects_loaded_predecessor_that_differs_from_checkpoint() {
    let (graph, previous_mc, _) = graph_with_masterchain();
    let checkpoint = BlockId {
        workchain: previous_mc.workchain,
        shard: previous_mc.shard,
        seqno: previous_mc.seqno,
        root_hash: Hash256::new([0xee; 32]),
        file_hash: previous_mc.file_hash,
    };

    let error = CanonicalBlockSource::new(graph, 0)
        .next_raw_batch(Some(&checkpoint))
        .await
        .unwrap_err();
    assert!(matches!(error, SourceError::UnexpectedBlock { .. }));
}
