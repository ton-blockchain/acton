//! Block proof links for blocks that are handed to a node over the network.
//!
//! A node that downloads a block also verifies a `BlockProof` for it
//! (`validator/impl/check-proof.cpp`). For a shard block the proof may be a
//! *link*: a Merkle proof of the block header with no validator signatures. The
//! checker virtualizes the proof, requires its root hash to equal the block root
//! hash, and then unpacks the header: `BlockInfo` with its previous- and
//! masterchain references, a structurally valid `ValueFlow`, and the Merkle
//! update cell (only to read the two state hashes out of it).
//!
//! Everything else — `BlockExtra`, the previous vertical reference, and both
//! sides of the state update — is never loaded, so it is pruned. The result is a
//! few hundred bytes regardless of how large the block is.

use anyhow::Context;
use rustc_hash::FxHashSet;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::merkle::{FilterAction, MerkleFilter, MerkleProof};
use tycho_types::models::block::{Block, BlockId, BlockProof};
use tycho_types::prelude::HashBytes;

/// Builds the serialized `BlockProof` link for one block.
pub fn build_block_proof_link(block_id: &BlockId, block_root: &Cell) -> anyhow::Result<Vec<u8>> {
    let block = block_root
        .parse::<Block>()
        .context("Failed to parse block for its proof link")?;
    let info = block
        .info
        .load()
        .context("Failed to load block info for its proof link")?;

    let mut header = FxHashSet::default();
    header.insert(*block_root.repr_hash());
    header.insert(*block.info.inner().repr_hash());
    header.insert(*info.prev_ref.repr_hash());
    header.insert(*block.state_update.inner().repr_hash());
    if let Some(master_ref) = &info.master_ref {
        header.insert(*master_ref.inner().repr_hash());
    }

    let proof = MerkleProof::create(
        block_root.as_ref(),
        HeaderCells {
            header,
            // The checker validates the whole value flow record, which lives in
            // its own child cell, so the subtree has to stay intact.
            value_flow: *block.value_flow.inner().repr_hash(),
        },
    )
    .build()
    .context("Failed to build block header Merkle proof")?;

    let proof = BlockProof {
        proof_for: *block_id,
        root: CellBuilder::build_from(proof).context("Failed to serialize block header proof")?,
        signatures: None,
    };
    let cell = CellBuilder::build_from(&proof).context("Failed to serialize block proof")?;
    Ok(Boc::encode(cell))
}

/// Keeps the header cells of a block and prunes the rest.
struct HeaderCells {
    header: FxHashSet<HashBytes>,
    value_flow: HashBytes,
}

impl MerkleFilter for HeaderCells {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if cell == &self.value_flow {
            FilterAction::IncludeSubtree
        } else if self.header.contains(cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}
