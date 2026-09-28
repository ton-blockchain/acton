use super::cell_impl::VirtualCellWrapper;
use super::{Cell, CellDescriptor, CellImpl, CellInner, DynCell, HashBytes};

/// Rule for including cells in the usage tree.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UsageTreeMode {
    /// Include cell on load.
    OnLoad,
    /// Include cell only when accessing data.
    OnDataAccess,
}

/// Usage tree for a family of cells.
pub struct UsageTree {
    state: SharedState,
}

impl UsageTree {
    /// Creates a usage tree with the specified tracking mode.
    pub fn new(mode: UsageTreeMode) -> Self {
        Self {
            state: UsageTreeState::new(mode),
        }
    }

    /// Creates a usage tree with the specified tracking mode
    /// and a specified starting capacity.
    pub fn with_mode_and_capacity(mode: UsageTreeMode, capacity: usize) -> Self {
        Self {
            state: UsageTreeState::with_mode_and_capacity(mode, capacity),
        }
    }

    /// Wraps the specified cell in a usage cell to keep track
    /// of the data or links being accessed.
    pub fn track(&self, cell: &Cell) -> Cell {
        if self.state.mode == UsageTreeMode::OnLoad {
            self.state.insert(cell);
        }
        self.state.wrap(cell.clone())
    }

    /// Returns `true` if the cell with the specified representation hash
    /// is present in this usage tree.
    pub fn contains(&self, repr_hash: &HashBytes) -> bool {
        self.state.contains(repr_hash)
    }

    /// Extends the usage tree with subtree tracker.
    pub fn with_subtrees(self) -> UsageTreeWithSubtrees {
        UsageTreeWithSubtrees {
            state: self.state,
            subtrees: Default::default(),
        }
    }

    /// Returns `true` if there are no cells in the usage tree.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Returns the number of cells in the usage tree.
    pub fn len(&self) -> usize {
        self.state.len()
    }
}

/// Usage tree for a family of cells with subtrees.
pub struct UsageTreeWithSubtrees {
    state: SharedState,
    subtrees: ahash::HashSet<HashBytes>,
}

impl UsageTreeWithSubtrees {
    /// Wraps the specified cell in a usage cell to keep track
    /// of the data or links being accessed.
    pub fn track(&self, cell: &Cell) -> Cell {
        if self.state.mode == UsageTreeMode::OnLoad {
            self.state.insert(cell);
        }
        self.state.wrap(cell.clone())
    }

    /// Returns `true` if the cell with the specified representation hash
    /// is present in this usage tree.
    pub fn contains_direct(&self, repr_hash: &HashBytes) -> bool {
        self.state.as_ref().contains(repr_hash)
    }

    /// Returns `true` if the subtree root with the specified representation hash
    /// is present in this usage tree.
    pub fn contains_subtree(&self, repr_hash: &HashBytes) -> bool {
        self.subtrees.contains(repr_hash)
    }

    /// Adds a subtree to the usage tree.
    /// Returns whether the value was newly inserted.
    pub fn add_subtree(&mut self, root: &DynCell) -> bool {
        self.subtrees.insert(*root.repr_hash())
    }
}

#[cfg(not(feature = "sync"))]
use self::rc::{SharedState, UsageCell, UsageTreeState};
#[cfg(feature = "sync")]
use self::sync::{SharedState, UsageCell, UsageTreeState};

impl CellImpl for UsageCell {
    #[inline]
    fn untrack(self: CellInner<Self>) -> Cell {
        UsageCell::untrack_impl(self)
    }

    fn descriptor(&self) -> CellDescriptor {
        self.cell.descriptor()
    }

    fn data(&self) -> &[u8] {
        if self.should_insert() {
            if let Some(usage_tree) = self.usage_tree.upgrade() {
                usage_tree.insert(&self.cell);
            } else {
                self.set_orphaned();
            }
            self.set_inserted();
        }
        self.cell.data()
    }

    fn bit_len(&self) -> u16 {
        self.cell.bit_len()
    }

    fn reference(&self, index: u8) -> Option<&DynCell> {
        if self.is_orphaned() {
            self.cell.reference(index)
        } else {
            Some(self.load_reference(index)?.as_ref())
        }
    }

    fn reference_cloned(&self, index: u8) -> Option<Cell> {
        if self.is_orphaned() {
            self.cell.reference_cloned(index)
        } else {
            let cell = self.load_reference(index)?.clone();

            #[cfg(not(feature = "sync"))]
            {
                Some(Cell::from(cell as std::rc::Rc<DynCell>))
            }

            #[cfg(feature = "sync")]
            {
                Some(Cell::from(cell as std::sync::Arc<DynCell>))
            }
        }
    }

