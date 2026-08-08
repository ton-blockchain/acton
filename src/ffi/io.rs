use crate::commands::common::{error_fmt, format_nanograms};
use crate::context::{BuildCache, Context, to_cell};
use crate::ffi::emulation::{compilation_result_for_code, normalize_address_input};
use crate::formatter::FormatterContext;
use acton_config::color::OwoColorize;
use acton_debug::render_tuple_item_as_tolk_type;
use anyhow::{Context as AnyhowContext, anyhow, bail};
use inquire::validator::{ErrorMessage, Validation};
use inquire::{Confirm, Select, Text};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write, stdin};
use tolk_compiler::SourceMap;
use tolk_compiler::abi::Ty;
use tolk_compiler::types_kernel::{TyIdx, render_ty};
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvm_ffi::from_stack::FromStack;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{DynCell, LevelMask};
use tycho_types::models::{StdAddr, StdAddrFormat};

extension!(println in (Context) with (arg6: TupleItem, type6: BigInt, arg5: TupleItem, type5: BigInt, arg4: TupleItem, type4: BigInt, arg3: TupleItem, type3: BigInt, arg2: TupleItem, type2: BigInt, arg1: TupleItem, type1: BigInt) using println_impl);
#[allow(clippy::too_many_arguments)]
fn println_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    arg6: TupleItem,
    type6: BigInt,
    arg5: TupleItem,
    type5: BigInt,
    arg4: TupleItem,
    type4: BigInt,
    arg3: TupleItem,
    type3: BigInt,
    arg2: TupleItem,
    type2: BigInt,
    arg1: TupleItem,
    type1: BigInt,
) -> anyhow::Result<()> {
    let args = collect_non_void_args(
        ctx,
        [
            (type1, arg1),
            (type2, arg2),
            (type3, arg3),
            (type4, arg4),
            (type5, arg5),
            (type6, arg6),
        ],
    )?;
    let formatter = FormatterContext::from_context(ctx);
    let source_map = ctx.env.source_map.as_ref();
    let (mut formatted, tail) = if let Some(arg) = args.first()
        && is_top_level_string_ty_idx(source_map, arg.ty_idx)
        && let Ok(fmt) = String::from_item(arg.arg.clone().unwrap_single())
        && let Ok((rendered, consumed)) = format_args(ctx, &formatter, &fmt, &args[1..], true)
    {
        (rendered, &args[1 + consumed..])
    } else {
        (String::new(), args.as_slice())
    };

    for arg in tail {
        if !formatted.is_empty() {
            formatted.push(' ');
        }
        formatted.push_str(&format_reflected_arg(ctx, &formatter, arg, true)?);
    }

    if ctx.io.capture_output {
        ctx.io.stdout_buffer.push_str(&formatted);
        ctx.io.stdout_buffer.push('\n');
    }
    if ctx.io.live_output || !ctx.io.capture_output {
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{formatted}")?;
        stdout.flush()?;
    }
    Ok(())
}

extension!(eprintln in (Context) with (s: String) using eprintln_impl);
fn eprintln_impl(ctx: &mut Context, _stack: &mut Tuple, s: String) -> anyhow::Result<()> {
    if ctx.io.capture_output {
        ctx.io.stderr_buffer.push_str(&s);
        ctx.io.stderr_buffer.push('\n');
    }
    if ctx.io.live_output || !ctx.io.capture_output {
        let mut stderr = io::stderr().lock();
        writeln!(stderr, "{s}")?;
        stderr.flush()?;
    }
    Ok(())
}

extension!(format in (Context) with (arg5: TupleItem, type5: BigInt, arg4: TupleItem, type4: BigInt, arg3: TupleItem, type3: BigInt, arg2: TupleItem, type2: BigInt, arg1: TupleItem, type1: BigInt, fmt: String) using format_impl);
#[allow(clippy::too_many_arguments)]
fn format_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg5: TupleItem,
    type5: BigInt,
    arg4: TupleItem,
    type4: BigInt,
    arg3: TupleItem,
    type3: BigInt,
    arg2: TupleItem,
    type2: BigInt,
    arg1: TupleItem,
    type1: BigInt,
    fmt: String,
) -> anyhow::Result<()> {
    let args = collect_non_void_args(
        ctx,
        [
            (type1, arg1),
            (type2, arg2),
            (type3, arg3),
            (type4, arg4),
            (type5, arg5),
        ],
    )?;
    let formatter = FormatterContext::from_context(ctx);
    let (result, _) = format_args(ctx, &formatter, &fmt, &args, false)?;
    stack.push_string(&result);
    Ok(())
}

