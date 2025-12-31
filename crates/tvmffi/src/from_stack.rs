//! This module provides functionality for converting TupleItem to Rust types.
//!
//! This module is mostly used for defining FFI functions that are called from the TVM emulator.
use crate::stack::{Flattened, FlattenedOption, Tuple, TupleItem};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use thiserror::Error;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::cell::Load;
use tycho_types::models::IntAddr;

/// An error type for converting TupleItem to a Rust type.
#[derive(Debug, Error, PartialEq)]
pub enum ArgError {
    #[error("stack underflow")]
    StackUnderflow,
    #[error("type mismatch: expected {expected}")]
    TypeMismatch { expected: &'static str },
    #[error("cell parse error")]
    CellParse,
    #[error("tuple size mismatch: expected {expected}, got {actual}")]
    TupleSizeMismatch { expected: usize, actual: usize },
    #[error("extra elements in tuple: expected {expected}, got {actual}")]
    ExtraElements { expected: usize, actual: usize },
    #[error("missing elements in tuple: expected {expected}, got {actual}")]
    MissingElements { expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeserializationOptions {
    pub allow_extra: bool,
    pub allow_missing: bool,
}

/// A trait for converting TupleItem to a Rust type.
pub trait FromStack: Sized {
    /// Number of items this type consumes on the stack when flattened.
    const FIELD_COUNT: usize = 1;

    /// Convert a TupleItem to a Rust type.
    fn from_item(item: TupleItem) -> Result<Self, ArgError>;

    /// Convert from a tuple at a specific offset.
    /// By default, it just takes one item.
    fn from_tuple(
        tuple: &Tuple,
        offset: &mut usize,
        _options: DeserializationOptions,
    ) -> Result<Self, ArgError> {
        let item = tuple
            .get(*offset)
            .cloned()
            .ok_or(ArgError::StackUnderflow)?;
        *offset += 1;
        Self::from_item(item)
    }
}

/// Convert a TupleItem to a TupleItem.
/// This is a no-op to define the Any-like type in FFI functions.
impl FromStack for TupleItem {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        Ok(item)
    }
}

/// Convert a TupleItem to a String.
/// Note that this conversion is automatically handle snake strings.
impl FromStack for String {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Slice(slice) => Tuple::parse_snake_string(&slice).ok_or(ArgError::CellParse),
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(String)",
            }),
        }
    }
}

/// Convert a TupleItem to a BigInt.
impl FromStack for BigInt {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => Ok(i),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

impl FromStack for i32 {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => i.to_i32().ok_or(ArgError::TypeMismatch { expected: "i32" }),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

impl FromStack for u32 {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => i.to_u32().ok_or(ArgError::TypeMismatch { expected: "u32" }),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

impl FromStack for u64 {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => i.to_u64().ok_or(ArgError::TypeMismatch { expected: "u64" }),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

/// Convert a TupleItem to a bool.
///
/// Note that in the TVM true is -1 and false is 0.
impl FromStack for bool {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => {
                // TVM: true = -1, false = 0
                if i == BigInt::from(-1) {
                    Ok(true)
                } else if i == BigInt::from(0) {
                    Ok(false)
                } else {
                    // Treat any other value as true
                    Ok(true)
                }
            }
            TupleItem::Null => Ok(false),
            _ => Err(ArgError::TypeMismatch {
                expected: "Int(-1/0) as bool",
            }),
        }
    }
}

/// Convert a TupleItem to a Tuple.
impl FromStack for Tuple {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Tuple(v) => Ok(v),
            _ => Err(ArgError::TypeMismatch { expected: "Tuple" }),
        }
    }
}

/// Convert a TupleItem to a Cell.
impl FromStack for ArcCell {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(c) => Ok(c),
            _ => Err(ArgError::TypeMismatch { expected: "Cell" }),
        }
    }
}

impl FromStack for IntAddr {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        let cell = match item {
            TupleItem::Cell(cell) => cell,
            TupleItem::Slice(cell) => cell,
            _ => {
                return Err(ArgError::TypeMismatch {
                    expected: "Cell or Slice(IntAddr)",
                });
            }
        };