    fn reference_repr_hash(&self, index: u8) -> Option<HashBytes> {
        self.cell.reference_repr_hash(index)
    }

    fn virtualize(&self) -> &DynCell {
        VirtualCellWrapper::wrap(self)
    }

    fn hash(&self, level: u8) -> &HashBytes {
        self.cell.hash(level)
    }

    fn depth(&self, level: u8) -> u16 {
        self.cell.depth(level)
    }
}

#[cfg(not(feature = "sync"))]
mod rc {
    use std::rc::{Rc, Weak};

    use super::UsageTreeMode;
    use crate::cell::{Cell, DynCell, HashBytes};

    pub type SharedState = Rc<UsageTreeState>;

    type VisitedCells = std::cell::RefCell<ahash::HashSet<HashBytes>>;

    pub struct UsageTreeState {
        pub mode: UsageTreeMode,
        pub visited: VisitedCells,
    }

    impl UsageTreeState {
        pub fn new(mode: UsageTreeMode) -> SharedState {
            Rc::new(Self {
                mode,
                visited: Default::default(),
            })
        }

        pub fn with_mode_and_capacity(mode: UsageTreeMode, capacity: usize) -> SharedState {
            Rc::new(Self {
                mode,
                visited: std::cell::RefCell::new(ahash::HashSet::with_capacity_and_hasher(
                    capacity,
                    Default::default(),
                )),
            })
        }

        pub fn wrap(self: &SharedState, cell: Cell) -> Cell {
            Cell::from(Rc::new(UsageCell::new(cell, Rc::downgrade(self), self.mode)) as Rc<DynCell>)
        }

        #[inline]
        pub fn insert(&self, cell: &Cell) {
            self.visited.borrow_mut().insert(*cell.repr_hash());
        }

        #[inline]
        pub fn contains(&self, repr_hash: &HashBytes) -> bool {
            self.visited.borrow().contains(repr_hash)
        }

        #[inline]
        pub fn is_empty(&self) -> bool {
            self.visited.borrow().is_empty()
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.visited.borrow().len()
        }
    }

    pub struct UsageCell {
        pub cell: Cell,
        pub usage_tree: Weak<UsageTreeState>,
        pub children: std::cell::UnsafeCell<[Option<Rc<Self>>; 4]>,
        pub inserted: std::cell::Cell<bool>,
        pub orphaned: std::cell::Cell<bool>,
        pub mode: UsageTreeMode,
    }

    impl UsageCell {
        pub fn new(cell: Cell, usage_tree: Weak<UsageTreeState>, mode: UsageTreeMode) -> Self {
            Self {
                cell,
                usage_tree,
                children: Default::default(),
                inserted: std::cell::Cell::new(mode == UsageTreeMode::OnLoad),
                orphaned: std::cell::Cell::new(false),
                mode,
            }
        }

        pub fn untrack_impl(self: Rc<Self>) -> Cell {
            self.cell.clone()
        }

        #[inline]
        pub fn is_orphaned(&self) -> bool {
            self.orphaned.get()
        }

        #[inline]
        pub fn set_orphaned(&self) {
            self.orphaned.set(true);
        }

        #[inline]
        pub fn should_insert(&self) -> bool {
            !self.inserted.get()
        }

        #[inline]
        pub fn set_inserted(&self) {
            self.inserted.set(true);
        }

        pub fn load_reference(&self, index: u8) -> Option<&Rc<Self>> {
            if index < 4 {
                let children = unsafe { &mut *self.children.get() };
                Some(match &mut children[index as usize] {
                    Some(value) => value,
                    slot @ None => {
                        let child = self.cell.as_ref().reference_cloned(index)?;
                        if self.mode == UsageTreeMode::OnLoad {
                            if let Some(usage_tree) = self.usage_tree.upgrade() {
                                usage_tree.insert(&child);
                            } else {
                                self.set_orphaned();
                            }
                        }

                        slot.insert(Rc::new(UsageCell::new(
                            child,
                            self.usage_tree.clone(),
                            self.mode,
                        )))
                    }
                })
            } else {
                None
            }
        }
    }
}

