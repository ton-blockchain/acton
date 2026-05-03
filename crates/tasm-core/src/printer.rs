use crate::types::{ArgValue, Code, Instruction};
use std::fs;
use tolk_compiler::SourceMap;
use ton_source_map::SourceLocation;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice};

const MIN_OFFSET_WIDTH: usize = 4;
const MAX_RENDERED_SOURCE_RANGE_LINES: usize = 4;

#[derive(Clone, Default)]
pub struct FormatOptions {
    pub show_hashes: bool,
    pub show_offsets: bool,
    pub source_map: Option<Box<SourceMap>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceLocationKey {
    file: String,
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
}

impl From<&SourceLocation> for SourceLocationKey {
    fn from(value: &SourceLocation) -> Self {
        Self {
            file: value.file.clone(),
            line: value.line,
            column: value.column,
            end_line: value.end_line,
            end_column: value.end_column,
        }
    }
}

#[derive(Default)]
pub(crate) struct PrintState {
    last_source_location: Option<SourceLocationKey>,
}

impl Instruction {
    #[must_use]
    pub(crate) fn print(
        &self,
        depth: usize,
        opts: &FormatOptions,
        offset: Option<u16>,
        offset_width: usize,
        state: &mut PrintState,
    ) -> String {
        let indent = "    ".repeat(depth);
        let mut builder = String::new();

        if opts.show_offsets {
            push_offset_prefix(&mut builder, offset, offset_width);
        }

        if let Instruction::Ref(instr) = self {
            state.last_source_location = None;
            builder.push_str(&indent);
            builder.push_str("ref ");
            builder.push_str(&format_arg(&instr.code, depth, opts, offset_width));
            return builder.trim_end().to_string();
        }

        if let Instruction::ExoticCell(instr) = self {
            state.last_source_location = None;
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

        if let Instruction::Slice(instr) = self {
            state.last_source_location = None;
            builder.push_str(&indent);
            builder.push_str("embed ");
            builder.push_str(&format_cell(&instr.cell));
            return builder.trim_end().to_string();
        }

        let Instruction::Plain(instr) = self else {
            return builder;
        };

        builder.push_str(&indent);
        builder.push_str(&normalize_name(&instr.name));
        builder.push(' ');

        for (i, arg) in instr.args.iter().enumerate() {
            builder.push_str(&format_instruction_arg(
                &instr.name,
                i,
                arg,
                depth,
                opts,
                offset_width,
            ));
            if i < instr.args.len() - 1 {
                builder.push(' ');
            }
        }

        let result = builder.trim_end().to_string();

        if let Some(source_context) = next_source_context(
            opts,
            instr.source_cell.as_ref(),
            offset,
            offset_width,
            depth,
            state,
        ) {
            return format!("{source_context}\n{result}");
        }

        result
    }
}

fn get_source_location(
    source_map: &SourceMap,
    cell: Option<&Cell>,
    offset: u16,
) -> Option<SourceLocation> {
    let cell = cell?;
    let hash = cell.repr_hash().to_string().to_uppercase();
    source_map.find_source_loc(&hash, offset)
}

fn next_source_context(
    opts: &FormatOptions,
    cell: Option<&Cell>,
    offset: Option<u16>,
    offset_width: usize,
    depth: usize,
    state: &mut PrintState,
) -> Option<String> {
    let Some(source_map) = &opts.source_map else {
        state.last_source_location = None;
        return None;
    };
    let Some(offset) = offset else {
        state.last_source_location = None;
        return None;
    };
    let Some(location) = get_source_location(source_map, cell, offset) else {
        state.last_source_location = None;
        return None;
    };

    let key = SourceLocationKey::from(&location);
    let should_render = state.last_source_location.as_ref() != Some(&key);
    state.last_source_location = Some(key);

    should_render.then(|| {
        format_source_context(&location, &source_context_prefix(depth, opts, offset_width))
    })
}

fn source_context_prefix(depth: usize, opts: &FormatOptions, offset_width: usize) -> String {
    let mut prefix = String::new();
    if opts.show_offsets {
        prefix.push_str(&offset_padding(offset_width));
    }
    prefix.push_str(&"    ".repeat(depth));
    prefix.push_str("// ");
    prefix
}

fn format_source_context(location: &SourceLocation, prefix: &str) -> String {
    use std::fmt::Write as _;

    let mut result = String::new();
    write!(result, "{prefix}{}", location.format()).ok();

    if let Ok(content) = fs::read_to_string(&location.file) {
        let lines: Vec<&str> = content.lines().collect();
        if let Some((start_line_idx, end_line_idx)) = source_line_range(location, lines.len()) {
            let line_number_width = (end_line_idx + 1).to_string().len();

            for excerpt in source_excerpt_lines(start_line_idx, end_line_idx) {
                match excerpt {
                    SourceExcerptLine::Code(line_idx) => {
                        let line_content = lines[line_idx];
                        let pointer = format_range_pointer(
                            location,
                            line_idx,
                            line_content,
                            start_line_idx,
                            end_line_idx,
                        );
                        let line_no = line_idx + 1;

                        write!(
                            result,
                            "\n{prefix}{line_no:>line_number_width$} | {line_content}"
                        )
                        .ok();
                        write!(result, "\n{prefix}{:>line_number_width$} | {pointer}", "").ok();
                    }
                    SourceExcerptLine::Ellipsis { omitted_lines } => {
                        write!(
                            result,
                            "\n{prefix}{:>line_number_width$} | ... {omitted_lines} lines omitted ...",
                            ".."
                        )
                        .ok();
                    }
                }
            }
        }
    }

    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceExcerptLine {
    Code(usize),
    Ellipsis { omitted_lines: usize },
}

fn source_line_range(location: &SourceLocation, total_lines: usize) -> Option<(usize, usize)> {
    if total_lines == 0 {
        return None;
    }

    let start_line_idx = location.line.saturating_sub(1) as usize;
    if start_line_idx >= total_lines {
        return None;
    }

    let mut end_line_idx = if location.end_line > 0 {
        location.end_line.saturating_sub(1) as usize
    } else {
        start_line_idx
    };
    end_line_idx = end_line_idx.clamp(start_line_idx, total_lines - 1);

    Some((start_line_idx, end_line_idx))
}

fn source_excerpt_lines(start_line_idx: usize, end_line_idx: usize) -> Vec<SourceExcerptLine> {
    let line_count = end_line_idx - start_line_idx + 1;
    if line_count <= MAX_RENDERED_SOURCE_RANGE_LINES {
        return (start_line_idx..=end_line_idx)
            .map(SourceExcerptLine::Code)
            .collect();
    }

    vec![
        SourceExcerptLine::Code(start_line_idx),
        SourceExcerptLine::Ellipsis {
            omitted_lines: line_count - 2,
        },
        SourceExcerptLine::Code(end_line_idx),
    ]
}

fn format_range_pointer(
    location: &SourceLocation,
    line_idx: usize,
    line_content: &str,
    start_line_idx: usize,
    end_line_idx: usize,
) -> String {
    let line_len = line_content.len();
    let start_col = if line_idx == start_line_idx {
        normalize_column(location.column, line_len)
    } else {
        0
    };
    let end_col = if line_idx == end_line_idx {
        normalize_column(location.end_column, line_len).max(start_col)
    } else {
        line_len
    };
    let start_col =
        trim_leading_whitespace_within_range(line_content, start_col, end_col).unwrap_or(start_col);
    let underline_len = end_col.saturating_sub(start_col).max(1);

    format!("{}{}", " ".repeat(start_col), "^".repeat(underline_len))
}

pub(crate) fn offset_width_for(code: &Code) -> usize {
    offset_width_from_slice(code.offsets.as_deref())
}

pub(crate) fn offset_padding(offset_width: usize) -> String {
    format!("{}│ ", " ".repeat(offset_width))
}

fn push_offset_prefix(builder: &mut String, offset: Option<u16>, offset_width: usize) {
    use std::fmt::Write as _;

    if let Some(off) = offset {
        write!(builder, "{off:<offset_width$}│ ").ok();
    } else {
        builder.push_str(&offset_padding(offset_width));
    }
}

fn offset_width_from_slice(offsets: Option<&[u16]>) -> usize {
    offsets
        .and_then(|offsets| offsets.iter().max().copied())
        .map_or(MIN_OFFSET_WIDTH, |offset| {
            offset.to_string().len().max(MIN_OFFSET_WIDTH)
        })
}

fn normalize_column(column: i64, line_len: usize) -> usize {
    column.saturating_sub(1).clamp(0, line_len as i64) as usize
}

fn trim_leading_whitespace_within_range(
    line_content: &str,
    start_col: usize,
    end_col: usize,
) -> Option<usize> {
    let safe_end = end_col.min(line_content.len());
    if start_col >= line_content.len() {
        return None;
    }

    if start_col < safe_end
        && let Some(pos) = line_content[start_col..safe_end]
            .char_indices()
            .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(start_col + offset))
    {
        return Some(pos);
    }

    let safe_end = safe_end.min(line_content.len());
    let is_leading_whitespace_range = line_content[..safe_end].chars().all(char::is_whitespace);
    if !is_leading_whitespace_range {
        return None;
    }

    line_content[safe_end..]
        .char_indices()
        .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(safe_end + offset))
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

fn format_arg(arg: &ArgValue, depth: usize, opts: &FormatOptions, offset_width: usize) -> String {
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

            let nested_offset_width = offset_width_for(code);
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
            let mut nested_state = PrintState::default();
            for (i, instruction) in code.instructions.iter().enumerate() {
                let instr_offset = code.offsets.as_ref().and_then(|offs| offs.get(i).copied());
                builder.push_str(&instruction.print(
                    depth + 1,
                    opts,
                    instr_offset,
                    nested_offset_width,
                    &mut nested_state,
                ));
                builder.push('\n');
            }

            if opts.show_offsets {
                builder.push_str(&offset_padding(nested_offset_width));
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
                let method_offset_width = offset_width_from_slice(method.offsets.as_deref());
                if opts.show_offsets {
                    builder.push_str(&offset_padding(method_offset_width));
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
                let mut method_state = PrintState::default();
                for (i, instruction) in method.instructions.iter().enumerate() {
                    let instr_offset = method
                        .offsets
                        .as_ref()
                        .and_then(|offs| offs.get(i).copied());
                    builder.push_str(&instruction.print(
                        depth + 2,
                        opts,
                        instr_offset,
                        method_offset_width,
                        &mut method_state,
                    ));
                    builder.push('\n');
                }

                if opts.show_offsets {
                    builder.push_str(&offset_padding(method_offset_width));
                }

                builder.push_str("    ");
                builder.push_str(&indent);
                builder.push_str("}\n");
            }

            if opts.show_offsets {
                builder.push_str(&offset_padding(offset_width));
            }

            builder.push_str(&indent);
            builder.push(']');
            builder
        }
        ArgValue::UInt(v) => format!("{v}"),
    }
}

fn format_instruction_arg(
    instruction_name: &str,
    index: usize,
    arg: &ArgValue,
    depth: usize,
    opts: &FormatOptions,
    offset_width: usize,
) -> String {
    if instruction_name == "BLKPUSH"
        && index == 1
        && let ArgValue::StackRegister(register) = arg
    {
        return register.idx.to_string();
    }

    format_arg(arg, depth, opts, offset_width)
}

fn format_cell(s: &Cell) -> String {
    let slice = s.as_slice_allow_exotic();
    format_slice(&slice)
}

fn format_slice(slice: &CellSlice<'_>) -> String {
    if slice.size_refs() == 0 {
        format!("x{{{:X}}}", slice.display_data())
    } else {
        let mut builder = CellBuilder::new();
        builder.store_slice(slice).ok();
        let Ok(cell) = builder.build() else {
            return String::new();
        };
        format!("boc{{{}}}", Boc::encode_hex(cell))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_context_prefix_uses_five_digit_offset_width() {
        let code = Code {
            instructions: Vec::new(),
            offsets: Some(vec![0, 10_000]),
        };
        let opts = FormatOptions {
            show_offsets: true,
            ..FormatOptions::default()
        };

        let offset_width = offset_width_for(&code);

        assert_eq!(offset_width, 5);
        assert_eq!(source_context_prefix(0, &opts, offset_width), "     │ // ");
    }
}
