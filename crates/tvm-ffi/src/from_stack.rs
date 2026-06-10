//! This module provides functionality for converting `TupleItem` to Rust types.
//!
//! This module is mostly used for defining FFI functions that are called from the TVM emulator.
use crate::stack::{Tuple, TupleItem};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use thiserror::Error;
use tycho_types::cell::{Cell, HashBytes, Load};
use tycho_types::models::{IntAddr, ShardAccount, StdAddr};

/// An error type for converting `TupleItem` to a Rust type.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArgError {
    #[error("stack underflow")]
    StackUnderflow,
    #[error("tuple length mismatch: expected {expected}, actual {actual}")]
    TupleLengthMismatch { expected: usize, actual: usize },
    #[error("type mismatch: expected {expected}")]
    TypeMismatch { expected: &'static str },
    #[error("cell parse error")]
    CellParse,
    #[error("invalid snake bytes")]
    InvalidSnakeBytes,
}

/// A trait for converting `TupleItem` to a Rust type.
pub trait FromStack: Sized {
    /// Convert a `TupleItem` to a Rust type.
    fn from_item(item: TupleItem) -> Result<Self, ArgError>;
}

/// A trait for converting a TVM tuple stack to a Rust struct.
pub trait FromStackTuple: Sized {
    /// Convert a `Tuple` to a Rust type.
    fn from_tuple(tuple: Tuple) -> Result<Self, ArgError>;
}

impl FromStackTuple for Tuple {
    fn from_tuple(tuple: Tuple) -> Result<Self, ArgError> {
        Ok(tuple)
    }
}

/// Convert a `TupleItem` to a `TupleItem`.
/// This is a no-op to define the Any-like type in FFI functions.
impl FromStack for TupleItem {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        Ok(item)
    }
}

/// Convert a `TupleItem` to an optional value.
///
/// `TupleItem::Null` is mapped to `None`, all other values are decoded as `Some(T)`.
impl<T: FromStack> FromStack for Option<T> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Null => Ok(None),
            other => T::from_item(other).map(Some),
        }
    }
}

/// Convert a `TupleItem` to a String.
/// Note that this conversion is automatically handle snake strings.
impl FromStack for String {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(cell) => Tuple::parse_snake_string(&cell).ok_or(ArgError::CellParse),
            TupleItem::Slice(slice) => Tuple::parse_snake_string(&slice).ok_or(ArgError::CellParse),
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(String)",
            }),
        }
    }
}

/// Convert a `TupleItem` to bytes represented as a snake string in a cell/slice.
impl FromStack for Vec<u8> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => {
                Tuple::parse_snake_bytes(&cell).ok_or(ArgError::InvalidSnakeBytes)
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(bytes)",
            }),
        }
    }
}

/// Convert a `TupleItem` to a list of strings.
impl FromStack for Vec<String> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        Vec::<TupleItem>::from_item(item)?
            .into_iter()
            .map(String::from_item)
            .collect::<Result<Vec<_>, _>>()
    }
}

fn decode_big_array_items(tuple: Tuple) -> Result<Vec<TupleItem>, ArgError> {
    // [topLevel: array<array<T>>, size: int]
    let [TupleItem::Tuple(top_level), TupleItem::Int(size)] = tuple.0.as_slice() else {
        return Err(ArgError::TypeMismatch {
            expected: "Tuple(BigArray<T>)",
        });
    };

    let Some(size) = size.to_usize() else {
        return Err(ArgError::TypeMismatch {
            expected: "Tuple(BigArray<T>)",
        });
    };

    let mut result = Vec::with_capacity(size);
    for bin in top_level.iter() {
        let TupleItem::Tuple(bin_items) = bin else {
            return Err(ArgError::TypeMismatch {
                expected: "Tuple(BigArray<T>)",
            });
        };

        for item in bin_items.iter() {
            if result.len() == size {
                break;
            }
            result.push(item.clone());
        }

        if result.len() == size {
            break;
        }
    }

    if result.len() != size {
        return Err(ArgError::TypeMismatch {
            expected: "Tuple(BigArray<T>)",
        });
    }

    Ok(result)
}

