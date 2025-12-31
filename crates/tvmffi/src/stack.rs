use num_bigint::BigInt;
use std::ops::{Deref, DerefMut};
use tonlib_core::cell::ArcCell;

/// Tuple represent a stack of items for the TVM.
#[derive(Default, Debug, Clone, Eq)]
pub struct Tuple(pub Vec<TupleItem>);

impl Tuple {
    /// Create an empty tuple.
    pub fn empty() -> Tuple {
        Tuple(vec![])
    }

    /// Creates wrapper over values with specified type.
    pub fn to_typed(&self, type_name: &String) -> TupleItem {
        TupleItem::TypedTuple {
            type_name: type_name.clone(),
            inner: self.clone(),
        }
    }

    /// Unwrap an empty tuple.
    ///
    /// ```text
    /// (()) -> ()
    /// ```
    pub fn unwrap_empty(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.get(0)
            && item.len() == 0
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
    pub fn unwrap_single(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.get(0)
            && item.len() == 1
        {
            return Tuple(vec![item[0].clone()]);
        }

        (*self).clone()
    }

    pub fn unwrap_tuple(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.get(0) {
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
            BigInt::from(0)
        }));
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TupleItem {
    Null,
    Int(BigInt),
    Nan,
    Cell(ArcCell),
    Slice(ArcCell),
    Builder(ArcCell),
    Tuple(Tuple),
    TypedTuple { type_name: String, inner: Tuple },
}

impl TupleItem {
    /// Creates wrapper over values with specified type.
    pub fn to_typed(&self, type_name: &String) -> TupleItem {
        if let TupleItem::Tuple(item) = self {
            return TupleItem::TypedTuple {
                type_name: type_name.clone(),
                inner: item.clone(),
            };
        }

        TupleItem::TypedTuple {
            type_name: type_name.clone(),
            inner: Tuple(vec![self.clone()]),
        }
    }

    /// Unwrap a single item tuple.
    ///
    /// ```text
    /// (x) -> x
    /// ```
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

impl Default for TupleItem {
    fn default() -> Self {
        TupleItem::Null
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flattened<T>(pub T);

impl<T> Deref for Flattened<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenedOption<T>(pub Option<T>);

impl<T> Deref for FlattenedOption<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
