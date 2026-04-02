use ton_executor::get::step::StepGetExecutor;
use ton_executor::message::step::StepExecutor;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use vmlogs::parser::{CellLike, CellSlice, VmStackValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugCodePosition {
    pub cell_hash: String,
    pub offset: i32,
}

#[derive(Debug, Clone)]
pub struct DebugExecutorSnapshot {
    pub code_position: Option<DebugCodePosition>,
    pub stack_values: Option<Vec<VmStackValue>>,
}

/// Small sum-type wrapper over the live step executors we can debug through the replayer.
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
    }
}