#[derive(Clone)]
struct ReflectedArg {
    ty_idx: TyIdx,
    arg: TupleItem,
}

#[derive(Copy, Clone)]
enum PlaceholderKind {
    Plain,
    Hex,
    HexPrefixed,
    Binary,
    BinaryPrefixed,
    Gram,
    CellTree,
}

#[derive(Copy, Clone)]
enum FormatAlign {
    Left,
    Right,
    Center,
}

#[derive(Clone)]
struct PlaceholderSpec {
    kind: PlaceholderKind,
    repr: String,
    fill: char,
    align: Option<FormatAlign>,
    width: Option<usize>,
    sign_aware_zero_pad: bool,
}

#[derive(Clone)]
enum FormatToken {
    Literal(String),
    Placeholder(PlaceholderSpec),
}

const fn parse_align(ch: char) -> Option<FormatAlign> {
    match ch {
        '<' => Some(FormatAlign::Left),
        '>' => Some(FormatAlign::Right),
        '^' => Some(FormatAlign::Center),
        _ => None,
    }
}

fn parse_placeholder_spec(
    content: &str,
    placeholder: &str,
    byte_pos: usize,
) -> anyhow::Result<PlaceholderSpec> {
    if content.is_empty() {
        return Ok(PlaceholderSpec {
            kind: PlaceholderKind::Plain,
            repr: placeholder.to_owned(),
            fill: ' ',
            align: None,
            width: None,
            sign_aware_zero_pad: false,
        });
    }

    let Some(mut spec) = content.strip_prefix(':') else {
        bail!(
            "Invalid format string at byte {byte_pos}: unsupported placeholder {placeholder} (supported: {{}}, {{:x}}, {{:X}}, {{:b}}, {{:B}}, {{:gram}}, {{:grams}}, {{:ton}}, {{:cell-tree}})"
        )
    };
    if spec.is_empty() {
        return Err(unknown_format_modifier_error("", placeholder, byte_pos));
    }

    let mut fill = ' ';
    let mut align = None;
    let mut sign_aware_zero_pad = false;

    let mut chars = spec.chars();
    if let Some(first) = chars.next() {
        let after_first = chars.as_str();
        if let Some(second) = chars.next()
            && let Some(parsed_align) = parse_align(second)
        {
            fill = first;
            align = Some(parsed_align);
            spec = chars.as_str();
        } else if let Some(parsed_align) = parse_align(first) {
            align = Some(parsed_align);
            spec = after_first;
        }
    }

    if align.is_none()
        && spec.len() > 1
        && spec.starts_with('0')
        && spec.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
    {
        fill = '0';
        align = Some(FormatAlign::Right);
        sign_aware_zero_pad = true;
        spec = &spec[1..];
    }

    let width_digits_len = spec
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let width = if width_digits_len == 0 {
        None
    } else {
        let digits = &spec[..width_digits_len];
        spec = &spec[width_digits_len..];
        Some(digits.parse::<usize>().with_context(|| {
            format!("Invalid format string at byte {byte_pos}: width '{digits}' is too large")
        })?)
    };

    if align.is_some() && width.is_none() && spec.is_empty() {
        bail!("Invalid format string at byte {byte_pos}: missing width in {placeholder}");
    }

    let modifier = spec.strip_prefix(':').unwrap_or(spec);
    let kind = match modifier {
        "" => PlaceholderKind::Plain,
        "x" => PlaceholderKind::Hex,
        "X" => PlaceholderKind::HexPrefixed,
        "b" => PlaceholderKind::Binary,
        "B" => PlaceholderKind::BinaryPrefixed,
        "gram" | "grams" | "ton" => PlaceholderKind::Gram,
        "cell-tree" => PlaceholderKind::CellTree,
        _ => {
            return Err(unknown_format_modifier_error(
                modifier,
                placeholder,
                byte_pos,
            ));
        }
    };

    Ok(PlaceholderSpec {
        kind,
        repr: placeholder.to_owned(),
        fill,
        align,
        width,
        sign_aware_zero_pad,
    })
}

