//! `DebugExecutorHandle` normalizes the live step executors used by runtime debugging.
//! The replayer only cares about "where execution is now / what is on stack / which
//! runtime registers are visible", regardless of whether the boundary came from
//! `send_message` or `run_get_method`.

use ton_executor::get::step::StepGetExecutor;
use ton_executor::message::step::StepExecutor;
use tvm_ffi::serde::parse_tuple_item;
use tvm_ffi::stack::{Tuple, TupleItem};
use tvm_logs::parser::{CellLike, CellSlice, VmStackValue};
use tycho_types::boc::Boc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugCodePosition {
    pub cell_hash: String,
    pub offset: i32,
}

#[derive(Debug, Clone)]
pub struct DebugExecutorSnapshot {
    /// Raw code position + stack snapshot used to synthesize replayer events.
    pub code_position: Option<DebugCodePosition>,
    pub stack_values: Option<Vec<VmStackValue>>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeDebugSnapshot {
    /// Values exposed under the DAP "Registers" scope for live runtimes.
    pub stack_values: Vec<VmStackValue>,
    pub c4: Option<VmStackValue>,
    pub c5: Option<VmStackValue>,
    pub c7: Option<VmStackValue>,
}

/// Small sum-type wrapper over the live step executors we can debug through the replayer.
/// `Message` drives nested transaction execution, `Get` drives nested get methods.
#[derive(Clone)]
pub enum DebugExecutorHandle {
    Get(StepGetExecutor),
    Message(StepExecutor),
}

impl DebugExecutorHandle {
    #[must_use]
    pub fn step(&self) -> bool {
        match self {
            DebugExecutorHandle::Get(get) => get.step(),
            DebugExecutorHandle::Message(msg) => msg.step(),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> DebugExecutorSnapshot {
        DebugExecutorSnapshot {
            code_position: self.code_position(),
            stack_values: self.stack_values(),
        }
    }

    #[must_use]
    pub fn runtime_snapshot(&self) -> RuntimeDebugSnapshot {
        // c4/c5/c7 are the registers the UI can currently explain meaningfully:
        // storage, outgoing actions and temporary runtime environment.
        RuntimeDebugSnapshot {
            stack_values: self.stack_values().unwrap_or_default(),
            c4: self.control_register_value(4),
            c5: self.control_register_value(5),
            c7: self.c7_value(),
        }
    }

    #[must_use]
    pub fn current_instruction(&self) -> Option<String> {
        let instr = match self {
            DebugExecutorHandle::Get(get) => get.get_current_instr(),
            DebugExecutorHandle::Message(msg) => msg.get_current_instr(),
        };
        let instr = instr.trim();
        if instr.is_empty() || instr == "unknown" {
            None
        } else {
            Some(instr.to_owned())
        }
    }

    #[must_use]
    pub fn uncaught_exception_code(&self) -> Option<String> {
        let code = match self {
            DebugExecutorHandle::Get(get) => get.get_uncaught_exception_code(),
            DebugExecutorHandle::Message(msg) => msg.get_uncaught_exception_code(),
        }?;
        Some(code.to_string())
    }

    #[must_use]
    fn code_pos_text(&self) -> String {
        match self {
            DebugExecutorHandle::Get(get) => get.get_code_pos(),
            DebugExecutorHandle::Message(msg) => msg.get_code_pos(),
        }
    }

    #[must_use]
    fn stack_boc_base64(&self) -> String {
        match self {
            DebugExecutorHandle::Get(get) => get.get_stack(),
            DebugExecutorHandle::Message(msg) => msg.get_stack(),
        }
    }

    #[must_use]
    pub fn get_c7(&self) -> String {
        match self {
            DebugExecutorHandle::Get(get) => get.get_c7(),
            DebugExecutorHandle::Message(msg) => msg.get_c7(),
        }
    }

    #[must_use]
    pub fn get_control_register(&self, idx: usize) -> String {
        match self {
            DebugExecutorHandle::Get(get) => get.get_control_register(idx),
            DebugExecutorHandle::Message(msg) => msg.get_control_register(idx),
        }
    }

    #[must_use]
    fn code_position(&self) -> Option<DebugCodePosition> {
        // Step executors expose code position as `cell_hash:offset`, so parse it
        // once here and keep the replayer free from executor-specific string APIs.
        let pos = self.code_pos_text();
        let (cell_hash, offset): (&str, &str) = pos.split_once(':')?;
        let offset = offset.parse::<i32>().ok()?;
        Some(DebugCodePosition {
            cell_hash: cell_hash.to_owned(),
            offset,
        })
    }

    #[must_use]
    fn stack_values(&self) -> Option<Vec<VmStackValue>> {
        // Live SBS executors expose stack as a tuple BoC. Convert it to the same
        // `VmStackValue` shape that VM-log replay uses so rendering stays shared.
        let stack = self.stack_boc_base64();
        let stack_cell = Boc::decode_base64(&stack).ok()?;
        let stack_tuple = Tuple::deserialize(&stack_cell).ok()?;
        Some(
            stack_tuple
                .iter()
                .map(tuple_item_to_vm_stack_value)
                .collect(),
        )
    }

    fn c7_value(&self) -> Option<VmStackValue> {
        parse_base64_tuple_item_to_vm_stack_value(&self.get_c7())
    }

    fn control_register_value(&self, idx: usize) -> Option<VmStackValue> {
        parse_base64_tuple_item_to_vm_stack_value(&self.get_control_register(idx))
    }
}

impl From<StepGetExecutor> for DebugExecutorHandle {
    fn from(value: StepGetExecutor) -> Self {
        Self::Get(value)
    }
}

impl From<StepExecutor> for DebugExecutorHandle {
    fn from(value: StepExecutor) -> Self {
        Self::Message(value)
    }
}

fn tuple_item_to_vm_stack_value(item: &TupleItem) -> VmStackValue {
    match item {
        TupleItem::Null => VmStackValue::Null,
        TupleItem::Int(v) => VmStackValue::Integer(v.to_string()),
        TupleItem::Nan => VmStackValue::NaN,
        TupleItem::Cell(cell) => VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(cell))),
        // `get_stack()` exposes a whole cell for slice values, but not the exact viewed
        // bit/ref window from VM logs, so range information is intentionally left absent.
        TupleItem::Slice(cell) => VmStackValue::CellSlice(CellSlice {
            value: Boc::encode_hex(cell),
            bits: None,
            refs: None,
        }),
        TupleItem::Builder(cell) => VmStackValue::Builder(Boc::encode_hex(cell)),
        TupleItem::Tuple(items) => {
            VmStackValue::Tuple(items.iter().map(tuple_item_to_vm_stack_value).collect())
        }
        TupleItem::TypedTuple { inner, .. } => {
            VmStackValue::Tuple(inner.iter().map(tuple_item_to_vm_stack_value).collect())
        }
        TupleItem::Cont(item) => VmStackValue::Continuation(Boc::encode_base64(item.code.clone())),
    }
}

fn parse_base64_tuple_item_to_vm_stack_value(boc_base64: &str) -> Option<VmStackValue> {
    let cell = Boc::decode_base64(boc_base64).ok()?;
    let mut slice = cell.as_slice_allow_exotic();
    let item = parse_tuple_item(&mut slice).ok()?;
    Some(tuple_item_to_vm_stack_value(&item))
}
