use std::marker::PhantomData;

use crate::cell::{
    Cell, CellBuilder, CellContext, CellSlice, DynCell, EquivalentRepr, Load, LoadCell, Size, Store,
};
use crate::error::Error;
use crate::util::{debug_tuple_field1_finish, unlikely};

/// Lazy-loaded model.
#[repr(transparent)]
pub struct Lazy<T, const EXOTIC: bool = false> {
    cell: Cell,
    _marker: PhantomData<T>,
}

/// Lazy-loaded exotic cell.
pub type LazyExotic<T> = Lazy<T, true>;

impl<T, const EXOTIC: bool> crate::cell::ExactSize for Lazy<T, EXOTIC> {
    #[inline]
    fn exact_size(&self) -> Size {
        Size { bits: 0, refs: 1 }
    }
}

impl<T> std::fmt::Debug for Lazy<T, false> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_tuple_field1_finish(f, "Lazy", &self.cell)
    }
}

impl<T> std::fmt::Debug for Lazy<T, true> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_tuple_field1_finish(f, "LazyExotic", &self.cell)
    }
}

impl<T, const EXOTIC: bool> Eq for Lazy<T, EXOTIC> {}
impl<T, const EXOTIC: bool> PartialEq for Lazy<T, EXOTIC> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cell.as_ref().eq(other.cell.as_ref())
    }
}

impl<T, const EXOTIC: bool> PartialEq<Cell> for Lazy<T, EXOTIC> {
    #[inline]
    fn eq(&self, other: &Cell) -> bool {
        PartialEq::eq(self.inner(), other)
    }
}

impl<T, const EXOTIC: bool> PartialEq<&Cell> for Lazy<T, EXOTIC> {
    #[inline]
    fn eq(&self, other: &&Cell) -> bool {
        PartialEq::eq(self.inner(), *other)
    }
}

impl<T, const EXOTIC: bool> PartialEq<Lazy<T, EXOTIC>> for Cell {
    #[inline]
    fn eq(&self, other: &Lazy<T, EXOTIC>) -> bool {
        PartialEq::eq(self, other.inner())
    }
}

impl<T, const EXOTIC: bool> PartialEq<&Lazy<T, EXOTIC>> for Cell {
    #[inline]
    fn eq(&self, other: &&Lazy<T, EXOTIC>) -> bool {
        PartialEq::eq(self, other.inner())
    }
}

impl<T, const EXOTIC: bool> Clone for Lazy<T, EXOTIC> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            cell: self.cell.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T, const EXOTIC: bool> Lazy<T, EXOTIC> {
    /// Wraps the cell in a typed wrapper.
    #[inline]
    pub fn from_raw(cell: Cell) -> Result<Self, Error> {
        if unlikely(cell.is_exotic() != EXOTIC) {
            return Err(if EXOTIC {
                Error::UnexpectedOrdinaryCell
            } else {
                Error::UnexpectedExoticCell
            });
        }

        Ok(Self {
            cell,
            _marker: PhantomData,
        })
    }

    /// Wraps the cell in a typed wrapper.
    ///
    /// # Safety
    ///
    /// The cell `is_exotic` flag must be in sync with `EXOTIC` type param.
    #[inline]
    pub unsafe fn from_raw_unchecked(cell: Cell) -> Self {
        Self {
            cell,
            _marker: PhantomData,
        }
    }

    /// Converts into the underlying cell.
    #[inline]
    pub fn into_inner(self) -> Cell {
        self.cell
    }

    /// Returns the underlying cell.
    #[inline]
    pub fn inner(&self) -> &Cell {
        &self.cell
    }

    /// Converts into a lazy loader for an equivalent type.
    pub fn cast_into<Q>(self) -> Lazy<Q, EXOTIC>
    where
        Q: EquivalentRepr<T>,
    {
        Lazy {
            cell: self.cell,
            _marker: PhantomData,
        }
    }

    /// Casts itself into a lazy loaded for an equivalent type.
    pub fn cast_ref<Q>(&self) -> &Lazy<Q, EXOTIC>
    where
        Q: EquivalentRepr<T>,
    {
        // SAFETY: Lazy is #[repr(transparent)]
        unsafe { &*(self as *const Self as *const Lazy<Q, EXOTIC>) }
    }

    /// Serializes only the root hash of the cell.
    #[cfg(feature = "serde")]
    pub fn serialize_repr_hash<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(self.cell.repr_hash(), serializer)
    }
}

impl<T, const EXOTIC: bool> AsRef<DynCell> for Lazy<T, EXOTIC> {
    #[inline]
    fn as_ref(&self) -> &DynCell {
        self.cell.as_ref()
    }
}

impl<T, const EXOTIC: bool> std::ops::Deref for Lazy<T, EXOTIC> {
    type Target = Cell;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.cell
    }
}

impl<T: Store, const EXOTIC: bool> Lazy<T, EXOTIC> {
    /// Serializes the provided data and returns the typed wrapper around it.
    pub fn new(data: &T) -> Result<Self, Error> {
        Self::from_raw(ok!(CellBuilder::build_from(data)))
    }

    /// Updates the content with the provided data.
    pub fn set(&mut self, data: &T) -> Result<(), Error> {
        *self = ok!(Self::new(data));
        Ok(())
    }
}

impl<'a, T: Load<'a> + 'a> Lazy<T, false> {
    /// Loads inner data from cell expecting an ordinary cell.
    pub fn load(&'a self) -> Result<T, Error> {
        self.cell.as_ref().parse::<T>()
    }
}

impl<'a, T: LoadCell<'a> + 'a> Lazy<T, true> {
    /// Loads inner data from cell expecting an exotic cell.
    pub fn load(&'a self) -> Result<T, Error> {
        self.cell.as_ref().parse_exotic::<T>()
    }
}

impl<T, const EXOTIC: bool> Store for Lazy<T, EXOTIC> {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_reference(self.cell.clone())
    }
}

impl<'a, T, const EXOTIC: bool> Load<'a> for Lazy<T, EXOTIC> {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_reference_cloned() {
            Ok(cell) => Self::from_raw(cell),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Lazy<T, false>
where
    for<'a> T: serde::Serialize + Load<'a>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let value = ok!(self.load().map_err(serde::ser::Error::custom));
            value.serialize(serializer)
        } else {
            crate::boc::Boc::serialize(&self.cell, serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Lazy<T, true>
where
    for<'a> T: serde::Serialize + LoadCell<'a>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let value = ok!(self.load().map_err(serde::ser::Error::custom));
            value.serialize(serializer)
        } else {
            crate::boc::Boc::serialize(&self.cell, serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, T, const EXOTIC: bool> serde::Deserialize<'de> for Lazy<T, EXOTIC>
where
    T: serde::Deserialize<'de> + Store,
{
    fn deserialize<D>(deserializer: D) -> Result<Lazy<T, EXOTIC>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = T::deserialize(deserializer)?;
            Lazy::new(&value)
        } else {
            let cell = crate::boc::Boc::deserialize(deserializer)?;
            Lazy::from_raw(cell)
        }
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a, T: Store + arbitrary::Arbitrary<'a>> arbitrary::Arbitrary<'a> for Lazy<T, false> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let inner = u.arbitrary::<T>()?;
        Lazy::new(&inner).map_err(|_| arbitrary::Error::IncorrectFormat)
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    #[inline]
    fn try_size_hint(
        depth: usize,
    ) -> arbitrary::Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        <T as arbitrary::Arbitrary>::try_size_hint(depth)
    }
}
