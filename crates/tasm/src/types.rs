use crate::spec::SpecInstruction;
use num_bigint::{BigInt, BigUint};
use tycho_types::cell::Cell;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub name: String,
    pub instr: Option<Box<SpecInstruction>>,
    pub args: smallvec::SmallVec<[ArgValue; 3]>,
}

#[derive(Debug, Clone)]
pub struct Control {
    pub idx: u64,
}

impl Control {
    pub fn string(&self) -> String {
        format!("c{}", self.idx)
    }
}

#[derive(Debug, Clone)]
pub struct StackRegister {
    pub idx: i64,
}

impl StackRegister {
    pub fn string(&self) -> String {
        format!("s{}", self.idx)
    }
}

#[derive(Debug, Clone)]
pub struct Code {
    pub instructions: Vec<Instruction>,
}

impl Code {
    pub fn string(&self) -> String {
        let mut builder = String::new();
        for instruction in &self.instructions {
            builder.push_str(&format!("{}\n", instruction.print(0)));
        }
        builder
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string())
    }
}

#[derive(Debug, Clone)]
pub struct Method {
    pub id: u64,
    pub instructions: Vec<Instruction>,
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
    Code(Box<Code>),
    CodeDictionary(CodeDictionary),
}

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.print(0))
    }
}
