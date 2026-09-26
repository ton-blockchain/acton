use serde::Serialize;
use tasm_core::types::{ArgValue, Code, Instruction};

/// Static opcode cost, counting each instruction and nested continuation once.
/// This is not a path bound: loops, callees, implicit returns, stack-copy charges,
/// and data-dependent fees require execution to measure. Unknown costs are omitted
/// from `value` and counted separately so clients can show a partial estimate.
#[derive(Default, Serialize)]
pub(super) struct GasEstimate {
    pub value: u64,
    pub has_dynamic_cost: bool,
    pub has_control_flow: bool,
    pub unknown_instructions: usize,
}

impl GasEstimate {
    pub(super) fn for_code(code: &Code) -> Self {
        let mut estimate = Self::default();
        estimate.visit(&code.instructions);
        estimate
    }

    fn visit(&mut self, instructions: &[Instruction]) {
        for instruction in instructions {
            match instruction {
                Instruction::Plain(instruction) => {
                    let spec = &instruction.instr;
                    let costs = &spec.description.gas;
                    let known_cost = costs
                        .iter()
                        .filter(|cost| cost.formula.is_none())
                        .filter_map(|cost| u64::try_from(cost.value).ok())
                        .min();
                    if let Some(cost) = known_cost {
                        self.value += cost;
                    } else {
                        self.unknown_instructions += 1;
                    }
                    self.has_dynamic_cost |= costs.len() != 1
                        || costs.iter().any(|cost| cost.formula.is_some())
                        || matches!(
                            spec.category.as_str(),
                            "dictionary" | "tuple" | "crypto" | "message"
                        )
                        || matches!(spec.name.as_str(), "PUSHINT_LONG" | "STSLICECONST");
                    self.has_control_flow |=
                        spec.control_flow.is_some() || spec.category == "exception";
                    for arg in &instruction.args {
                        self.visit_arg(arg);
                    }
                }
                Instruction::Ref(instruction) => {
                    // A reference at the end of a code cell is an implicit jump.
                    self.value += 10;
                    self.has_control_flow = true;
                    self.visit_arg(&instruction.code);
                }
                Instruction::ExoticCell(_) | Instruction::Slice(_) => {
                    self.has_dynamic_cost = true;
                    self.unknown_instructions += 1;
                }
            }
        }
    }

    fn visit_arg(&mut self, arg: &ArgValue) {
        match arg {
            ArgValue::Code { code, .. } => {
                self.has_control_flow = true;
                self.visit(&code.instructions);
            }
            ArgValue::CodeDictionary(_) => {
                // A dictionary is not a linear part of the containing function's execution.
                self.has_control_flow = true;
                self.unknown_instructions += 1;
            }
            _ => {}
        }
    }
}