fn unknown_format_modifier_error(
    modifier: &str,
    placeholder: &str,
    byte_pos: usize,
) -> anyhow::Error {
    anyhow!(
        "Invalid format string at byte {byte_pos}: unknown format modifier '{modifier}' in {placeholder} (supported: :x, :X, :b, :B, :gram, :grams, :ton, :cell-tree)"
    )
}

fn parse_format(fmt: &str) -> anyhow::Result<Vec<FormatToken>> {
    let mut tokens: Vec<FormatToken> = Vec::new();
    let mut literal = String::new();
    let mut i = 0;

    while i < fmt.len() {
        let rem = &fmt[i..];

        if rem.starts_with("{{") {
            literal.push('{');
            i += 2;
            continue;
        }
        if rem.starts_with("}}") {
            literal.push('}');
            i += 2;
            continue;
        }

        if let Some(stripped) = rem.strip_prefix('{') {
            let Some(close_rel) = stripped.find('}') else {
                bail!("Invalid format string at byte {i}: unclosed '{{' placeholder");
            };

            let close_pos = i + 1 + close_rel;
            let content = &fmt[i + 1..close_pos];
            let placeholder = &fmt[i..=close_pos];
            let spec = parse_placeholder_spec(content, placeholder, i)?;

            if !literal.is_empty() {
                tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
            }
            tokens.push(FormatToken::Placeholder(spec));

            i = close_pos + 1;
            continue;
        }

        if rem.starts_with('}') {
            bail!("Invalid format string at byte {i}: unmatched '}}'");
        }

        let ch = rem
            .chars()
            .next()
            .expect("format parser should always have a next char here");
        literal.push(ch);
        i += ch.len_utf8();
    }

    if !literal.is_empty() {
        tokens.push(FormatToken::Literal(literal));
    }

    Ok(tokens)
}

fn format_default(
    ctx: &Context<'_>,
    formatter: &FormatterContext<'_>,
    ty_idx: TyIdx,
    arg: TupleItem,
    colorize: bool,
) -> anyhow::Result<String> {
    format_reflected_arg(ctx, formatter, &ReflectedArg { ty_idx, arg }, colorize)
}

fn format_single_arg(
    ctx: &Context<'_>,
    formatter: &FormatterContext<'_>,
    spec: &PlaceholderSpec,
    ty_idx: TyIdx,
    arg: TupleItem,
    colorize: bool,
) -> anyhow::Result<String> {
    let int_value = single_int_value(&arg);
    let is_int = int_value.is_some();
    let formatted = match spec.kind {
        PlaceholderKind::Hex => match int_value {
            Some(value) => format!("{value:x}"),
            None => format_default(ctx, formatter, ty_idx, arg, colorize)?,
        },
        PlaceholderKind::HexPrefixed => match int_value {
            Some(value) => format_prefixed_int(format!("{value:x}"), "0x"),
            None => format_default(ctx, formatter, ty_idx, arg, colorize)?,
        },
        PlaceholderKind::Binary => match int_value {
            Some(value) => format!("{value:b}"),
            None => format_default(ctx, formatter, ty_idx, arg, colorize)?,
        },
        PlaceholderKind::BinaryPrefixed => match int_value {
            Some(value) => format_prefixed_int(format!("{value:b}"), "0b"),
            None => format_default(ctx, formatter, ty_idx, arg, colorize)?,
        },
        PlaceholderKind::Gram => match int_value {
            Some(value) => format_nanograms(value),
            None => format_default(ctx, formatter, ty_idx, arg, colorize)?,
        },
        PlaceholderKind::CellTree => {
            if let TupleItem::Tuple(items) = &arg
                && items.len() == 1
                && let TupleItem::Cell(cell) | TupleItem::Slice(cell) | TupleItem::Builder(cell) =
                    &items[0]
            {
                format_cell_tree(cell.as_ref(), colorize)
            } else {
                format_default(ctx, formatter, ty_idx, arg, colorize)?
            }
        }
        PlaceholderKind::Plain => format_default(ctx, formatter, ty_idx, arg, colorize)?,
    };

    Ok(apply_width(formatted, spec, is_int))
}

fn single_int_value(arg: &TupleItem) -> Option<&BigInt> {
    if let TupleItem::Tuple(items) = arg
        && items.len() == 1
        && let TupleItem::Int(value) = &items[0]
    {
        Some(value)
    } else {
        None
    }
}

