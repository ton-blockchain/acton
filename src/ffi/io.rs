use crate::context::Context;
use emulator::traits::BaseExecutor;
use emulator::{extension, pop_args, register_ext_methods};
use inquire::{Confirm, Select, Text};
use num_bigint::BigInt;
use tvmffi::from_stack::FromStack;
use tvmffi::stack::{Tuple, TupleItem};

extension!(println in (Context) with (s: TupleItem, type_name: String) using println_impl);
fn println_impl(ctx: &mut Context, _stack: &mut Tuple, arg: TupleItem, type_name: String) {
    let typed_arg = arg.unwrap_single().to_typed(&type_name);

    let formatter = crate::formatter::FormatterContext::from_context(ctx);
    let formatted = strip_quotes(formatter.format(&typed_arg));

    if ctx.capture_output {
        ctx.stdout_buffer.push_str(&formatted);
        ctx.stdout_buffer.push_str("\n");
    } else {
        println!("{}", formatted);
    }
}

extension!(eprintln in (Context) with (s: String) using eprintln_impl);
fn eprintln_impl(ctx: &mut Context, _stack: &mut Tuple, s: String) {
    if ctx.capture_output {
        ctx.stderr_buffer.push_str(&s);
        ctx.stderr_buffer.push_str("\n");
    } else {
        eprintln!("{}", s);
    }
}

extension!(format1 in (Context) with (arg1: TupleItem, type1: String, fmt: String) using format1_impl);
fn format1_impl(ctx: &mut Context, stack: &mut Tuple, arg1: TupleItem, type1: String, fmt: String) {
    let args = vec![(type1, arg1)];
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result)
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
) {
    let args = vec![(type1, arg1), (type2, arg2)];
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result)
}

extension!(format3 in (Context) with (arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format3_impl);
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
) {
    let args = vec![(type1, arg1), (type2, arg2), (type3, arg3)];
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result)
}

extension!(format4 in (Context) with (arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format4_impl);
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
) {
    let args = vec![(type1, arg1), (type2, arg2), (type3, arg3), (type4, arg4)];
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result)
}

extension!(format5 in (Context) with (arg5: TupleItem, type5: String, arg4: TupleItem, type4: String, arg3: TupleItem, type3: String, arg2: TupleItem, type2: String, arg1: TupleItem, type1: String, fmt: String) using format5_impl);
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
) {
    let args = vec![
        (type1, arg1),
        (type2, arg2),
        (type3, arg3),
        (type4, arg4),
        (type5, arg5),
    ];
    let result = format_args(ctx, fmt, args);
    stack.push_string(&result)
}

fn format_args(ctx: &mut Context, mut fmt: String, args: Vec<(String, TupleItem)>) -> String {
    for (type_name, arg) in args {
        // Special formatting for hexadecimal numbers
        if let Some(pos) = fmt.find("{:x}")
            && let TupleItem::Tuple(args) = &arg
            && args.len() == 1
            && let TupleItem::Int(typed_arg) = &args[0]
        {
            let formatted_arg = format!("{:x}", typed_arg);
            fmt.replace_range(pos..pos + 4, formatted_arg.as_str());
            continue;
        }

        let typed_arg = arg.to_typed(&type_name);

        let formatter = crate::formatter::FormatterContext::from_context(ctx);
        let formatted = strip_quotes(formatter.format(&typed_arg));

        if let Some(pos) = fmt.find("{}") {
            fmt.replace_range(pos..pos + 2, formatted.as_str());
        }
    }
    fmt
}

fn strip_quotes(formatted: String) -> String {
    if formatted.starts_with("\"") && formatted.ends_with("\"") {
        formatted[1..formatted.len() - 1].to_string()
    } else {
        formatted
    }
}

extension!(prompt in (Context) with (placeholder: String, message: String) using prompt_impl);
fn prompt_impl(_ctx: &mut Context, stack: &mut Tuple, placeholder: String, message: String) {
    let text = Text::new(&message)
        .with_placeholder(&placeholder)
        .prompt()
        .unwrap_or("".to_string());

    stack.push_string(&text);
}

extension!(select in (Context) with (variants: TupleItem, message: String) using select_impl);
fn select_impl(_ctx: &mut Context, stack: &mut Tuple, variants: TupleItem, message: String) {
    let TupleItem::Tuple(raw_variants) = variants else {
        stack.push_string("");
        return;
    };

    let variants = raw_variants
        .iter()
        .flat_map(|var| {
            let str = String::from_item((*var).clone());
            str.ok()
        })
        .collect::<Vec<_>>();

    let result = Select::new(&message, variants)
        .with_starting_cursor(0)
        .prompt()
        .unwrap_or("".to_string());

    stack.push_string(&result);
}

extension!(confirm in (Context) with (help_message: String, default: BigInt, message: String) using confirm_impl);
fn confirm_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    help_message: String,
    default: BigInt,
    message: String,
) {
    let res = Confirm::new(&message)
        .with_default(default != BigInt::from(0))
        .with_help_message(&help_message)
        .prompt()
        .unwrap_or(false);

    stack.push_bool(res);
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