fn decode_vec_like_items(item: TupleItem) -> Result<Vec<TupleItem>, ArgError> {
    let TupleItem::Tuple(tuple) = item else {
        return Err(ArgError::TypeMismatch {
            expected: "Tuple(Array<T> | BigArray<T>)",
        });
    };

    let looks_like_big_array = tuple.len() == 2
        && matches!(tuple.first(), Some(TupleItem::Tuple(_)))
        && matches!(tuple.get(1), Some(TupleItem::Int(_)));

    if looks_like_big_array && let Ok(items) = decode_big_array_items(tuple.clone()) {
        return Ok(items);
    }

    Ok(tuple.0)
}

impl FromStack for Vec<TupleItem> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        decode_vec_like_items(item)
    }
}

/// Convert a `TupleItem` to a `BigInt`.
impl FromStack for BigInt {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => Ok(i),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

/// Convert a `TupleItem` to a bool.
///
/// Note that in the TVM true is -1 and false is 0.
impl FromStack for bool {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => {
                // TVM: true = -1, false = 0
                if i == BigInt::from(-1) {
                    Ok(true)
                } else if i == BigInt::ZERO {
                    Ok(false)
                } else {
                    // Treat any other value as true
                    Ok(true)
                }
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Int(-1/0) as bool",
            }),
        }
    }
}

/// Convert a `TupleItem` to a Tuple.
impl FromStack for Tuple {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Tuple(v) => Ok(v),
            _ => Err(ArgError::TypeMismatch { expected: "Tuple" }),
        }
    }
}

/// Convert a `TupleItem` to a Cell.
impl FromStack for Cell {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(c) | TupleItem::Slice(c) => Ok(c),
            _ => Err(ArgError::TypeMismatch {
                expected: "Cell | Slice",
            }),
        }
    }
}

/// Convert a `TupleItem` to a standard TON address.
impl FromStack for StdAddr {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => {
                cell.parse::<StdAddr>().map_err(|_| ArgError::CellParse)
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(StdAddr)",
            }),
        }
    }
}

/// Convert a `TupleItem` to a internal TON address.
impl FromStack for IntAddr {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => {
                let mut slice = cell.as_slice_allow_exotic();
                IntAddr::load_from(&mut slice).map_err(|_| ArgError::CellParse)
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(IntAddr)",
            }),
        }
    }
}

/// Convert a `TupleItem` to a 32-byte hash.
impl FromStack for HashBytes {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        let (TupleItem::Cell(cell) | TupleItem::Slice(cell)) = item else {
            return Err(ArgError::TypeMismatch {
                expected: "Slice(HashBytes)",
            });
        };

        let mut slice = cell.as_slice().map_err(|_| ArgError::CellParse)?;
        if slice.size_bits() != 256 || slice.size_refs() != 0 {
            return Err(ArgError::CellParse);
        }
        slice.load_u256().map_err(|_| ArgError::CellParse)
    }
}

