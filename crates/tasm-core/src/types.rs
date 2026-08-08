use crate::printer::{FormatOptions, PrintState, offset_width_for};
use crate::spec::SpecInstruction;
use num_bigint::{BigInt, BigUint};
use std::fmt::Write;
use tycho_types::cell::Cell;

#[derive(Debug, Clone)]
pub enum Instruction {
    Plain(PlainInstruction),
    Ref(RefInstruction),
    ExoticCell(ExoticCellInstruction),
    Slice(SliceInstruction),
}

#[derive(Debug, Clone)]
pub struct ExoticCellInstruction {
    pub source_cell: Option<Cell>,
    pub cell: Cell,
}

#[derive(Debug, Clone)]
pub struct SliceInstruction {
    pub source_cell: Option<Cell>,
    pub cell: Cell,
}

#[derive(Debug, Clone)]
pub struct RefInstruction {
    pub source_cell: Option<Cell>,
    pub code: ArgValue,
}

#[derive(Debug, Clone)]
pub struct PlainInstruction {
    pub name: String,
    pub instr: Box<SpecInstruction>,
    pub source_cell: Option<Cell>,
    pub args: smallvec::SmallVec<[ArgValue; 3]>,
}

#[derive(Debug, Clone)]
pub struct Control {
    pub idx: u64,
}

impl std::fmt::Display for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.idx)
    }
}

#[derive(Debug, Clone)]
pub struct StackRegister {
    pub idx: i64,
}

impl std::fmt::Display for StackRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s{}", self.idx)
    }
}

#[derive(Debug, Clone)]
pub struct Code {
    pub instructions: Vec<Instruction>,
    pub offsets: Option<Vec<u16>>,
}

impl Code {
    #[must_use]
    pub fn print(&self, options: &FormatOptions) -> String {
        let mut s = String::new();
        let mut state = PrintState::default();
        let offset_width = offset_width_for(self);

        if options.show_offsets {
            writeln!(s, "{:<offset_width$}│ instruction", "off").ok();
            writeln!(
                s,
                "{}┼───────────────────────────────────────",
                "─".repeat(offset_width)
            )
            .ok();
        }

        for (i, instruction) in self.instructions.iter().enumerate() {
            let offset = self.offsets.as_ref().and_then(|offs| offs.get(i).copied());
            s.write_str(
                instruction
                    .print(0, options, offset, offset_width, &mut state)
                    .as_str(),
            )
            .ok();
            s.write_str("\n").ok();
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Method {
    pub id: u64,
    pub source: Cell,
    pub instructions: Vec<Instruction>,
    pub offsets: Option<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub struct CodeDictionary {
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone)]
pub enum ArgValue {
    Int(BigInt),
    UInt(BigUint),
    Control(Control),
    StackRegister(StackRegister),
    Cell(Cell),
    Code {
        code: Box<Code>,
        source: Cell,
        offset: u16,
    },
    CodeDictionary(CodeDictionary),
}