fn format_prefixed_int(formatted: String, prefix: &str) -> String {
    if let Some(stripped) = formatted.strip_prefix('-') {
        format!("-{prefix}{stripped}")
    } else {
        format!("{prefix}{formatted}")
    }
}

fn apply_width(mut formatted: String, spec: &PlaceholderSpec, is_int: bool) -> String {
    let Some(width) = spec.width else {
        return formatted;
    };
    let len = FormatterContext::strip_ansi_text(&formatted)
        .chars()
        .count();
    if len >= width {
        return formatted;
    }

    let default_align = if is_int
        || matches!(
            spec.kind,
            PlaceholderKind::Hex
                | PlaceholderKind::HexPrefixed
                | PlaceholderKind::Binary
                | PlaceholderKind::BinaryPrefixed
                | PlaceholderKind::Gram
        ) {
        FormatAlign::Right
    } else {
        FormatAlign::Left
    };
    let align = spec.align.unwrap_or(default_align);
    let padding_len = width - len;

    match align {
        FormatAlign::Left => {
            formatted.extend(std::iter::repeat_n(spec.fill, padding_len));
            formatted
        }
        FormatAlign::Right => {
            let padding = std::iter::repeat_n(spec.fill, padding_len).collect::<String>();
            if spec.sign_aware_zero_pad && spec.fill == '0' {
                let ansi_prefix_len = leading_ansi_escape_prefix_len(&formatted);
                let (ansi_prefix, rest) = formatted.split_at(ansi_prefix_len);
                let (sign, unsigned) = if let Some(unsigned) = rest.strip_prefix('-') {
                    ("-", unsigned)
                } else if let Some(unsigned) = rest.strip_prefix('+') {
                    ("+", unsigned)
                } else {
                    ("", rest)
                };
                let (prefix, digits) = match spec.kind {
                    PlaceholderKind::HexPrefixed => unsigned
                        .strip_prefix("0x")
                        .map_or(("", unsigned), |digits| ("0x", digits)),
                    PlaceholderKind::BinaryPrefixed => unsigned
                        .strip_prefix("0b")
                        .map_or(("", unsigned), |digits| ("0b", digits)),
                    _ => ("", unsigned),
                };
                return format!("{ansi_prefix}{sign}{prefix}{padding}{digits}");
            }
            format!("{padding}{formatted}")
        }
        FormatAlign::Center => {
            let left_len = padding_len / 2;
            let right_len = padding_len - left_len;
            let left = std::iter::repeat_n(spec.fill, left_len).collect::<String>();
            let right = std::iter::repeat_n(spec.fill, right_len).collect::<String>();
            format!("{left}{formatted}{right}")
        }
    }
}

fn leading_ansi_escape_prefix_len(value: &str) -> usize {
    let mut prefix_len = 0;
    while let Some(rest) = value[prefix_len..].strip_prefix('\x1b') {
        let Some(end) = rest.find('m') else {
            break;
        };
        prefix_len += 1 + end + 1;
    }
    prefix_len
}

fn format_cell_tree(root: &DynCell, colorize: bool) -> String {
    let mut out = String::new();
    let mut stack = vec![(0, root)];

    while let Some((level, cell)) = stack.pop() {
        write_cell_tree_root(&mut out, cell, level, colorize);

        for index in (0..cell.reference_count()).rev() {
            if let Some(child) = cell.reference(index) {
                stack.push((level + 1, child));
            }
        }
    }

    out.trim_end().to_owned()
}

fn write_cell_tree_root(out: &mut String, cell: &DynCell, level: usize, colorize: bool) {
    let indent = " ".repeat(level * 2);
    let data = hex::encode(cell.data());
    let descriptor = cell.descriptor();
    let cell_type = format!("{:?}", descriptor.cell_type());
    let level_mask = format!("{:?}", descriptor.level_mask());
    let depth = cell.depth(LevelMask::MAX_LEVEL).to_string();
    let hash = cell.repr_hash().to_string();

    out.push_str(&indent);
    out.push_str(&color_cell_type(&cell_type, colorize));
    out.push_str(": ");
    out.push_str(&color_cell_data(&data, colorize));
    out.push('\n');

    out.push_str(&indent);
    out.push_str(&color_cell_label("bits", colorize));
    out.push_str(": ");
    out.push_str(&color_cell_value(
        &format!("{:>4}", cell.bit_len()),
        colorize,
    ));
    out.push_str(&color_cell_label(", refs: ", colorize));
    out.push_str(&color_cell_value(
        &descriptor.reference_count().to_string(),
        colorize,
    ));
    out.push_str(&color_cell_label(", l: ", colorize));
    out.push_str(&color_cell_value(&level_mask, colorize));
    out.push_str(&color_cell_label(", depth: ", colorize));
    out.push_str(&color_cell_value(&depth, colorize));
    out.push_str(&color_cell_label(", hash: ", colorize));
    out.push_str(&color_cell_hash(&hash, colorize));
    out.push('\n');
}