#[cfg(feature = "sync")]
mod sync {
    use std::cell::UnsafeCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Once, Weak};

    use super::UsageTreeMode;
    use crate::cell::{Cell, DynCell, HashBytes};

    pub type SharedState = Arc<UsageTreeState>;

    type VisitedCells = scc::HashSet<HashBytes, ahash::RandomState>;

    pub struct UsageTreeState {
        pub mode: UsageTreeMode,
        pub visited: VisitedCells,
    }

    impl UsageTreeState {
        pub fn new(mode: UsageTreeMode) -> SharedState {
            Arc::new(Self {
                mode,
                visited: Default::default(),
            })
        }

        pub fn with_mode_and_capacity(mode: UsageTreeMode, capacity: usize) -> SharedState {
            Arc::new(Self {
                mode,
                visited: VisitedCells::with_capacity_and_hasher(capacity, Default::default()),
            })
        }

        pub fn wrap(self: &SharedState, cell: Cell) -> Cell {
            Cell::from(
                Arc::new(UsageCell::new(cell, Arc::downgrade(self), self.mode)) as Arc<DynCell>,
            )
        }

        #[inline]
        pub fn insert(&self, cell: &Cell) {
            _ = self.visited.insert(*cell.repr_hash());
        }

        #[inline]
        pub fn contains(&self, repr_hash: &HashBytes) -> bool {
            self.visited.contains(repr_hash)
        }

        #[inline]
        pub fn is_empty(&self) -> bool {
            self.visited.is_empty()
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.visited.len()
        }
    }

    pub struct UsageCell {
        pub cell: Cell,
        pub usage_tree: Weak<UsageTreeState>,
        // TODO: Compress into one futex with bitset.
        pub reference_states: [Once; 4],
        pub reference_data: [UnsafeCell<Option<Arc<Self>>>; 4],
        pub inserted: AtomicBool,
        pub orphaned: AtomicBool,
        pub mode: UsageTreeMode,
    }

    impl UsageCell {
        pub fn new(cell: Cell, usage_tree: Weak<UsageTreeState>, mode: UsageTreeMode) -> Self {
            Self {
                // NOTE: Untrack loaded cell to prevent its `UsageTree`s from stacking.
                cell: Cell::untrack(cell),
                usage_tree,
                reference_states: [(); 4].map(|_| Once::new()),
                reference_data: [(); 4].map(|_| UnsafeCell::new(None)),
                inserted: AtomicBool::new(mode == UsageTreeMode::OnLoad),
                orphaned: AtomicBool::new(false),
                mode,
            }
        }

        pub fn untrack_impl(self: Arc<Self>) -> Cell {
            match Arc::try_unwrap(self) {
                Ok(inner) => inner.cell,
                Err(this) => this.cell.clone(),
            }
        }

        #[inline]
        pub fn is_orphaned(&self) -> bool {
            self.orphaned.load(Ordering::Acquire)
        }

        #[inline]
        pub fn set_orphaned(&self) {
            self.orphaned.store(true, Ordering::Release);
        }

        #[inline]
        pub fn should_insert(&self) -> bool {
            !self.inserted.load(Ordering::Acquire)
        }

        #[inline]
        pub fn set_inserted(&self) {
            self.inserted.store(true, Ordering::Release);
        }

        pub fn load_reference(&self, index: u8) -> Option<&Arc<Self>> {
            if index < 4 {
                let mut should_insert = false;
                self.reference_states[index as usize].call_once_force(|_| {
                    let Some(child) = self.cell.as_ref().reference_cloned(index) else {
                        // NOTE: Don't forget to set `None` here if `MaybeUninit` is used.
                        return;
                    };

                    should_insert = self.mode == UsageTreeMode::OnLoad;

                    // SAFETY: `UnsafeCell` data is controlled by the `Once` state.
                    unsafe {
                        *self.reference_data[index as usize].get() = Some(Arc::new(Self::new(
                            child,
                            self.usage_tree.clone(),
                            self.mode,
                        )));
                    };
                });

                // SAFETY: `UnsafeCell` data is controlled by the `Once` state.
                let child = unsafe { &*self.reference_data[index as usize].get().cast_const() };
                if crate::util::unlikely(should_insert)
                    && let Some(child) = child
                {
                    if let Some(usage_tree) = self.usage_tree.upgrade() {
                        usage_tree.insert(&child.cell);
                    } else {
                        self.set_orphaned();
                    }
                }

                child.as_ref()
            } else {
                None
            }
        }
    }

    // SAFETY: `UnsafeCell` data is controlled by the `Once` state.
    unsafe impl Send for UsageCell {}
    unsafe impl Sync for UsageCell {}
}
