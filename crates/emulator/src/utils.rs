use crate::executor::RegisterExtMethodCallback;
use serde::{Deserialize, Serialize};
use std::os::raw::c_void;
use ton_executor::get::GetExecutor;
use ton_executor::message::{Executor, RunTransactionArgs};
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};

pub trait StoreExt: Store {
    fn to_cell(&self) -> Cell;
}

impl<T: Store + ?Sized> StoreExt for T {
    fn to_cell(&self) -> Cell {
        let mut builder = CellBuilder::new();
        self.store_into(&mut builder, Cell::empty_context())
            .expect("Failed to store data into cell builder");
        builder.build().expect("Failed to build cell from builder")
    }
}

pub trait BaseExecutor {
    fn step(&self) -> bool;
    fn register_ext_method(
        &mut self,
        id: i32,
        ctx: *mut std::os::raw::c_void,
        callback: RegisterExtMethodCallback,
    );
}

impl BaseExecutor for Executor {
    fn step(&self) -> bool {
        false
    }

    fn register_ext_method(
        &mut self,
        id: i32,
        ctx: *mut c_void,
        callback: RegisterExtMethodCallback,
    ) {
        unsafe {
            self.register_ext_method(id, &mut *ctx, callback).ok();
        }
    }
}

impl BaseExecutor for GetExecutor {
    fn step(&self) -> bool {
        false
    }

    fn register_ext_method(
        &mut self,
        id: i32,
        ctx: *mut c_void,
        callback: RegisterExtMethodCallback,
    ) {
        unsafe {
            self.register_ext_method(id, &mut *ctx, callback).ok();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EmulationInternalParams {
    pub utime: u32,
    pub lt: String, // For some reason this field is a String in C++ code treated as u64
    pub rand_seed: String,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tick_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

impl From<&RunTransactionArgs> for EmulationInternalParams {
    fn from(args: &RunTransactionArgs) -> Self {
        let rand_seed = match &args.random_seed {
            Some(seed) => hex::encode(seed),
            None => String::new(),
        };

        let prev_blocks_info = args
            .prev_blocks_info
            .as_ref()
            .map(|_| panic!("TODO: Implement prev_blocks_info serialization"));

        Self {
            utime: args.now,
            lt: args.lt.to_string(),
            rand_seed,
            ignore_chksig: args.ignore_chksig,
            debug_enabled: args.debug_enabled,
            is_tick_tock: args.is_tick_tock,
            is_tock: args.is_tock,
            prev_blocks_info,
        }
    }
}
