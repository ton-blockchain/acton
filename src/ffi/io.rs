use crate::context::Context;
use anyhow::bail;
use inquire::{Confirm, Select, Text};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::stack::{Tuple, TupleItem};

extension!(println in (Context) with (s: TupleItem, type_name: String) using println_impl);
fn println_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    arg: TupleItem,
    type_name: String,
) -> anyhow::Result<()> {
    let typed_arg = arg.unwrap_single().to_typed(&type_name);

    let formatter = crate::formatter::FormatterContext::from_context(ctx);
    let formatted = formatter.format_with_color(&typed_arg);

    if ctx.io.capture_output {
        ctx.io.stdout_buffer.push_str(&formatted);
        ctx.io.stdout_buffer.push('\n');
    } else {
        println!("{formatted}");
    }
    Ok(())
}

extension!(eprintln in (Context) with (s: String) using eprintln_impl);
fn eprintln_impl(ctx: &mut Context, _stack: &mut Tuple, s: String) -> anyhow::Result<()> {
    if ctx.io.capture_output {
        ctx.io.stderr_buffer.push_str(&s);
        ctx.io.stderr_buffer.push('\n');
    } else {
        eprintln!("{s}");
    }
    Ok(())
}

extension!(format1 in (Context) with (arg1: TupleItem, type1: String, fmt: String) using format1_impl);
fn format1_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg1: TupleItem,
    type1: String,
    fmt: String,
) -> anyhow::Result<()> {
    let args = vec![(type1, arg1)];
    let result = format_args(ctx, fmt, args)?;
    stack.push_string(&result);
    Ok(())
}

extension!(format2 in (Context) with (arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format2_impl);
fn format2_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg2: TupleItem,
    type2: String,
    arg1: TupleItem,
    type1: String,
    fmt: String,
) -> anyhow::Result<()> {
    let args = vec![(type1, arg1), (type2, arg2)];
    let result = format_args(ctx, fmt, args)?;
    stack.push_string(&result);
    Ok(())
}

extension!(format3 in (Context) with (arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format3_impl);
#[allow(clippy::too_many_arguments)]
fn format3_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg3: TupleItem,
    type3: String,
    arg2: TupleItem,
    type2: String,
    arg1: TupleItem,
    type1: String,
    fmt: String,
) -> anyhow::Result<()> {
    let args = vec![(type1, arg1), (type2, arg2), (type3, arg3)];
    let result = format_args(ctx, fmt, args)?;
    stack.push_string(&result);
    Ok(())
}

extension!(format4 in (Context) with (arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format4_impl);
#[allow(clippy::too_many_arguments)]
fn format4_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg4: TupleItem,
    type4: String,
    arg3: TupleItem,
    type3: String,
    arg2: TupleItem,
    type2: String,
    arg1: TupleItem,
    type1: String,
    fmt: String,
) -> anyhow::Result<()> {
    let args = vec![(type1, arg1), (type2, arg2), (type3, arg3), (type4, arg4)];
    let result = format_args(ctx, fmt, args)?;
    stack.push_string(&result);
    Ok(())
}

extension!(format5 in (Context) with (arg5: TupleItem, type5: String, arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format5_impl);
#[allow(clippy::too_many_arguments)]
fn format5_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    arg5: TupleItem,
    type5: String,
    arg4: TupleItem,
    type4: String,
    arg3: TupleItem,
    type3: String,
    arg2: TupleItem,
    type2: String,
    arg1: TupleItem,
    type1: String,
    fmt: String,
) -> anyhow::Result<()> {
    let args = vec![
        (type1, arg1),
        (type2, arg2),
        (type3, arg3),
        (type4, arg4),
        (type5, arg5),
    ];
    let result = format_args(ctx, fmt, args)?;
    stack.push_string(&result);
    Ok(())
}

#[derive(Copy, Clone)]
enum PlaceholderKind {
    Plain,
    Hex,
    Ton,
}

#[derive(Clone)]
enum FormatToken {
    Literal(String),
    Placeholder(PlaceholderKind),
}

const fn placeholder_repr(kind: PlaceholderKind) -> &'static str {
    match kind {
        PlaceholderKind::Plain => "{}",
        PlaceholderKind::Hex => "{:x}",
        PlaceholderKind::Ton => "{:ton}",
    }
}

