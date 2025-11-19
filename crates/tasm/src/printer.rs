use crate::types::{ArgValue, Instruction};
use std::fs;
use tolkc::source_map::SourceMap;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

const OFFSET_PADDING: &str = "    │ ";

#[derive(Clone, Default)]
pub struct FormatOptions {
    pub show_hashes: bool,
    pub show_offsets: bool,
    pub source_map: Option<Box<SourceMap>>,
}

impl Instruction {
    pub fn print(&self, depth: usize, opts: &FormatOptions, offset: Option<u16>) -> String {
        let indent = "    ".repeat(depth);
        let mut builder = String::new();

        if opts.show_offsets {
            if let Some(off) = offset {
                builder.push_str(&format!("{:<4}│ ", off));
            } else {
                builder.push_str("     │");
            }
        }

        builder.push_str(&indent);
        builder.push_str(&normalize_name(&self.name));
        builder.push(' ');

        for (i, arg) in self.args.iter().enumerate() {
            builder.push_str(&format_arg(arg, depth, opts));
            if i < self.args.len() - 1 {
                builder.push(' ');
            }
        }

        let result = builder.trim_end().to_string();
        let padding = 100_usize.saturating_sub(builder.len());

        if let Some(source_map) = &opts.source_map
            && let Some(off) = offset
        {
            if let Some(locations) =
                get_source_locations(&source_map, self.source_cell.as_ref(), off as i32)
                && !locations.is_empty()
            {
                let source_contexts: Vec<String> = locations
                    .iter()
                    .filter_map(|location| format_source_context(location))
                    .collect();

                if !source_contexts.is_empty() {
                    let before = format!("    └{}┐\n", "─".repeat(56));
                    let after = format!("    ┌{}┘", "─".repeat(56));
                    let source_output = source_contexts.join("");
                    return format!(
                        "{}{:>padding$}\n{before}{}{after}",
                        result, "", source_output
                    );
                }
            }
        }

        result
    }
}

fn get_source_locations<'a>(
    source_map: &'a SourceMap,
    cell: Option<&Cell>,
    offset: i32,
) -> Option<Vec<&'a tolkc::source_map::SourceLocation>> {
    if let Some(cell) = cell {
        let hash = cell.repr_hash().to_string().to_uppercase();
        if let Some(marks) = source_map.debug_marks.get(&hash) {
            let debug_ids: Vec<i64> = marks
                .iter()
                .filter_map(|(mark_offset, debug_id)| {
                    if *mark_offset == offset {
                        Some(*debug_id as i64)
                    } else {
                        None
                    }
                })
                .collect();

            if !debug_ids.is_empty() {
                let locations: Vec<&tolkc::source_map::SourceLocation> = debug_ids
                    .iter()
                    .filter_map(|debug_id| {
                        source_map
                            .high_level
                            .locations
                            .iter()
                            .find(|loc| loc.idx == *debug_id)
                            .map(|loc| &loc.loc)
                    })
                    .collect();

                if !locations.is_empty() {
                    return Some(locations);
                }
            }
        }
    }
    None
}

fn format_source_context(location: &tolkc::source_map::SourceLocation) -> Option<String> {
    let content = fs::read_to_string(&location.file).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    let line_idx = (location.line as usize).saturating_sub(1); // Convert to 0-based index
    if line_idx >= lines.len() {
        return None;
    }

    let start_line = line_idx.saturating_sub(1);
    let end_line = (line_idx + 2).min(lines.len());

    let mut result = String::new();
    result.push_str(&format!(
        "{:<60} │  {}:{}:{}",
        " ",
        tolkc::source_map::SourceLocation::normalize_path(&location.file),
        location.line + 1,
        location.column + 2
    ));

    for i in start_line..end_line {
        let line_num = i + 1;
        let line_content = lines[i];
        result.push_str(&format!(
            "\n{:>60}{}│  {:>3}: {}",
            "", " ", line_num, line_content
        ));

        if i == line_idx + 1 {
            let cursor_pos = location.column as usize + 1;
            result.push_str(&format!(
                "\n{:>60} │  {:>3}  {}{}",
                "",
                "",
                " ".repeat(cursor_pos),
                "^"
            ));
        }
    }

    result.push('\n');

    Some(result)
}

impl ArgValue {
    pub fn string(&self) -> String {
        match self {
            ArgValue::Control(c) => format!("{c}"),
            ArgValue::StackRegister(s) => format!("{s}"),
            ArgValue::Int(b) => format!("{b}"),
            _ => panic!("unhandled value: {self:?}"),
        }
    }
}

fn normalize_name(name: &str) -> String {
    if let Some(stripped) = name.strip_prefix('2') {
        format!("{stripped}2")
    } else {
        name.replace('#', "_")
    }
}

fn format_arg(arg: &ArgValue, depth: usize, opts: &FormatOptions) -> String {
    let indent = "    ".repeat(depth);
    match arg {
        ArgValue::Control(c) => format!("{c}"),
        ArgValue::StackRegister(s) => format!("{s}"),
        ArgValue::Int(b) => format!("{b}"),
        ArgValue::Cell(s) => {
            let slice = s.as_slice().unwrap();
            if slice.size_refs() == 0 {
                format!("x{{{}}}", slice.display_data().to_string())
            } else {
                format!("boc{{{}}}", Boc::encode_hex(s))
            }
        }
        ArgValue::Code {
            code,
            source,
            offset,
        } => {
            let mut builder = String::new();
            builder.push('{');
            if opts.show_hashes {
                builder.push_str(&format!(
                    " // {} offset {}",
                    source.repr_hash().to_string().to_uppercase(),
                    offset
                ));
            }
            builder.push('\n');
            for (i, instruction) in code.instructions.iter().enumerate() {
                let instr_offset = code.offsets.as_ref().and_then(|offs| offs.get(i).copied());
                builder.push_str(&instruction.print(depth + 1, opts, instr_offset));
                builder.push('\n');
            }

            if opts.show_offsets {
                builder.push_str("    │ ");
            }

            builder.push_str(&indent);
            builder.push('}');
            builder
        }
        ArgValue::CodeDictionary(dict) => {
            let mut builder = String::new();
            builder.push_str("[\n");
            for method in &dict.methods {
                if opts.show_offsets {
                    builder.push_str(OFFSET_PADDING);
                }

                builder.push_str(&indent);
                builder.push_str(&format!("    {} => ", method.id));
                builder.push_str("{");
                if opts.show_hashes {
                    builder.push_str(&format!(
                        " // {}",
                        method.source.repr_hash().to_string().to_uppercase()
                    ));
                }
                builder.push('\n');
                for (i, instruction) in method.instructions.iter().enumerate() {
                    let instr_offset = method
                        .offsets
                        .as_ref()
                        .and_then(|offs| offs.get(i).copied());
                    builder.push_str(&instruction.print(depth + 2, opts, instr_offset));
                    builder.push('\n');
                }

                if opts.show_offsets {
                    builder.push_str(OFFSET_PADDING);
                }

                builder.push_str("    ");
                builder.push_str(&indent);
                builder.push_str("}\n");
            }

            if opts.show_offsets {
                builder.push_str(OFFSET_PADDING);
            }

            builder.push_str(&indent);
            builder.push(']');
            builder
        }
        ArgValue::UInt(v) => format!("{v}"),
    }
}
