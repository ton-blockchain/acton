use crate::types::{ArgValue, Instruction};
use tycho_types::boc::Boc;

impl Instruction {
    pub fn print(&self, depth: usize) -> String {
        let indent = "    ".repeat(depth);
        let mut builder = String::new();
        builder.push_str(&indent);
        builder.push_str(&normalize_name(&self.name));
        builder.push(' ');

        for (i, arg) in self.args.iter().enumerate() {
            builder.push_str(&format_arg(arg, depth));
            if i < self.args.len() - 1 {
                builder.push(' ');
            }
        }

        builder.trim_end().to_string()
    }

    pub fn string(&self) -> String {
        self.print(0)
    }
}

impl ArgValue {
    pub fn string(&self) -> String {
        match self {
            ArgValue::Control(c) => c.string(),
            ArgValue::StackRegister(s) => s.string(),
            ArgValue::Int(b) => format!("{}", b),
            _ => panic!("unhandled value: {:?}", self),
        }
    }
}

fn normalize_name(name: &str) -> String {
    if name.starts_with('2') {
        format!("{}2", &name[1..])
    } else {
        name.replace('#', "_")
    }
}

fn format_arg(arg: &ArgValue, depth: usize) -> String {
    let indent = "    ".repeat(depth);
    match arg {
        ArgValue::Control(c) => c.string(),
        ArgValue::StackRegister(s) => s.string(),
        ArgValue::Int(b) => format!("{}", b),
        ArgValue::Cell(s) => {
            let slice = s.as_slice().unwrap();
            if slice.size_refs() == 0 {
                format!("x{{{}}}", slice.display_data().to_string())
            } else {
                format!("boc{{{}}}", Boc::encode_hex(s))
            }
        }
        ArgValue::Code(code) => {
            let mut builder = String::new();
            builder.push_str("{\n");
            for instruction in &code.instructions {
                builder.push_str(&instruction.print(depth + 1));
                builder.push('\n');
            }
            builder.push_str(&indent);
            builder.push('}');
            builder
        }
        ArgValue::CodeDictionary(dict) => {
            let mut builder = String::new();
            builder.push_str("[\n");
            for method in &dict.methods {
                builder.push_str(&indent);
                builder.push_str(&format!("    {} => {{\n", method.id));
                for instruction in &method.instructions {
                    builder.push_str(&instruction.print(depth + 2));
                    builder.push('\n');
                }
                builder.push_str("    ");
                builder.push_str(&indent);
                builder.push_str("}\n");
            }
            builder.push_str(&indent);
            builder.push(']');
            builder
        }
        ArgValue::UInt(v) => format!("{}", v),
    }
}