fn parse_placeholder_kind(
    content: &str,
    placeholder: &str,
    byte_pos: usize,
) -> anyhow::Result<PlaceholderKind> {
    if content.is_empty() {
        return Ok(PlaceholderKind::Plain);
    }
    if let Some(modifier) = content.strip_prefix(':') {
        return match modifier {
            "x" => Ok(PlaceholderKind::Hex),
            "ton" => Ok(PlaceholderKind::Ton),
            _ => bail!(
                "Invalid format string at byte {}: unknown format modifier '{}' in {} (supported: :x, :ton)",
                byte_pos,
                modifier,
                placeholder
            ),
        };
    }
    bail!(
        "Invalid format string at byte {}: unsupported placeholder {} (supported: {{}}, {{:x}}, {{:ton}})",
        byte_pos,
        placeholder
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
                bail!(
                    "Invalid format string at byte {}: unclosed '{{' placeholder",
                    i
                );
            };

            let close_pos = i + 1 + close_rel;
            let content = &fmt[i + 1..close_pos];
            let placeholder = &fmt[i..=close_pos];
            let kind = parse_placeholder_kind(content, placeholder, i)?;

            if !literal.is_empty() {
                tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
            }
            tokens.push(FormatToken::Placeholder(kind));

            i = close_pos + 1;
            continue;
        }

        if rem.starts_with('}') {
            bail!("Invalid format string at byte {}: unmatched '}}'", i);
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

fn format_default(ctx: &mut Context, type_name: &str, arg: TupleItem) -> String {
    let typed_arg = arg.to_typed(type_name);
    let formatter = crate::formatter::FormatterContext::from_context(ctx);
    formatter.format(&typed_arg)
}

fn format_single_arg(
    ctx: &mut Context,
    kind: PlaceholderKind,
    type_name: &str,
    arg: TupleItem,
) -> String {
    match kind {
        PlaceholderKind::Hex => {
            if let TupleItem::Tuple(items) = &arg
                && items.len() == 1
                && let TupleItem::Int(value) = &items[0]
            {
                return format!("{value:x}");
            }
            format_default(ctx, type_name, arg)
        }
        PlaceholderKind::Ton => {
            if let TupleItem::Tuple(items) = &arg
                && items.len() == 1
                && let TupleItem::Int(value) = &items[0]
            {
                let amount = value.to_f64().unwrap_or(0.0) / 1e9;
                return format!("{amount} TON");
            }
            format_default(ctx, type_name, arg)
        }
        PlaceholderKind::Plain => format_default(ctx, type_name, arg),
    }
}

fn format_args(
    ctx: &mut Context,
    fmt: String,
    args: Vec<(String, TupleItem)>,
) -> anyhow::Result<String> {
    let tokens = parse_format(&fmt)?;
    let mut out = String::with_capacity(fmt.len());
    let mut args_iter = args.into_iter();

    for token in tokens {
        match token {
            FormatToken::Literal(text) => out.push_str(&text),
            FormatToken::Placeholder(kind) => {
                if let Some((type_name, arg)) = args_iter.next() {
                    let formatted = format_single_arg(ctx, kind, &type_name, arg);
                    out.push_str(&formatted);
                } else {
                    out.push_str(placeholder_repr(kind));
                }
            }
        }
    }

    Ok(out)
}

extension!(prompt in (Context) with (placeholder: String, message: String) using prompt_impl);
fn prompt_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    placeholder: String,
    message: String,
) -> anyhow::Result<()> {
    let text = Text::new(&message)
        .with_placeholder(&placeholder)
        .prompt()
        .unwrap_or_default();

    stack.push_string(&text);
    Ok(())
}

extension!(select in (Context) with (variants: Vec<String>, message: String) using select_impl);
fn select_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    variants: Vec<String>,
    message: String,
) -> anyhow::Result<()> {
    let result = Select::new(&message, variants)
        .with_starting_cursor(0)
        .prompt()
        .unwrap_or_default();

    stack.push_string(&result);
    Ok(())
}

extension!(confirm in (Context) with (help_message: String, default: BigInt, message: String) using confirm_impl);
fn confirm_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    help_message: String,
    default: BigInt,
    message: String,
) -> anyhow::Result<()> {
    let res = Confirm::new(&message)
        .with_default(default != BigInt::ZERO)
        .with_help_message(&help_message)
        .prompt()
        .unwrap_or(false);

    stack.push_bool(res);
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        1 => println : 2,
        2 => eprintln : 1,
        200 => format1 : 3,
        201 => format2 : 5,
        202 => format3 : 7,
        203 => format4 : 9,
        204 => format5 : 11,
        205 => prompt : 2,
        206 => select : 2,
        207 => confirm : 3,
    });
}
