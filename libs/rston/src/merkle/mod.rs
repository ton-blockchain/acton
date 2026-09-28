//! Merkle stuff.

use std::collections::HashSet;
use std::hash::BuildHasher;

pub use self::proof::{MerkleProof, MerkleProofBuilder, MerkleProofExtBuilder, MerkleProofRef};
pub use self::pruned_branch::make_pruned_branch;
#[cfg(all(feature = "rayon", feature = "sync"))]
pub use self::update::ParMerkleUpdateApplier;
pub use self::update::{
    FindCell, MerkleApplyResult, MerkleBuildResult, MerkleStats, MerkleUpdate, MerkleUpdateApplier,
    MerkleUpdateApplierKey, MerkleUpdateBuilder,
};
use crate::cell::{HashBytes, UsageTree, UsageTreeWithSubtrees};

mod proof;
mod pruned_branch;
mod update;

#[cfg(all(feature = "rayon", feature = "sync"))]
pub mod sync_util;

#[cfg(test)]
mod tests;

#[cfg(feature = "sync")]
#[doc(hidden)]
mod __checks {
    use super::*;

    assert_impl_all!(MerkleProof: Send);
    assert_impl_all!(MerkleUpdate: Send);
}

/// A cell tree filter that controls which cells will be included
/// in the Merkle proof or update.
pub trait MerkleFilter {
    /// Returns how the cell should be included in the Merkle proof or update.
    fn check(&self, cell: &HashBytes) -> FilterAction;
}

/// Merkle filter action.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilterAction {
    /// Skip this cell and its subtree.
    Skip,
    /// Include this cell and continue checking its subtree.
    Include,
    /// Include this cell and its subtree.
    IncludeSubtree,
}

impl<T: MerkleFilter + ?Sized> MerkleFilter for &T {
    #[inline]
    fn check(&self, cell: &HashBytes) -> FilterAction {
        <T as MerkleFilter>::check(self, cell)
    }
}

impl MerkleFilter for UsageTree {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if UsageTree::contains(self, cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

impl MerkleFilter for UsageTreeWithSubtrees {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if UsageTreeWithSubtrees::contains_direct(self, cell) {
            FilterAction::Include
        } else if UsageTreeWithSubtrees::contains_subtree(self, cell) {
            FilterAction::IncludeSubtree
        } else {
            FilterAction::Skip
        }
    }
}

impl<S: BuildHasher> MerkleFilter for HashSet<HashBytes, S> {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if HashSet::contains(self, cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

impl<S: BuildHasher> MerkleFilter for HashSet<&HashBytes, S> {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if HashSet::contains(self, cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

#[cfg(feature = "rayon")]
impl<S: BuildHasher + Clone> MerkleFilter for dashmap::DashSet<&'_ HashBytes, S> {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if dashmap::DashSet::contains(self, cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

#[cfg(feature = "rayon")]
impl<S: BuildHasher + Clone> MerkleFilter for dashmap::DashSet<HashBytes, S> {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if dashmap::DashSet::contains(self, cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}