fn color_cell_type(value: &str, colorize: bool) -> String {
    if colorize {
        value.magenta().to_string()
    } else {
        value.to_owned()
    }
}

fn color_cell_data(value: &str, colorize: bool) -> String {
    if colorize {
        value.cyan().to_string()
    } else {
        value.to_owned()
    }
}

fn color_cell_label(value: &str, colorize: bool) -> String {
    if colorize {
        value.dimmed().to_string()
    } else {
        value.to_owned()
    }
}

fn color_cell_value(value: &str, colorize: bool) -> String {
    if colorize {
        value.yellow().to_string()
    } else {
        value.to_owned()
    }
}

fn color_cell_hash(value: &str, colorize: bool) -> String {
    if colorize {
        value.dimmed().to_string()
    } else {
        value.to_owned()
    }
}

fn collect_non_void_args<const N: usize>(
    ctx: &Context<'_>,
    args: [(BigInt, TupleItem); N],
) -> anyhow::Result<Vec<ReflectedArg>> {
    let mut collected = Vec::with_capacity(N);
    let source_map = ctx.env.source_map.as_ref();
    for (type_idx, arg) in args {
        let ty_idx = type_idx
            .to_usize()
            .ok_or_else(|| anyhow!("ty_idx=`{type_idx}` does not fit into usize"))?;
        let Some(ty) = source_map.ty_by_idx(ty_idx) else {
            continue;
        };
        if matches!(ty, Ty::Void) {
            break;
        }
        collected.push(ReflectedArg { ty_idx, arg });
    }
    Ok(collected)
}

fn build_cache_for_send_result_list(
    ctx: &Context<'_>,
    base_cache: &BuildCache,
    item: &TupleItem,
) -> BuildCache {
    let mut build_cache = base_cache.clone();
    let TupleItem::Tuple(items) = item else {
        return build_cache;
    };

    let mut seen = HashSet::new();

    for tx in FormatterContext::send_result_transactions(items) {
        let addr = StdAddr::new(0, tx.account);
        let Some(code) =
            FormatterContext::account_code(ctx.chain.world_state.get_accounts(), &addr)
        else {
            continue;
        };

        if !seen.insert(*code.repr_hash()) {
            continue;
        }
        if build_cache.result_for_code(&Some(code.clone())).is_some() {
            continue;
        }
        // A contract created via `fromAddress()` does not call `build(...)`, so
        // its ABI is absent from the main BuildCache. Resolve it into this
        // formatter-local overlay so rendering has names without mutating the
        // script/test environment.
        if let Some((path, result)) = compilation_result_for_code(ctx, Some(&code), true) {
            build_cache.built.insert(path, result);
        }
    }

    build_cache
}

fn format_args(
    ctx: &Context<'_>,
    formatter: &FormatterContext<'_>,
    fmt: &str,
    args: &[ReflectedArg],
    colorize: bool,
) -> anyhow::Result<(String, usize)> {
    let tokens = parse_format(fmt)?;
    let mut out = String::with_capacity(fmt.len());
    let mut args_iter = args.iter();
    let mut consumed = 0;

    for token in tokens {
        match token {
            FormatToken::Literal(text) => out.push_str(&text),
            FormatToken::Placeholder(spec) => {
                if let Some(arg) = args_iter.next() {
                    let formatted = format_single_arg(
                        ctx,
                        formatter,
                        &spec,
                        arg.ty_idx,
                        arg.arg.clone(),
                        colorize,
                    )?;
                    out.push_str(&formatted);
                    consumed += 1;
                } else {
                    out.push_str(&spec.repr);
                }
            }
        }
    }

    Ok((out, consumed))
}