/// Convert a `TupleItem` to `ShardAccount`.
impl FromStack for ShardAccount {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => cell
                .parse::<ShardAccount>()
                .map_err(|_| ArgError::CellParse),
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(ShardAccount)",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::{Tuple, TupleItem};
    use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Store};
    use tycho_types::models::{OptionalAccount, ShardAccount, StdAddr};

    #[test]
    fn test_string_from_stack() {
        // Test successful string conversion from slice
        let mut tuple = Tuple::empty();
        tuple.push_string_slice("Hello World");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok("Hello World".to_string()));

        // Test empty string
        let mut tuple = Tuple::empty();
        tuple.push_string_slice("");
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok(String::new()));

        // Test large string (snake string)
        let large_string = "A".repeat(200);
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(&large_string);
        let TupleItem::Slice(slice) = &tuple.0[0] else {
            panic!("Expected slice");
        };

        let result = String::from_item(TupleItem::Slice(slice.clone()));
        assert_eq!(result, Ok(large_string));

        // Test invalid UTF-8 (should return CellParse error)
        let mut builder = CellBuilder::new();
        builder.store_raw(&[0xFF, 0xFF], 16).unwrap(); // Invalid UTF-8
        let cell = builder.build().unwrap();

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
        let big_int = BigInt::ZERO;
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
        let result = bool::from_item(TupleItem::Int(BigInt::ZERO));
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
        inner_tuple.push_string_slice("test");
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
        builder.store_raw(b"test", 8).unwrap();
        let cell = builder.build().unwrap();

        let result = Cell::from_item(TupleItem::Cell(cell.clone()));
        assert_eq!(result, Ok(cell));
    }

    #[test]
    fn test_tuple_item_from_stack() {
        // Test TupleItem identity conversion (no-op)
        let original = TupleItem::Int(BigInt::from(42));
        let result = TupleItem::from_item(original.clone());
        assert_eq!(result, Ok(original));

        let mut tuple = Tuple::empty();
        tuple.push_string_slice("test");
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
        tuple.push_string_slice("test");
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

        // Test Cell from non-cell
        let result = Cell::from_item(TupleItem::Int(BigInt::from(42)));
        assert!(matches!(
            result,
            Err(ArgError::TypeMismatch {
                expected: "Cell | Slice"
            })
        ));
    }

    #[test]
    fn test_option_from_stack() {
        let none_val = Option::<BigInt>::from_item(TupleItem::Null).unwrap();
        assert_eq!(none_val, None);

        let some_val = Option::<BigInt>::from_item(TupleItem::Int(BigInt::from(7))).unwrap();
        assert_eq!(some_val, Some(BigInt::from(7)));
    }

    #[test]
    fn test_vec_u8_from_stack() {
        let bytes = vec![1u8, 2, 3, 4, 255];
        let mut tuple = Tuple::empty();
        tuple.push_bytes(&bytes);
        let item = tuple.0[0].clone();

        let parsed = Vec::<u8>::from_item(item).unwrap();
        assert_eq!(parsed, bytes);
    }

    #[test]
    fn test_vec_string_from_stack() {
        let mut words = Tuple::empty();
        words.push_string_slice("one");
        words.push_string_slice("two");
        words.push_string_slice("three");

        let parsed = Vec::<String>::from_item(TupleItem::Tuple(words)).unwrap();
        assert_eq!(parsed, vec!["one", "two", "three"]);
    }

    #[test]
    fn test_vec_tuple_item_from_big_array() {
        let values = vec![
            TupleItem::Int(1.into()),
            TupleItem::Int(2.into()),
            TupleItem::Int(3.into()),
        ];

        let big_array = TupleItem::big_array_from_items(values.clone());
        let parsed = Vec::<TupleItem>::from_item(big_array).unwrap();
        assert_eq!(parsed, values);
    }

    #[test]
    fn test_vec_string_from_big_array() {
        let mut one = Tuple::empty();
        one.push_string_slice("one");
        let mut two = Tuple::empty();
        two.push_string_slice("two");
        let mut three = Tuple::empty();
        three.push_string_slice("three");

        let big_array =
            TupleItem::big_array_from_items(vec![one[0].clone(), two[0].clone(), three[0].clone()]);

        let parsed = Vec::<String>::from_item(big_array).unwrap();
        assert_eq!(parsed, vec!["one", "two", "three"]);
    }

    #[test]
    fn test_hash_bytes_from_stack() {
        let hash = HashBytes([0xAB; 32]);
        let mut builder = CellBuilder::new();
        builder.store_u256(&hash).unwrap();
        let cell = builder.build().unwrap();

        let parsed = HashBytes::from_item(TupleItem::Slice(cell)).unwrap();
        assert_eq!(parsed, hash);
    }

    #[test]
    fn test_std_addr_from_stack() {
        let addr = StdAddr::new(-1, HashBytes([0x11; 32]));
        let mut builder = CellBuilder::new();
        addr.store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();

        let parsed = StdAddr::from_item(TupleItem::Cell(cell)).unwrap();
        assert_eq!(parsed, addr);
    }

    #[test]
    fn test_shard_account_from_stack() {
        let shard = ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).unwrap(),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        };
        let mut builder = CellBuilder::new();
        shard
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();

        let parsed = ShardAccount::from_item(TupleItem::Slice(cell)).unwrap();
        assert_eq!(parsed.last_trans_lt, 0);
        assert_eq!(parsed.last_trans_hash, HashBytes::ZERO);
    }

    #[test]
    fn test_edge_cases() {
        // Test string with odd number of bits (not divisible by 8)
        let mut builder = CellBuilder::new();
        builder.store_raw(&[0xFF], 7).unwrap(); // 7 bits, not divisible by 8
        let cell = builder.build().unwrap();

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
