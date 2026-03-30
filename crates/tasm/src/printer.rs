use crate::types::{ArgValue, Instruction};
use std::fs;
use tolkc::TolkSourceMap;
use ton_source_map::SourceLocation;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice};

const OFFSET_PADDING: &str = "    │ ";

#[derive(Clone, Default)]
pub struct FormatOptions {
    pub show_hashes: bool,
    pub show_offsets: bool,
    pub source_map: Option<Box<TolkSourceMap>>,
}

impl Instruction {
    #[must_use]
    pub fn print(&self, depth: usize, opts: &FormatOptions, offset: Option<u16>) -> String {
        use std::fmt::Write as _;

        let indent = "    ".repeat(depth);
        let mut builder = String::new();

        if opts.show_offsets {
            if let Some(off) = offset {
                write!(builder, "{off:<4}│ ").ok();
            } else {
                builder.push_str("     │");
            }
        }

        if let Instruction::Ref(instr) = self {
            builder.push_str(&indent);
            builder.push_str("ref ");
            builder.push_str(&format_arg(&instr.code, depth, opts));
            return builder.trim_end().to_string();
        }

        if let Instruction::ExoticCell(instr) = self {
            builder.push_str(&indent);
            builder.push_str("exotic ");

            let mut slice = instr.cell.as_slice_allow_exotic();
            let typ = slice.load_u8().unwrap_or(0);
            if typ == 2 {
                builder.push_str("library ");
                builder.push_str(&format_slice(&slice));
            } else {
                builder.push_str(&format_cell(&instr.cell));
            }
            return builder.trim_end().to_string();
        }

        let Instruction::Plain(instr) = self else {
            return builder;
        };

        builder.push_str(&indent);
        builder.push_str(&normalize_name(&instr.name));
        builder.push(' ');

        for (i, arg) in instr.args.iter().enumerate() {
            builder.push_str(&format_arg(arg, depth, opts));
            if i < instr.args.len() - 1 {
                builder.push(' ');
            }
        }

        let result = builder.trim_end().to_string();
        let padding = 100_usize.saturating_sub(builder.len());

        if let Some(source_map) = &opts.source_map
            && let Some(off) = offset
            && let Some(location) = get_source_location(source_map, instr.source_cell.as_ref(), off)
        {
            if let Some(source_context) = format_source_context(&location) {
                let before = format!("    └{}┐\n", "─".repeat(56));
                let after = format!("    ┌{}┘", "─".repeat(56));
                return format!(
                    "{}{:>padding$}\n{before}{}{after}",
                    result, "", source_context
                );
            }
        }

        result
    }
}

fn get_source_location(
    source_map: &TolkSourceMap,
    cell: Option<&Cell>,
    offset: u16,
) -> Option<SourceLocation> {
    let cell = cell?;
    let hash = cell.repr_hash().to_string().to_uppercase();
    source_map.find_source_loc(&hash, offset)
}

fn format_source_context(location: &SourceLocation) -> Option<String> {
    use std::fmt::Write as _;

    let content = fs::read_to_string(&location.file).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    let line_idx = (location.line as usize).saturating_sub(1); // Convert to 0-based index
    if line_idx >= lines.len() {
        return None;
    }

    let start_line = line_idx.saturating_sub(1);
    let end_line = (line_idx + 2).min(lines.len());

    let mut result = String::new();
    write!(
        result,
        "{:<60} │  {}:{}:{}",
        " ",
        SourceLocation::normalize_path(&location.file),
        location.line,
        location.column
    )
    .ok();

    for (i, line_content) in lines.iter().enumerate().take(end_line).skip(start_line) {
        let line_num = i + 1;
        write!(result, "\n{:>60} │  {:>3}: {}", "", line_num, line_content).ok();

        if i == line_idx {
            let cursor_pos = location.column.saturating_sub(1) as usize;
            write!(
                result,
                "\n{:>60} │  {:>3}  {}^",
                "",
                "",
                " ".repeat(cursor_pos)
            )
            .ok();
        }
    }

    result.push('\n');

    Some(result)
}

impl ArgValue {
    #[must_use]
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
        ArgValue::Cell(s) => format_cell(s),
        ArgValue::Code {
            code,
            source,
            offset,
        } => {
            use std::fmt::Write as _;

            let mut builder = String::new();
            builder.push('{');
            if opts.show_hashes {
                write!(
                    builder,
                    " // {} offset {}",
                    source.repr_hash().to_string().to_uppercase(),
                    offset
                )
                .ok();
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
            use std::fmt::Write as _;

            let mut builder = String::new();
            builder.push_str("[\n");
            for method in &dict.methods {
                if opts.show_offsets {
                    builder.push_str(OFFSET_PADDING);
                }

                builder.push_str(&indent);
                write!(builder, "    {} => ", method.id).ok();
                builder.push('{');
                if opts.show_hashes {
                    write!(
                        builder,
                        " // {}",
                        method.source.repr_hash().to_string().to_uppercase()
                    )
                    .ok();
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

fn format_cell(s: &Cell) -> String {
    let slice = s.as_slice_allow_exotic();
    format_slice(&slice)
}

fn format_slice(slice: &CellSlice<'_>) -> String {
    if slice.size_refs() == 0 {
        format!("x{{{}}}", slice.display_data())
    } else {
        let mut builder = CellBuilder::new();
        builder.store_slice(slice).ok();
        let Ok(cell) = builder.build() else {
            return String::new();
        };
        format!("boc{{{}}}", Boc::encode_hex(cell))
    }
}
