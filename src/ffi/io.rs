use crate::commands::common::error_fmt;
use crate::context::Context;
use crate::formatter::FormatterContext;
use anyhow::bail;
use inquire::{Confirm, Select, Text};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::io::{IsTerminal, stdin};
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::from_stack::FromStack;
use tvmffi::stack::{Tuple, TupleItem};

extension!(println in (Context) with (arg6: TupleItem, type6: String, arg5: TupleItem, type5: String, arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String) using println_impl);
#[allow(clippy::too_many_arguments)]
fn println_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    arg6: TupleItem,
    type6: String,
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
) -> anyhow::Result<()> {
    let args = collect_non_void_args([
        (type1, arg1),
        (type2, arg2),
        (type3, arg3),
        (type4, arg4),
        (type5, arg5),
        (type6, arg6),
    ]);
    let formatter = FormatterContext::from_context(ctx);
    let (mut formatted, tail) = if let Some((type_name, arg)) = args.first()
        && type_name == "string"
        && let Ok(fmt) = String::from_item(arg.clone().unwrap_single())
        && let Ok((rendered, consumed)) = format_args(&formatter, &fmt, &args[1..])
    {
        (rendered, &args[1 + consumed..])
    } else {
        (String::new(), args.as_slice())
    };

    for (type_name, arg) in tail {
        if !formatted.is_empty() {
            formatted.push(' ');
        }
        let typed_arg = arg.unwrap_single().to_typed(type_name);
        formatted.push_str(&formatter.format_with_color(&typed_arg));
    }

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

extension!(format in (Context) with (arg5: TupleItem, type5: String, arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format_impl);
#[allow(clippy::too_many_arguments)]
fn format_impl(
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
    let args = collect_non_void_args([
        (type1, arg1),
        (type2, arg2),
        (type3, arg3),
        (type4, arg4),
        (type5, arg5),
    ]);
    let formatter = FormatterContext::from_context(ctx);
    let (result, _) = format_args(&formatter, &fmt, &args)?;
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
                "Invalid format string at byte {byte_pos}: unknown format modifier '{modifier}' in {placeholder} (supported: :x, :ton)"
            ),
        };
    }
    bail!(
        "Invalid format string at byte {byte_pos}: unsupported placeholder {placeholder} (supported: {{}}, {{:x}}, {{:ton}})"
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
            let kind = parse_placeholder_kind(content, placeholder, i)?;

            if !literal.is_empty() {
                tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
            }
            tokens.push(FormatToken::Placeholder(kind));

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

fn format_default(formatter: &FormatterContext<'_>, type_name: &str, arg: TupleItem) -> String {
    let typed_arg = arg.to_typed(type_name);
    formatter.format(&typed_arg)
}

fn format_single_arg(
    formatter: &FormatterContext<'_>,
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
            format_default(formatter, type_name, arg)
        }
        PlaceholderKind::Ton => {
            if let TupleItem::Tuple(items) = &arg
                && items.len() == 1
                && let TupleItem::Int(value) = &items[0]
            {
                let amount = value.to_f64().unwrap_or(0.0) / 1e9;
                return format!("{amount} TON");
            }
            format_default(formatter, type_name, arg)
        }
        PlaceholderKind::Plain => format_default(formatter, type_name, arg),
    }
}

fn collect_non_void_args<const N: usize>(
    args: [(String, TupleItem); N],
) -> Vec<(String, TupleItem)> {
    let mut collected = Vec::with_capacity(N);
    for (type_name, arg) in args {
        if type_name == "void" {
            break;
        }
        collected.push((type_name, arg));
    }
    collected
}

fn format_args(
    formatter: &FormatterContext<'_>,
    fmt: &str,
    args: &[(String, TupleItem)],
) -> anyhow::Result<(String, usize)> {
    let tokens = parse_format(fmt)?;
    let mut out = String::with_capacity(fmt.len());
    let mut args_iter = args.iter().cloned();
    let mut consumed = 0;

    for token in tokens {
        match token {
            FormatToken::Literal(text) => out.push_str(&text),
            FormatToken::Placeholder(kind) => {
                if let Some((type_name, arg)) = args_iter.next() {
                    let formatted = format_single_arg(formatter, kind, &type_name, arg);
                    out.push_str(&formatted);
                    consumed += 1;
                } else {
                    out.push_str(placeholder_repr(kind));
                }
            }
        }
    }

    Ok((out, consumed))
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
        String::new()
    };

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
    let res = if stdin().is_terminal() {
        Confirm::new(&message)
            .with_default(default != BigInt::ZERO)
            .with_help_message(&help_message)
            .prompt()
            .unwrap_or(false)
    } else {
        false
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

    let result = match Select::new(&message, wallet_names)
        .with_starting_cursor(0)
        .prompt()
    {
        Ok(name) => name,
        Err(_) => {
            stack.push(TupleItem::Null);
            return Ok(());
        }
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
    });
}