fn format_reflected_arg(
    ctx: &Context<'_>,
    formatter: &FormatterContext<'_>,
    arg: &ReflectedArg,
    colorize: bool,
) -> anyhow::Result<String> {
    let item = arg.arg.clone().unwrap_single();
    let source_map = ctx.env.source_map.as_ref();

    if is_top_level_string_ty_idx(source_map, arg.ty_idx)
        && let Ok(value) = String::from_item(item.clone())
    {
        return Ok(value);
    }

    if is_send_result_list_type(source_map, arg.ty_idx) {
        return Ok(format_send_result_list(ctx, formatter, &item));
    }

    Ok(render_with_source_map(
        ctx.env.source_map.as_ref(),
        formatter,
        &item,
        arg.ty_idx,
        colorize,
    ))
}

fn render_with_source_map(
    symbols: &SourceMap,
    formatter: &FormatterContext<'_>,
    item: &TupleItem,
    ty_idx: TyIdx,
    colorize: bool,
) -> String {
    let options = if colorize {
        formatter.pretty_render_options_with_cli_color()
    } else {
        formatter.pretty_render_options()
    };
    render_tuple_item_as_tolk_type(symbols, item, ty_idx).to_pretty_string(options)
}

fn is_top_level_string_ty_idx(source_map: &SourceMap, ty_idx: TyIdx) -> bool {
    match source_map.ty_by_idx(ty_idx) {
        Some(Ty::String) => true,
        Some(Ty::Nullable { inner_ty_idx, .. }) => {
            is_top_level_string_ty_idx(source_map, *inner_ty_idx)
        }
        _ => false,
    }
}

fn is_send_result_list_type(source_map: &SourceMap, ty_idx: TyIdx) -> bool {
    match source_map.ty_by_idx(ty_idx) {
        Some(Ty::AliasRef { alias_name, .. }) if alias_name == "SendResultList" => true,
        Some(Ty::AliasRef {
            alias_name,
            type_args_ty_idx: Some(type_args),
        }) if alias_name == "BigArray" => type_args
            .first()
            .is_some_and(|&item_ty_idx| is_send_result_type(source_map, item_ty_idx)),
        Some(Ty::Nullable { inner_ty_idx, .. }) => {
            is_send_result_list_type(source_map, *inner_ty_idx)
        }
        _ => render_ty(source_map, ty_idx) == "SendResultList",
    }
}

fn is_send_result_type(source_map: &SourceMap, ty_idx: TyIdx) -> bool {
    matches!(
        source_map.ty_by_idx(ty_idx),
        Some(Ty::StructRef {
            struct_name,
            ..
        }) if struct_name == "SendResult"
    )
}

fn format_send_result_list(
    ctx: &Context<'_>,
    formatter: &FormatterContext<'_>,
    item: &TupleItem,
) -> String {
    let build_cache = build_cache_for_send_result_list(ctx, formatter.build_cache.as_ref(), item);
    let formatter = FormatterContext {
        build_cache: Cow::Owned(build_cache),
        ..formatter.clone()
    };
    match item {
        TupleItem::Tuple(items) => formatter.format_transaction_list(items),
        TupleItem::Null => "null".to_owned(),
        _ => "not a TVM tuple".to_owned(),
    }
}

extension!(prompt in (Context) with (default: String, placeholder: String, message: String) using prompt_impl);
fn prompt_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    default: String,
    placeholder: String,
    message: String,
) -> anyhow::Result<()> {
    let text = if stdin().is_terminal() {
        let mut text = Text::new(&message).with_placeholder(&placeholder);
        if !default.is_empty() {
            text = text.with_default(&default);
        }
        text.prompt().unwrap_or_else(|_| default.clone())
    } else {
        default
    };

    stack.push_string(&text);
    Ok(())
}

fn parse_prompt_int(input: &str) -> anyhow::Result<BigInt> {
    input
        .trim()
        .parse::<BigInt>()
        .with_context(|| format!("Failed to parse integer from '{input}'"))
}

extension!(prompt_int in (Context) with (default: String, placeholder: String, message: String) using prompt_int_impl);
fn prompt_int_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    default: String,
    placeholder: String,
    message: String,
) -> anyhow::Result<()> {
    let input = if stdin().is_terminal() {
        let mut text = Text::new(&message).with_placeholder(&placeholder);
        if !default.is_empty() {
            text = text.with_default(&default);
        }

        text.with_validator(|input: &str| {
            if parse_prompt_int(input).is_ok() {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Enter a valid integer".to_owned(),
                )))
            }
        })
        .prompt()
        .unwrap_or_else(|_| default.clone())
    } else {
        default
    };

    stack.push(TupleItem::Int(parse_prompt_int(&input)?));
    Ok(())
}