        let boc = cell.to_boc(false).map_err(|_| ArgError::CellParse)?;
        let cell_parsed = tycho_types::boc::Boc::decode(&boc).map_err(|_| ArgError::CellParse)?;
        let mut slice = cell_parsed.as_slice().map_err(|_| ArgError::CellParse)?;
        IntAddr::load_from(&mut slice).map_err(|_| ArgError::CellParse)
    }
}

impl<T: FromStack> FromStack for Option<T> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Null => Ok(None),
            _ => Ok(Some(T::from_item(item)?)),
        }
    }
}

impl<T: FromStack> FromStack for Flattened<T> {
    const FIELD_COUNT: usize = T::FIELD_COUNT;

    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        T::from_item(item).map(Flattened)
    }

    fn from_tuple(
        tuple: &Tuple,
        offset: &mut usize,
        options: DeserializationOptions,
    ) -> Result<Self, ArgError> {
        T::from_tuple(tuple, offset, options).map(Flattened)
    }
}

impl<T: FromStack> FromStack for FlattenedOption<T> {
    const FIELD_COUNT: usize = T::FIELD_COUNT + 1;

    fn from_item(_item: TupleItem) -> Result<Self, ArgError> {
        Err(ArgError::TypeMismatch {
            expected: "FlattenedOption (multiple items)",
        })
    }

