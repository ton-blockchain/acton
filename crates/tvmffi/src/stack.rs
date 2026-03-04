use num_bigint::BigInt;
use std::ops::{Deref, DerefMut};
use tycho_types::cell::Cell;

/// Tuple represent a stack of items for the TVM.
#[derive(Default, Debug, Clone, Eq)]
pub struct Tuple(pub Vec<TupleItem>);

impl Tuple {
    /// Create an empty tuple.
    #[must_use]
    pub const fn empty() -> Tuple {
        Tuple(vec![])
    }

    /// Creates wrapper over values with specified type.
    #[must_use]
    pub fn to_typed(&self, type_name: &str) -> TupleItem {
        TupleItem::TypedTuple {
            type_name: type_name.to_owned(),
            inner: self.clone(),
        }
    }

    /// Unwrap an empty tuple.
    ///
    /// ```text
    /// (()) -> ()
    /// ```
    #[must_use]
    pub fn unwrap_empty(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first()
            && item.is_empty()
        {
            return Tuple(vec![]);
        }

        (*self).clone()
    }

    /// Unwrap a single item tuple.
    ///
    /// ```text
    /// ((x)) -> (x)
    /// ```
    #[must_use]
    pub fn unwrap_single(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first()
            && item.len() == 1
        {
            return Tuple(vec![item[0].clone()]);
        }

        (*self).clone()
    }

    #[must_use]
    pub fn unwrap_tuple(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first() {
            return Tuple(item.0.clone());
        }

        (*self).clone()
    }

    /// Push a boolean value to the tuple.
    ///
    /// In TVM `true` is represented as `-1` and `false` is represented as `0`.
    pub fn push_bool(&mut self, v: bool) {
        self.push(TupleItem::Int(if v {
            BigInt::from(-1)
        } else {
            BigInt::ZERO
        }));
    }

    pub fn equal_to(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter().zip(other.iter()).all(|(a, b)| a.equal_to(b))
    }
}

impl Deref for Tuple {
    type Target = Vec<TupleItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

/// Represents a stack value in TVM
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TupleItem {
    #[default]
    Null,
    Int(BigInt),
    Nan,
    Cell(Cell),
    Slice(Cell),
    Builder(Cell),
    Tuple(Tuple),
    TypedTuple {
        type_name: String,
        inner: Tuple,
    },
}

impl TupleItem {
    pub fn big_array_from_items(v: Vec<TupleItem>) -> Self {
        const BIN_SIZE: usize = 255;
        const MAX_SIZE: usize = BIN_SIZE * BIN_SIZE;

        let size = v.len();
        assert!(
            size <= MAX_SIZE,
            "BigArray supports at most {MAX_SIZE} items, got {size}"
        );

        // BigArray layout in stack tuple:
        // [topLevel: array<array<T>>, size: int]
        // topLevel stores bins of up to 255 items and keeps only used bins.
        let mut bins = vec![Vec::<TupleItem>::new(); size.div_ceil(BIN_SIZE)];
        for (index, value) in v.into_iter().enumerate() {
            let bin_idx = index / BIN_SIZE;
            bins[bin_idx].push(value);
        }

        let top_level = bins
            .into_iter()
            .map(|bin| Self::Tuple(Tuple(bin)))
            .collect::<Vec<_>>();

        Self::Tuple(Tuple(vec![
            Self::Tuple(Tuple(top_level)),
            Self::Int(BigInt::from(size)),
        ]))
    }

    pub fn big_array_from_vec(v: Vec<BigInt>) -> Self {
        Self::big_array_from_items(v.into_iter().map(Self::Int).collect())
    }
}

impl TupleItem {
    fn equal_to(&self, other: &Self) -> bool {
        // Since strings can be build in different cells, we need to compare string values.
        if let TupleItem::Cell(left) | TupleItem::Slice(left) = self
            && let TupleItem::Cell(right) | TupleItem::Slice(right) = other
            && let Some(left_str) = Tuple::parse_snake_string(left)
            && let Some(right_str) = Tuple::parse_snake_string(right)
        {
            return left_str == right_str;
        }

        self == other
    }
}

impl TupleItem {
    /// Creates wrapper over values with specified type.
    #[must_use]
    pub fn to_typed(&self, type_name: &str) -> TupleItem {
        if let TupleItem::Tuple(item) = self {
            return TupleItem::TypedTuple {
                type_name: type_name.to_owned(),
                inner: item.clone(),
            };
        }

        TupleItem::TypedTuple {
            type_name: type_name.to_owned(),
            inner: Tuple(vec![self.clone()]),
        }
    }

    /// Unwrap a single item tuple.
    ///
    /// ```text
    /// (x) -> x
    /// ```
    #[must_use]
    pub fn unwrap_single(&self) -> TupleItem {
        let TupleItem::Tuple(items) = self else {
            return (*self).clone();
        };

        if items.len() == 1 {
            return items[0].clone();
        }

        (*self).clone()
    }
}