fn parse_prompt_address(input: &str) -> anyhow::Result<StdAddr> {
    let (addr, _) = StdAddr::from_str_ext(normalize_address_input(input), StdAddrFormat::any())
        .with_context(|| format!("Cannot parse address: {input}"))?;
    Ok(addr)
}

extension!(prompt_address in (Context) with (default: String, placeholder: String, message: String) using prompt_address_impl);
fn prompt_address_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    default: String,
    placeholder: String,
    message: String,
) -> anyhow::Result<()> {
    let input = if stdin().is_terminal() {
        let mut text = Text::new(&message).with_placeholder(&placeholder);
        if !default.is_empty() {
            text = text.with_default(&default);
        }

        text.with_validator(|input: &str| {
            if parse_prompt_address(input).is_ok() {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "Enter a valid TON address".to_owned(),
                )))
            }
        })
        .prompt()
        .unwrap_or_else(|_| default.clone())
    } else {
        default
    };

    let addr = parse_prompt_address(&input)?;
    stack.push(TupleItem::Slice(to_cell(&addr)));
    Ok(())
}

extension!(select in (Context) with (default_index: BigInt, variants: Vec<String>, message: String) using select_impl);
fn select_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    default_index: BigInt,
    variants: Vec<String>,
    message: String,
) -> anyhow::Result<()> {
    let result = if stdin().is_terminal() {
        let cursor = default_index
            .to_usize()
            .unwrap_or(0)
            .min(variants.len().saturating_sub(1));
        Select::new(&message, variants)
            .with_starting_cursor(cursor)
            .prompt()
            .unwrap_or_default()
    } else {
        default_select_value(&default_index, &variants)
    };

    stack.push_string(&result);
    Ok(())
}

fn default_select_value(default_index: &BigInt, variants: &[String]) -> String {
    let Some(cursor) = default_index
        .to_usize()
        .map(|index| index.min(variants.len().saturating_sub(1)))
    else {
        return variants.first().cloned().unwrap_or_default();
    };

    variants.get(cursor).cloned().unwrap_or_default()
}

extension!(confirm in (Context) with (help_message: String, default: BigInt, message: String) using confirm_impl);
fn confirm_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    help_message: String,
    default: BigInt,
    message: String,
) -> anyhow::Result<()> {
    let default = default != BigInt::ZERO;
    let res = if stdin().is_terminal() {
        Confirm::new(&message)
            .with_default(default)
            .with_help_message(&help_message)
            .prompt()
            .unwrap_or(default)
    } else {
        default
    };

    stack.push_bool(res);
    Ok(())
}

extension!(prompt_wallet in (Context) with (message: String) using prompt_wallet_impl);
fn prompt_wallet_impl(ctx: &mut Context, stack: &mut Tuple, message: String) -> anyhow::Result<()> {
    // In emulate mode `scripts.wallet(name)` accepts any name, so there is nothing real to choose
    // from. Return a stable placeholder so scripts that call `promptWallet` keep working when
    // run without `--net` (e.g. plain `acton script`).
    if !ctx.is_broadcasting {
        stack.push_string("emulated-wallet");
        return Ok(());
    }

    if ctx.env.tonconnect.is_some() {
        stack.push_string("tonconnect");
        return Ok(());
    }

    let wallet_names: Vec<String> = ctx.env.open_wallets.keys().cloned().collect();

    if wallet_names.is_empty() {
        ctx.asserts.fail(error_fmt::no_wallets_found());
        stack.push(TupleItem::Null);
        return Ok(());
    }

    if wallet_names.len() == 1 {
        stack.push_string(&wallet_names[0]);
        return Ok(());
    }

    if !stdin().is_terminal() {
        ctx.asserts.fail(
            "Cannot prompt for wallet selection in a non-interactive environment. \
             Please specify the wallet explicitly."
                .to_string(),
        );
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let Ok(result) = Select::new(&message, wallet_names)
        .with_starting_cursor(0)
        .prompt()
    else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    stack.push_string(&result);
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        1 => println : 12,
        2 => eprintln : 1,
        200 => format : 11,
        205 => prompt : 3,
        206 => select : 3,
        207 => confirm : 3,
        208 => prompt_wallet : 1,
        209 => prompt_int : 3,
        210 => prompt_address : 3,
    });
}