    fn from_tuple(
        tuple: &Tuple,
        offset: &mut usize,
        options: DeserializationOptions,
    ) -> Result<Self, ArgError> {
        let flag_pos = *offset + T::FIELD_COUNT;
        let flag_item = tuple.get(flag_pos).cloned().ok_or(ArgError::StackUnderflow)?;

        let is_some = bool::from_item(flag_item)?;

        if is_some {
            let val = T::from_tuple(tuple, offset, options)?;
            *offset += 1; // consume flag
            Ok(FlattenedOption(Some(val)))
        } else {
            *offset += T::FIELD_COUNT + 1;
            Ok(FlattenedOption(None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::{Tuple, TupleItem};
    use tonlib_core::cell::CellBuilder;

    #[test]
    fn test_string_from_stack() {
        // Test successful string conversion from slice
        let mut tuple = Tuple::empty();
        tuple.push_string("Hello World");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok("Hello World".to_string()));

        // Test empty string
        let mut tuple = Tuple::empty();
        tuple.push_string("");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok("".to_string()));

        // Test large string (snake string)
        let large_string = "A".repeat(200);
        let mut tuple = Tuple::empty();
        tuple.push_string(&large_string);
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok(large_string));

        // Test invalid UTF-8 (should return CellParse error)
        let mut builder = CellBuilder::new();
        builder.store_bits(16, &[0xFF, 0xFF]).unwrap(); // Invalid UTF-8
        let cell = tonlib_core::cell::ArcCell::from(builder.build().unwrap());

        let result = String::from_item(TupleItem::Slice(cell));
        assert!(matches!(result, Err(ArgError::CellParse)));
    }

    #[test]
    fn test_bigint_from_stack() {
        // Test positive BigInt
        let big_int = BigInt::from(42);
        let result = BigInt::from_item(TupleItem::Int(big_int.clone()));
        assert_eq!(result, Ok(big_int));

        // Test negative BigInt
        let big_int = BigInt::from(-123);
        let result = BigInt::from_item(TupleItem::Int(big_int.clone()));
        assert_eq!(result, Ok(big_int));

        // Test zero
        let big_int = BigInt::from(0);
        let result = BigInt::from_item(TupleItem::Int(big_int.clone()));
        assert_eq!(result, Ok(big_int));

        // Test large BigInt
        let big_int = BigInt::from(2).pow(256); // Very large number
        let result = BigInt::from_item(TupleItem::Int(big_int.clone()));
        assert_eq!(result, Ok(big_int));
    }

    #[test]
    fn test_bool_from_stack() {
        // Test true (-1)
        let result = bool::from_item(TupleItem::Int(BigInt::from(-1)));
        assert_eq!(result, Ok(true));

        // Test false (0)
        let result = bool::from_item(TupleItem::Int(BigInt::from(0)));
        assert_eq!(result, Ok(false));

        // Test other values treated as true
        let result = bool::from_item(TupleItem::Int(BigInt::from(1)));
        assert_eq!(result, Ok(true));

        let result = bool::from_item(TupleItem::Int(BigInt::from(42)));
        assert_eq!(result, Ok(true));

        let result = bool::from_item(TupleItem::Int(BigInt::from(-42)));
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_tuple_from_stack() {
        // Test successful tuple conversion
        let mut inner_tuple = Tuple::empty();
        inner_tuple.push_string("test");
        inner_tuple.push(TupleItem::Int(BigInt::from(42)));

        let tuple_item = TupleItem::Tuple(Tuple(inner_tuple.0.clone()));
        let result = Tuple::from_item(tuple_item);
        assert_eq!(result, Ok(inner_tuple));

        // Test empty tuple
        let empty_tuple = Tuple::empty();
        let tuple_item = TupleItem::Tuple(Tuple(empty_tuple.0.clone()));
        let result = Tuple::from_item(tuple_item);
        assert_eq!(result, Ok(empty_tuple));
    }

    #[test]
    fn test_cell_from_stack() {
        // Test successful cell conversion
        let mut builder = CellBuilder::new();
        builder.store_bits(8, b"test").unwrap();
        let cell = tonlib_core::cell::ArcCell::from(builder.build().unwrap());

        let result = ArcCell::from_item(TupleItem::Cell(cell.clone()));
        assert_eq!(result, Ok(cell));
    }

    #[test]
    fn test_tuple_item_from_stack() {
        // Test TupleItem identity conversion (no-op)
        let original = TupleItem::Int(BigInt::from(42));
        let result = TupleItem::from_item(original.clone());
        assert_eq!(result, Ok(original));

        let mut tuple = Tuple::empty();
        tuple.push_string("test");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };
        let original = TupleItem::Slice(slice.clone());
        let result = TupleItem::from_item(original.clone());
        assert_eq!(result, Ok(original));
    }

    #[test]
    fn test_type_mismatch_errors() {
        // Test String from non-slice
        let result = String::from_item(TupleItem::Int(BigInt::from(42)));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch {
                expected: "Slice(String)"
            })
        ));

        // Test BigInt from non-int
        let mut tuple = Tuple::empty();
        tuple.push_string("test");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };
        let result = BigInt::from_item(TupleItem::Slice(slice.clone()));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch { expected: "Int" })
        ));

        // Test bool from non-int
        let result = bool::from_item(TupleItem::Tuple(Tuple::empty()));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch {
                expected: "Int(-1/0) as bool"
            })
        ));

        // Test Tuple from non-tuple
        let result = Tuple::from_item(TupleItem::Int(BigInt::from(42)));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch { expected: "Tuple" })
        ));

        // Test ArcCell from non-cell
        let result = ArcCell::from_item(TupleItem::Int(BigInt::from(42)));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch { expected: "Cell" })
        ));
    }

    #[test]
    fn test_edge_cases() {
        // Test string with odd number of bits (not divisible by 8)
        let mut builder = CellBuilder::new();
        builder.store_bits(7, &[0xFF]).unwrap(); // 7 bits, not divisible by 8
        let cell = ArcCell::from(builder.build().unwrap());

        let result = String::from_item(TupleItem::Slice(cell));
        assert!(matches!(result, Err(ArgError::CellParse)));

        // Test very large tuple
        let mut large_tuple = Tuple::empty();
        for i in 0..1000 {
            large_tuple.push(TupleItem::Int(BigInt::from(i)));
        }
        let tuple_item = TupleItem::Tuple(Tuple(large_tuple.0.clone()));
        let result = Tuple::from_item(tuple_item);
        assert_eq!(result.map(|t| t.0.len()), Ok(1000));
    }
}
