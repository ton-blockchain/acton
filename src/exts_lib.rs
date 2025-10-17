use crate::stack_serialization::{TupleItem, parse_tuple, serialize_tuple};
use num_bigint::BigInt;
use std::ffi::{CStr, CString};
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;

#[derive(Default)]
pub struct Tuple(Vec<TupleItem>);

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

impl Tuple {
    pub fn push_string(&mut self, s: &str) {
        let mut b = CellBuilder::new();
        b.store_bits(s.len() * 8, s.as_bytes()).unwrap();
        self.push(TupleItem::Slice {
            cell: ArcCell::from(b.build().unwrap()),
            start_bits: 0,
            end_bits: (s.len() * 8) as u32,
            end_refs: 0,
            start_refs: 0,
        });
    }

    pub fn push_bool_as_int(&mut self, v: bool) {
        self.push(TupleItem::Int(if v {
            BigInt::from(-1)
        } else {
            BigInt::from(0)
        }));
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgError {
    #[error("stack underflow")]
    StackUnderflow,
    #[error("type mismatch: expected {expected}")]
    TypeMismatch { expected: &'static str },
    #[error("utf8 decode error")]
    Utf8,
    #[error("cell parse error")]
    CellParse,
}

// =======================
pub trait FromStack: Sized {
    fn from_item(item: TupleItem) -> Result<Self, ArgError>;
}

impl FromStack for String {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Slice {
                cell,
                start_bits,
                end_bits,
                ..
            } => {
                let mut p = cell.parser();
                p.skip_bits(start_bits as usize)
                    .map_err(|_| ArgError::CellParse)?;
                let bits = p
                    .load_bits((end_bits - start_bits) as usize)
                    .map_err(|_| ArgError::CellParse)?;
                String::from_utf8(bits).map_err(|_| ArgError::Utf8)
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(String)",
            }),
        }
    }
}

impl FromStack for Vec<u8> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Slice {
                cell,
                start_bits,
                end_bits,
                ..
            } => {
                let mut p = cell.parser();
                p.skip_bits(start_bits as usize)
                    .map_err(|_| ArgError::CellParse)?;
                let bits = p
                    .load_bits((end_bits - start_bits) as usize)
                    .map_err(|_| ArgError::CellParse)?;
                Ok(bits)
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Slice(Bytes)",
            }),
        }
    }
}

impl FromStack for BigInt {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => Ok(i),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

impl FromStack for i64 {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => i.try_into().map_err(|_| ArgError::TypeMismatch {
                expected: "i64 (from Int)",
            }),
            _ => Err(ArgError::TypeMismatch { expected: "Int" }),
        }
    }
}

impl FromStack for bool {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Int(i) => {
                // TON: true = -1, false = 0
                if i == BigInt::from(-1) {
                    Ok(true)
                } else if i == BigInt::from(0) {
                    Ok(false)
                } else {
                    Err(ArgError::TypeMismatch {
                        expected: "Int(-1/0) as bool",
                    })
                }
            }
            _ => Err(ArgError::TypeMismatch {
                expected: "Int(-1/0) as bool",
            }),
        }
    }
}

impl FromStack for Tuple {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Tuple(v) => Ok(Tuple(v)),
            _ => Err(ArgError::TypeMismatch { expected: "Tuple" }),
        }
    }
}

impl FromStack for ArcCell {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Cell(c) => Ok(c),
            _ => Err(ArgError::TypeMismatch { expected: "Cell" }),
        }
    }
}

impl<T: FromStack> FromStack for Option<T> {
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match T::from_item(item) {
            Ok(v) => Ok(Some(v)),
            Err(ArgError::TypeMismatch { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub fn pop_arg<T: FromStack>(t: &mut Tuple) -> Result<T, ArgError> {
    let item = t.pop().ok_or(ArgError::StackUnderflow)?;
    T::from_item(item)
}

#[macro_export]
macro_rules! pop_args {
    ($tuple:expr, $($ty:ty),+ $(,)?) => {{
        #[allow(non_snake_case)]
        {
            let mut __errors: Option<$crate::exts_lib::ArgError> = None;
            let __result = ( $(
                match $crate::exts_lib::pop_arg::<$ty>($tuple) {
                    Ok(v) => v,
                    Err(e) => {
                        __errors = Some(e);
                        Default::default()
                    }
                }
            , )+ );
            if let Some(e) = __errors {
                Err(e)
            } else {
                Ok(__result)
            }
        }
    }};
}

#[macro_export]
macro_rules! extension {
    ($fn_name:ident, ($an:ident : $ty:ty), $body:expr ) => {
        unsafe extern "C" fn $fn_name(ptr: *const c_char) -> *const c_char {
            unsafe {
                $crate::exts_lib::with_tuple(ptr, |__t: &mut $crate::exts_lib::Tuple| {
                    match (|| -> Result<$ty, $crate::exts_lib::ArgError> {
                        $crate::exts_lib::pop_arg::<$ty>(__t)
                    })() {
                        Ok($an) => {
                            $body(__t, ($an,));
                        }
                        Err(e) => {
                            eprintln!("ext_args decode error in {}: {}", stringify!($fn_name), e);
                        }
                    }
                })
            }
        }
    };
    ($fn_name:ident, ($($an:ident : $ty:ty),+ $(,)?), $body:expr ) => {
        unsafe extern "C" fn $fn_name(ptr: *const c_char) -> *const c_char {
            unsafe {
                $crate::exts_lib::with_tuple(ptr, |__t: &mut $crate::exts_lib::Tuple| {
                    match (|| -> Result<($($ty),*), $crate::exts_lib::ArgError> {
                        pop_args!(__t, $($ty),*)
                    })() {
                        Ok(__vals) => {
                            #[allow(non_snake_case, unused_variables)]
                            let ($($an, )*) = __vals;
                            $body(__t, ($($an, )*));
                        }
                        Err(e) => {
                            eprintln!("ext_args decode error in {}: {}", stringify!($fn_name), e);
                        }
                    }
                })
            }
        }
    };
}

pub fn cell_to_ffi_boc64(cell: ArcCell) -> *const c_char {
    let s = cell.to_boc_b64(false).unwrap(); // при желании можно обернуть в Result
    CString::new(s).unwrap().into_raw().cast_const()
}

pub unsafe fn with_tuple(ptr: *const c_char, f: impl FnOnce(&mut Tuple)) -> *const c_char {
    let c = unsafe { CStr::from_ptr(ptr) };
    let boc = match c.to_str() {
        Ok(s) => s,
        Err(_) => return CString::new("").unwrap().into_raw().cast_const(),
    };

    let mut tuple = Tuple(
        ArcCell::from_boc_b64(boc)
            .ok()
            .and_then(|c| parse_tuple(&c).ok())
            .unwrap_or_else(|| Vec::new()),
    );

    f(&mut tuple);

    cell_to_ffi_boc64(serialize_tuple(&tuple).unwrap())
}

#[macro_export]
macro_rules! register_ext_methods {
    ($executor:expr, { $($id:expr => $fname:ident),+ $(,)? }) => {{
        $(
            $executor.register_ext_method($id, $fname);
        )+
    }};
}
