//! Sync util for parallel merkle algorithms.

use std::sync::{Arc, Condvar, Mutex};

use crate::cell::{Cell, CellBuilder, CellContext, CellDataBuilder, CellRefsBuilder};
use crate::error::Error;

// === Delayed Cell Stuff ===

/// Optionally deferred tree.
#[derive(Clone)]
pub enum ExtCell {
    /// Direct cell.
    Ordinary(Cell),
    /// A subtree with deferred cell in it.
    Partial(Arc<ExtCellParts>),
    /// A cell yet to be processed.
    Deferred(Promise<Result<ExtCell, Error>>),
}

impl ExtCell {
    /// Wait for the cell.
    pub fn resolve(mut self, context: &(dyn CellContext + Send + Sync)) -> Result<Cell, Error> {
        loop {
            match self {
                ExtCell::Ordinary(cell) => return Ok(cell),
                ExtCell::Partial(parts) => {
                    let parts = Arc::unwrap_or_clone(parts);

                    let mut refs = CellRefsBuilder::default();
                    for child in parts.refs {
                        let cell = child.resolve(context)?;
                        refs.store_reference(cell)?;
                    }

                    return CellBuilder::from_parts(parts.is_exotic, parts.data.clone(), refs)
                        .build_ext(context);
                }
                ExtCell::Deferred(promise) => {
                    self = ok!(promise.wait_cloned());
                }
            }
        }
    }
}

/// Deferred subtree builder.
#[derive(Clone)]
pub struct ExtCellParts {
    /// Cell data.
    pub data: CellDataBuilder,
    /// Whether the cell is exotic.
    pub is_exotic: bool,
    /// Deferred references.
    pub refs: Vec<ExtCell>,
}

/// Deferred refs builder.
pub enum ChildrenBuilder {
    /// Direct refs builder.
    Ordinary(CellRefsBuilder),
    /// Deferred references.
    Extended(Vec<ExtCell>),
}

impl ChildrenBuilder {
    /// Adds a deferred reference to the builder.
    pub fn store_reference(&mut self, cell: ExtCell) -> Result<(), Error> {
        match (&mut *self, cell) {
            (Self::Ordinary(builder), ExtCell::Ordinary(cell)) => builder.store_reference(cell),
            (Self::Ordinary(builder), cell) => {
                let capacity = builder.len() + 1;
                let Self::Ordinary(builder) =
                    std::mem::replace(self, Self::Extended(Vec::with_capacity(capacity)))
                else {
                    // SAFETY: We have just checked the `self` discriminant.
                    unsafe { std::hint::unreachable_unchecked() }
                };

                let Self::Extended(ext_builder) = self else {
                    // SAFETY: We have just updated the `self` with this value.
                    unsafe { std::hint::unreachable_unchecked() }
                };

                for cell in builder {
                    ext_builder.push(ExtCell::Ordinary(cell.clone()));
                }
                ext_builder.push(cell);
                Ok(())
            }
            (Self::Extended(builder), cell) => {
                builder.push(cell);
                Ok(())
            }
        }
    }
}

// === Promise Stuff ===

/// A stuff which will be known in a future.
#[derive(Clone)]
#[repr(transparent)]
pub struct Promise<T> {
    inner: Arc<(Mutex<Option<T>>, Condvar)>,
}

impl<T> Default for Promise<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Promise<T> {
    /// Creates an empty promise.
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }

    /// Sets value of the promise.
    pub fn set(&self, value: T) {
        let (lock, cvar) = &*self.inner;
        let mut data = lock.lock().unwrap();
        *data = Some(value);
        cvar.notify_all();
    }

    /// Waits for some value to be set.
    pub fn wait_cloned(&self) -> T
    where
        T: Clone,
    {
        let (lock, cvar) = &*self.inner;
        let mut data = lock.lock().unwrap();
        loop {
            match &*data {
                None => data = cvar.wait(data).unwrap(),
                Some(value) => break value.clone(),
            }
        }
    }
}
