use crate::stack::{Flattened, FlattenedOption, Tuple, TupleItem};
use num_bigint::BigInt;
use thiserror::Error;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::cell::{CellBuilder, CellFamily, Store};

#[derive(Debug, Error, PartialEq)]
pub enum SerializationError {
    #[error("cell build error")]
    CellBuild,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SerializationOptions {}

pub trait ToStack {
    const FIELD_COUNT: usize = 1;

    fn to_item(&self) -> Result<TupleItem, SerializationError>;

    fn to_tuple(
        &self,
        tuple: &mut Tuple,
        _options: SerializationOptions,
    ) -> Result<(), SerializationError> {
        tuple.push(self.to_item()?);
        Ok(())
    }
}

impl ToStack for TupleItem {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(self.clone())
    }
}

impl ToStack for BigInt {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Int(self.clone()))
    }
}

impl ToStack for bool {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Int(if *self {
            BigInt::from(-1)
        } else {
            BigInt::from(0)
        }))
    }
}

impl ToStack for i32 {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Int(BigInt::from(*self)))
    }
}

impl ToStack for u32 {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Int(BigInt::from(*self)))
    }
}

impl ToStack for u64 {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Int(BigInt::from(*self)))
    }
}

impl ToStack for ArcCell {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Cell(self.clone()))
    }
}

impl ToStack for tycho_types::models::IntAddr {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        let mut builder = CellBuilder::new();
        self.store_into(&mut builder, tycho_types::cell::Cell::empty_context())
            .map_err(|_| SerializationError::CellBuild)?;
        let cell = builder.build().map_err(|_| SerializationError::CellBuild)?;

        let boc = tycho_types::boc::Boc::encode(&cell);
        let arc_cell = ArcCell::from_boc(&boc).map_err(|_| SerializationError::CellBuild)?;
        Ok(TupleItem::Cell(arc_cell))
    }
}

impl ToStack for String {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        let mut tuple = Tuple::empty();
        tuple.push_string(self);
        tuple
            .0
            .pop()
            .ok_or(SerializationError::CellBuild) // Should not happen
    }
}

impl<T: ToStack> ToStack for Option<T> {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        match self {
            Some(v) => v.to_item(),
            None => Ok(TupleItem::Null),
        }
    }
}

impl ToStack for Tuple {
    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Ok(TupleItem::Tuple(self.clone()))
    }
}

impl<T: ToStack> ToStack for Flattened<T> {
    const FIELD_COUNT: usize = T::FIELD_COUNT;

    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        self.0.to_item()
    }

    fn to_tuple(
        &self,
        tuple: &mut Tuple,
        options: SerializationOptions,
    ) -> Result<(), SerializationError> {
        self.0.to_tuple(tuple, options)
    }
}

impl<T: ToStack> ToStack for FlattenedOption<T> {
    const FIELD_COUNT: usize = T::FIELD_COUNT + 1;

    fn to_item(&self) -> Result<TupleItem, SerializationError> {
        Err(SerializationError::CellBuild)
    }

    fn to_tuple(
        &self,
        tuple: &mut Tuple,
        options: SerializationOptions,
    ) -> Result<(), SerializationError> {
        match &self.0 {
            Some(val) => {
                val.to_tuple(tuple, options)?;
                tuple.push_bool(true);
            }
            None => {
                for _ in 0..T::FIELD_COUNT {
                    tuple.push(TupleItem::Null);
                }
                tuple.push_bool(false);
            }
        }
        Ok(())
    }
}
