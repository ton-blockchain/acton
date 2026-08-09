use tycho_types::cell::DynCell;
use tycho_types::merkle::{FilterAction, MerkleFilter};
use tycho_types::prelude::HashBytes;

pub(crate) struct OldStateCells(Vec<HashBytes>);

impl OldStateCells {
    pub(crate) const fn new(cells: Vec<HashBytes>) -> Self {
        Self(cells)
    }
}

impl MerkleFilter for OldStateCells {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if self.0.iter().any(|hash| hash == cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

pub(crate) fn collect_path_to_hash(
    cell: &DynCell,
    target_hash: &HashBytes,
    path: &mut Vec<HashBytes>,
) -> bool {
    let hash = *cell.repr_hash();
    if &hash == target_hash {
        path.push(hash);
        return true;
    }

    if cell
        .references()
        .any(|child| collect_path_to_hash(child, target_hash, path))
    {
        path.push(hash);
        return true;
    }

    false
}
