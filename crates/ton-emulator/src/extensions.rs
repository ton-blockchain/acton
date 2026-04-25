//! This module defines a simple DSL for defining extension functions for the emulator.
#![allow(unsafe_code)]
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use tvm_ffi::from_stack::{ArgError, FromStack};
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub fn pop_arg<T: FromStack>(t: &mut Tuple) -> Result<T, ArgError> {
    let item = t.pop().ok_or(ArgError::StackUnderflow)?;
    T::from_item(item)
}

#[macro_export]
macro_rules! pop_args {
    ($tuple:expr, $($ty:ty),+ $(,)?) => {{
        let mut __errors: Option<tvm_ffi::from_stack::ArgError> = None;
        let __result = ( $(
            match $crate::extensions::pop_arg::<$ty>($tuple) {
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
    }};
}

#[macro_export]
macro_rules! extension {
    ($fn_name:ident in ($ctx_ty:ty) using $body:expr) => {
        unsafe extern "C" fn $fn_name(ctx: *mut $ctx_ty, ptr: *const std::os::raw::c_char) -> *const std::os::raw::c_char {
            unsafe {
                let ctx = &mut *(ctx as *mut $ctx_ty);
                $crate::extensions::with_tuple(ptr, |__t: &mut tvm_ffi::stack::Tuple| {
                    let r: anyhow::Result<()> = $body(ctx, __t);
                    if let Err(e) = r {
                        ctx.asserts.fail(format!("{:#}", e));
                        __t.push(tvm_ffi::stack::TupleItem::Null);
                    }
                })
            }
        }
    };
    ($fn_name:ident in ($ctx_ty:ty) with ($an:ident : $ty:ty) using $body:expr) => {
        unsafe extern "C" fn $fn_name(ctx: *mut $ctx_ty, ptr: *const std::os::raw::c_char) -> *const std::os::raw::c_char {
            unsafe {
                let ctx = &mut *(ctx as *mut $ctx_ty);
                $crate::extensions::with_tuple(ptr, |__t: &mut tvm_ffi::stack::Tuple| {
                    match $crate::extensions::pop_arg::<$ty>(__t) {
                        Ok($an) => {
                            let r: anyhow::Result<()> = $body(ctx, __t, $an);
                            if let Err(e) = r {
                                ctx.asserts.fail(format!("{:#}", e));
                                __t.push(tvm_ffi::stack::TupleItem::Null);
                            }
                        }
                        Err(e) => {
                            eprintln!("ext_args decode error in {}: {}", stringify!($fn_name), e);
                        }
                    }
                })
            }
        }
    };
    ($fn_name:ident in ($ctx_ty:ty) with ($($an:ident : $ty:ty),+ $(,)?) using $body:expr) => {
        unsafe extern "C" fn $fn_name(ctx: *mut $ctx_ty, ptr: *const std::os::raw::c_char) -> *const std::os::raw::c_char {
            unsafe {
                debug_assert!(!ctx.is_null());
                debug_assert!(!ptr.is_null());
                let ctx = &mut *(ctx as *mut $ctx_ty);
                $crate::extensions::with_tuple(ptr, |__t: &mut tvm_ffi::stack::Tuple| {
                    match $crate::pop_args!(__t, $($ty),*) {
                        Ok(__vals) => {
                            #[allow(non_snake_case, unused_variables)]
                            let ($($an, )*) = __vals;
                            let r: anyhow::Result<()> = $body(ctx, __t, $($an, )*);
                            if let Err(e) = r {
                                ctx.asserts.fail(format!("{:#}", e));
                                __t.push(tvm_ffi::stack::TupleItem::Null);
                            }
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

fn cell_to_ffi_boc64(cell: &Cell) -> *const c_char {
    let s = Boc::encode_base64(cell);
    CString::new(s)
        .expect("Failed to create C string from BOC")
        .into_raw()
        .cast_const()
}

/// # Safety
///
/// Well...
pub unsafe fn with_tuple(ptr: *const c_char, f: impl FnOnce(&mut Tuple)) -> *const c_char {
    // SAFETY: We assume ptr is always valid C string
    let c = unsafe { CStr::from_ptr(ptr) };
    let Ok(boc) = c.to_str() else {
        return CString::new("")
            .expect("cannot create empty CString")
            .into_raw()
            .cast_const();
    };

    let mut tuple = Boc::decode_base64(boc)
        .ok()
        .and_then(|c| tvm_ffi::serde::parse_tuple(&c).ok())
        .unwrap_or_else(Tuple::empty);

    f(&mut tuple);

    cell_to_ffi_boc64(&tvm_ffi::serde::serialize_tuple(&tuple).expect("Failed to serialize tuple"))
}

#[macro_export]
macro_rules! register_ext_methods {
    (@register_one $executor:expr, $ctx:expr, $id:expr => $fname:ident, $stack_items_count:expr) => {
        $executor
            .register_ext_method($id, ($ctx), $stack_items_count, $fname)
            .expect(&format!("cannot register extension with id: {}", $id));
    };
    (@register_one $executor:expr, $ctx:expr, $id:expr => $fname:ident) => {
        $executor
            .register_ext_method($id, ($ctx), $fname)
            .expect(&format!("cannot register extension with id: {}", $id));
    };
    ($executor:expr, $ctx:expr, { $($id:expr => $fname:ident),+ $(,)? }) => {{
        $(
            $crate::register_ext_methods!(@register_one $executor, $ctx, $id => $fname);
        )+
    }};
    ($executor:expr, $ctx:expr, { $($id:expr => $fname:ident : $stack_items_count:expr),+ $(,)? }) => {{
        $(
            $crate::register_ext_methods!(@register_one $executor, $ctx, $id => $fname, $stack_items_count);
        )+
    }};
}
