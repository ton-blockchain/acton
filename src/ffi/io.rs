use crate::context::Context;
use inquire::{Confirm, Select, Text};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use tvmffi::from_stack::FromStack;
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
    let result = format_args(ctx, fmt, args);
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
    let result = format_args(ctx, fmt, args);
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
    let result = format_args(ctx, fmt, args);
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
    let result = format_args(ctx, fmt, args);
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
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result);
    Ok(())
}

fn format_args(ctx: &mut Context, mut fmt: String, args: Vec<(String, TupleItem)>) -> String {
    for (type_name, arg) in args {
        // Special formatting for hexadecimal numbers
        if let Some(pos) = fmt.find("{:x}")
            && let TupleItem::Tuple(args) = &arg
            && args.len() == 1
            && let TupleItem::Int(typed_arg) = &args[0]
        {
            let formatted_arg = format!("{typed_arg:x}");
            fmt.replace_range(pos..pos + 4, formatted_arg.as_str());
            continue;
        }

        // Special formatting for TON amount
        if let Some(pos) = fmt.find("{:ton}")
            && let TupleItem::Tuple(args) = &arg
            && args.len() == 1
            && let TupleItem::Int(typed_arg) = &args[0]
        {
            let amount = typed_arg.to_f64().unwrap_or(0.0) / 1e9;
            let formatted_arg = format!("{amount} TON");
            fmt.replace_range(pos..pos + 6, formatted_arg.as_str());
            continue;
        }

        let typed_arg = arg.to_typed(&type_name);

        let formatter = crate::formatter::FormatterContext::from_context(ctx);
        let formatted = formatter.format(&typed_arg);

        if let Some(pos) = fmt.find("{}") {
            fmt.replace_range(pos..pos + 2, formatted.as_str());
        }
    }
    fmt
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

extension!(select in (Context) with (variants: TupleItem, message: String) using select_impl);
fn select_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    variants: TupleItem,
    message: String,
) -> anyhow::Result<()> {
    let TupleItem::Tuple(raw_variants) = variants else {
        stack.push_string("");
        return Ok(());
    };

    let variants = raw_variants
        .iter()
        .filter_map(|var| {
            let str = String::from_item((*var).clone());
            str.ok()
        })
        .collect::<Vec<_>>();

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
        1 => println,
        2 => eprintln,
        200 => format1,
        201 => format2,
        202 => format3,
        203 => format4,
        204 => format5,
        205 => prompt,
        206 => select,
        207 => confirm,
    });
}
