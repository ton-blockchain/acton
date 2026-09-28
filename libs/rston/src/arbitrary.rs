//! More specific logic for generating arbitrary data.

use arbitrary::{Arbitrary, Result, Unstructured};

use crate::boc::Boc;
use crate::cell::{Cell, CellBuilder, DynCell, MAX_BIT_LEN};
use crate::util::ArrayVec;

/// [`Arbitrary`] helper for generating trees of only ordinary cells.
#[repr(transparent)]
pub struct OrdinaryCell(pub Cell);

impl std::fmt::Debug for OrdinaryCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&Boc::encode(self), f)
    }
}

impl std::ops::Deref for OrdinaryCell {
    type Target = Cell;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<DynCell> for OrdinaryCell {
    #[inline]
    fn as_ref(&self) -> &DynCell {
        self.0.as_ref()
    }
}

impl From<OrdinaryCell> for Cell {
    #[inline]
    fn from(value: OrdinaryCell) -> Self {
        value.0
    }
}

impl<'a> Arbitrary<'a> for OrdinaryCell {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let OrdinaryCellBuilder(b) = u.arbitrary()?;
        Ok(Self(b.build().unwrap()))
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <OrdinaryCellBuilder as Arbitrary>::size_hint(depth)
    }
}

/// [`Arbitrary`] helper for generating trees of only ordinary cells.
#[repr(transparent)]
#[derive(Debug)]
pub struct OrdinaryCellBuilder(pub CellBuilder);

impl From<OrdinaryCellBuilder> for CellBuilder {
    #[inline]
    fn from(value: OrdinaryCellBuilder) -> Self {
        value.0
    }
}

impl<'a> Arbitrary<'a> for OrdinaryCellBuilder {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let bit_len = u.int_in_range(0..=MAX_BIT_LEN)?;
        let refs = u.int_in_range(0..=4)?;

        let mut b = CellBuilder::new();
        b.store_raw(u.bytes(bit_len.div_ceil(8) as _)?, bit_len)
            .unwrap();

        let mut children = ArrayVec::<Cell, 4>::new();
        for i in 0..refs as u8 {
            let cell = 'cell: {
                if i > 0 {
                    // Allow to reuse cells.
                    if let Some(i) = u.int_in_range(0..=i)?.checked_sub(1) {
                        break 'cell children.get(i).cloned().unwrap();
                    }
                }

                u.arbitrary::<OrdinaryCell>().and_then(check_max_depth)?.0
            };

            b.store_reference(cell.clone()).unwrap();

            // SAFETY: `refs` is at most 4.
            unsafe { children.push(cell) };
        }

        Ok(Self(b))
    }

    #[inline]
    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (3, None)
    }
}

/// [`Arbitrary`] helper for generating a "real-life" balance.
#[cfg(feature = "models")]
#[derive(Debug)]
#[repr(transparent)]
pub struct SimpleBalance(pub crate::models::CurrencyCollection);

#[cfg(feature = "models")]
impl From<SimpleBalance> for crate::models::CurrencyCollection {
    #[inline]
    fn from(value: SimpleBalance) -> Self {
        value.0
    }
}

#[cfg(feature = "models")]
impl<'a> Arbitrary<'a> for SimpleBalance {
    #[inline]
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        const BIG_BALANCE: u128 = 100_000_000_000_000_000_000;

        Ok(Self(crate::models::CurrencyCollection {
            tokens: crate::num::Tokens::new(u.int_in_range(0..=BIG_BALANCE)?),
            other: crate::models::ExtraCurrencyCollection::new(),
        }))
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <crate::num::Tokens as Arbitrary>::size_hint(depth)
    }
}

/// Returns the argument if it doesn't have levels with max depth.
pub fn check_max_depth<T: AsRef<DynCell>>(cell: T) -> Result<T, arbitrary::Error> {
    if cell.as_ref().has_max_depth() {
        return Err(arbitrary::Error::IncorrectFormat);
    }
    Ok(cell)
}
