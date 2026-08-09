use crate::context::Context;
use crate::ffi::emulation::run_tolk_continuation;
use acton_config::color::OwoColorize;
use anyhow::Context as AnyhowContext;
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use tasm_core::decompile::Disassembler;
use tasm_core::printer::FormatOptions;
use tasm_core::types::{ArgValue, Code, Instruction, Method};
use ton_emulator::{extension, register_ext_methods};
use ton_executor::get::DEFAULT_GET_METHOD_GAS_LIMIT;
use ton_executor::{BaseExecutor, ExecutorVerbosity};
use ton_retrace::trace::{Trace, TraceStep};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{CellBuilder, CellContext, CellSlice, Load, Store};
use tycho_types::dict::Dict;
use tycho_types::error::Error;
use tycho_types::models::StdAddr;

extension!(measure in (Context) with (location: String, addr: StdAddr, args: TupleItem, baseline: TupleItem, function: TupleItem) using measure_impl);
fn measure_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    location: String,
    addr: StdAddr,
    args: TupleItem,
    baseline: TupleItem,
    function: TupleItem,
) -> anyhow::Result<()> {
    let baseline_addr = addr.clone();
    let baseline_args = args.clone();
    let asm_code = disassemble_bench_function(ctx, &function)?;
    let result = run_tolk_continuation(
        ctx,
        addr,
        args,
        function,
        ExecutorVerbosity::FullLocationGas,
    )?;
    ensure_continuation_success(&result, &location, "measured callback")?;

    let baseline_result = run_tolk_continuation(
        ctx,
        baseline_addr,
        baseline_args,
        baseline,
        ExecutorVerbosity::FullLocationGas,
    )?;
    ensure_continuation_success(&baseline_result, &location, "measurement baseline")?;

    let (gas_used, instructions) = build_instruction_dict(&result.vm_log, &baseline_result.vm_log)?;
    let cell = Boc::decode_base64(result.stack.as_ref()).context("Failed to decode stack BoC")?;
    let tuple = Tuple::deserialize(&cell).context("Failed to deserialize tuple")?;

    let mut response = Tuple::empty();
    response.push(TupleItem::Tuple(tuple));
    response.push(TupleItem::Int(BigInt::from(gas_used)));
    response.push(match instructions.into_root() {
        Some(root) => TupleItem::Cell(root),
        None => TupleItem::Null,
    });
    response.push_string(&asm_code);
    stack.push(TupleItem::Tuple(response));

    Ok(())
}

extension!(format_profile in (Context) with (instructions: TupleItem, gas_used: BigInt, name: String) using format_profile_impl);
fn format_profile_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    instructions: TupleItem,
    gas_used: BigInt,
    name: String,
) -> anyhow::Result<()> {
    let gas_used = parse_stack_u64(gas_used, "gasUsed")?;
    let instructions = decode_instruction_map(instructions)?;
    stack.push_string(&render_profile_table(&name, gas_used, &instructions));
    Ok(())
}

extension!(format_diff in (Context) with (current_instructions: TupleItem, current_gas_used: BigInt, current_name: String, baseline_instructions: TupleItem, baseline_gas_used: BigInt, baseline_name: String) using format_diff_impl);
#[allow(clippy::too_many_arguments)]
fn format_diff_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    current_instructions: TupleItem,
    current_gas_used: BigInt,
    current_name: String,
    baseline_instructions: TupleItem,
    baseline_gas_used: BigInt,
    baseline_name: String,
) -> anyhow::Result<()> {
    let baseline_gas_used = parse_stack_u64(baseline_gas_used, "baseline gasUsed")?;
    let current_gas_used = parse_stack_u64(current_gas_used, "current gasUsed")?;
    let baseline_instructions = decode_instruction_map(baseline_instructions)?;
    let current_instructions = decode_instruction_map(current_instructions)?;

    stack.push_string(&render_diff_table(
        &baseline_name,
        baseline_gas_used,
        &baseline_instructions,
        &current_name,
        current_gas_used,
        &current_instructions,
    ));
    Ok(())
}

fn ensure_continuation_success(
    result: &ton_executor::get::GetMethodResultSuccess,
    location: &str,
    context: &str,
) -> anyhow::Result<()> {
    if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
        let location = if location.is_empty() {
            "unknown location"
        } else {
            location
        };
        anyhow::bail!(
            "bench.measure at {location}: {context} exited with exit code {}",
            result.vm_exit_code
        );
    }
    Ok(())
}

fn parse_stack_u64(value: BigInt, label: &str) -> anyhow::Result<u64> {
    value
        .to_u64()
        .with_context(|| format!("bench {label} does not fit into uint64: {value}"))
}

fn disassemble_bench_function(ctx: &mut Context, function: &TupleItem) -> anyhow::Result<String> {
    let TupleItem::Cont(cont) = function else {
        anyhow::bail!("Expected continuation, got {function:?}");
    };

    let continuation_code = Disassembler::new()
        .decompile_cell(&cont.code)
        .context("Failed to disassemble bench continuation")?;

    let Some(method_id) = called_method_id(&continuation_code) else {
        return Ok(continuation_code.print(&FormatOptions::default()));
    };

    // Tolk function values usually arrive as a tiny continuation thunk:
    //
    //     CALLDICT 2
    //
    // Showing that to users is not useful for benchmarking; it only says how the
    // callback is invoked. Resolve the method id against the contract/test code
    // dictionary and return the actual callback body instead.
    let Some(contract_code) = &ctx.env.test_code else {
        return Ok(continuation_code.print(&FormatOptions::default()));
    };

    let contract_code = match Disassembler::new()
        .decompile_cell(contract_code)
        .context("Failed to disassemble bench contract code")
    {
        Ok(code) => code,
        Err(err) => return Ok(format!("{err:#}")),
    };
    let Some(method) =
        find_code_dictionary_method_in_instructions(&contract_code.instructions, method_id)
    else {
        return Ok(continuation_code.print(&FormatOptions::default()));
    };

    Ok(Code {
        instructions: method.instructions.clone(),
        offsets: method.offsets.clone(),
    }
    .print(&FormatOptions::default()))
}

fn called_method_id(code: &Code) -> Option<u64> {
    let [Instruction::Plain(instruction)] = code.instructions.as_slice() else {
        return None;
    };

    if instruction.name != "CALLDICT" {
        return None;
    }

    let [ArgValue::UInt(id)] = instruction.args.as_slice() else {
        return None;
    };

    id.to_u64()
}

fn find_code_dictionary_method_in_instructions(
    instructions: &[Instruction],
    method_id: u64,
) -> Option<&Method> {
    for instruction in instructions {
        match instruction {
            Instruction::Plain(instruction) => {
                for arg in &instruction.args {
                    if let Some(method) = find_code_dictionary_method_in_arg(arg, method_id) {
                        return Some(method);
                    }
                }
            }
            Instruction::Ref(instruction) => {
                if let Some(method) =
                    find_code_dictionary_method_in_arg(&instruction.code, method_id)
                {
                    return Some(method);
                }
            }
            Instruction::ExoticCell(_) | Instruction::Slice(_) => {}
        }
    }

    None
}

fn find_code_dictionary_method_in_arg(arg: &ArgValue, method_id: u64) -> Option<&Method> {
    match arg {
        ArgValue::Code { code, .. } => {
            find_code_dictionary_method_in_instructions(&code.instructions, method_id)
        }
        ArgValue::CodeDictionary(dict) => dict
            .methods
            .iter()
            .find(|method| method.id == method_id)
            .or_else(|| {
                dict.methods.iter().find_map(|method| {
                    find_code_dictionary_method_in_instructions(&method.instructions, method_id)
                })
            }),
        ArgValue::Int(_)
        | ArgValue::UInt(_)
        | ArgValue::Control(_)
        | ArgValue::StackRegister(_)
        | ArgValue::Cell(_) => None,
    }
}

fn build_instruction_dict(
    vm_log: &str,
    baseline_vm_log: &str,
) -> anyhow::Result<(u64, Dict<u32, BenchInstructionInfoCell>)> {
    let mut samples_by_name = collect_instruction_samples(vm_log);
    subtract_baseline_samples(
        &mut samples_by_name,
        collect_instruction_samples(baseline_vm_log),
    );

    let mut gas_used = 0u64;
    let mut dict = Dict::<u32, BenchInstructionInfoCell>::new();
    for (id, (name, samples)) in samples_by_name.into_iter().enumerate() {
        let stats = InstructionStats::from_samples(&samples);
        let Some(stats) = stats else {
            continue;
        };
        let id = u32::try_from(id).context("Instruction profile has more than u32::MAX entries")?;
        gas_used = gas_used.saturating_add(stats.total_gas);
        dict.set(
            id,
            BenchInstructionInfoCell {
                id,
                name,
                count: stats.count,
                total_gas: stats.total_gas,
                min_gas: stats.min_gas,
                max_gas: stats.max_gas,
            },
        )?;
    }
    Ok((gas_used, dict))
}

fn collect_instruction_samples(vm_log: &str) -> BTreeMap<String, Vec<u64>> {
    let trace = Trace::new(vm_log, usize::try_from(DEFAULT_GET_METHOD_GAS_LIMIT).ok());
    let mut samples_by_name = BTreeMap::<String, Vec<u64>>::new();

    for step in trace.steps {
        let TraceStep::Execute { instr, gas, .. } = step else {
            continue;
        };
        if instr.is_empty() {
            continue;
        }
        samples_by_name
            .entry(bench_instruction_name(&instr))
            .or_default()
            .push(gas as u64);
    }

    samples_by_name
}

fn subtract_baseline_samples(
    samples_by_name: &mut BTreeMap<String, Vec<u64>>,
    baseline_samples_by_name: BTreeMap<String, Vec<u64>>,
) {
    for (name, baseline_samples) in baseline_samples_by_name {
        let Some(samples) = samples_by_name.get_mut(&name) else {
            continue;
        };

        for baseline_gas in baseline_samples {
            // Gas may differ between the baseline thunk and the measured call
            // path, but one matching mnemonic still belongs to call overhead.
            let position = samples
                .iter()
                .position(|gas| *gas == baseline_gas)
                .unwrap_or(0);
            if position < samples.len() {
                samples.remove(position);
            }
        }
    }

    samples_by_name.retain(|_, samples| !samples.is_empty());
}

impl InstructionStats {
    fn from_samples(samples: &[u64]) -> Option<Self> {
        let count = u32::try_from(samples.len()).ok()?;
        let min_gas = samples.iter().copied().min()?;
        let max_gas = samples.iter().copied().max()?;
        let total_gas = samples
            .iter()
            .fold(0u64, |total, gas| total.saturating_add(*gas));

        Some(Self {
            count,
            total_gas,
            min_gas,
            max_gas,
        })
    }
}

fn decode_instruction_map(
    instructions: TupleItem,
) -> anyhow::Result<BTreeMap<String, BenchInstructionInfoCell>> {
    let mut dict = match instructions {
        TupleItem::Null => Dict::<u32, BenchInstructionInfoCell>::new(),
        TupleItem::Cell(root) | TupleItem::Slice(root) => {
            Dict::<u32, BenchInstructionInfoCell>::from_raw(Some(root))
        }
        other => anyhow::bail!("Expected bench instructions dict, got {other:?}"),
    };
    let mut rows = BTreeMap::new();

    while let Some((_key, parts)) = dict.remove_min_raw(false)? {
        let mut slice = CellSlice::apply(&parts)?;
        let info = BenchInstructionInfoCell::load_from(&mut slice)?;
        rows.insert(info.name.clone(), info);
    }

    Ok(rows)
}

fn render_profile_table(
    name: &str,
    gas_used: u64,
    instructions: &BTreeMap<String, BenchInstructionInfoCell>,
) -> String {
    let mut output = format!(
        "Bench profile: {}\nTotal gas: {}\n\n",
        format_bench_name(name),
        format_gas_value(gas_used),
    );

    if instructions.is_empty() {
        output.push_str("No instructions captured.\n");
        return output;
    }

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Instruction", "Count", "Total", "Avg", "Min", "Max"]);

    let mut rows = instructions.values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .total_gas
            .cmp(&left.total_gas)
            .then_with(|| left.name.cmp(&right.name))
    });

    for row in rows {
        table.add_row(vec![
            TableCell::new(&row.name)
                .set_alignment(CellAlignment::Left)
                .fg(Color::DarkCyan),
            right_cell(row.count).fg(Color::Grey),
            right_cell(row.total_gas).fg(Color::DarkGreen),
            right_cell(row.avg_gas()).fg(Color::DarkGreen),
            right_cell(row.min_gas).fg(Color::DarkMagenta),
            right_cell(row.max_gas).fg(Color::DarkYellow),
        ]);
    }

    push_table(&mut output, &table);
    output
}

fn render_diff_table(
    baseline_name: &str,
    baseline_gas_used: u64,
    baseline_instructions: &BTreeMap<String, BenchInstructionInfoCell>,
    current_name: &str,
    current_gas_used: u64,
    current_instructions: &BTreeMap<String, BenchInstructionInfoCell>,
) -> String {
    let mut output = format!(
        "Bench diff: {} -> {}\nBaseline gas for {}: {}\nCurrent gas for {}: {}\nGas diff: {} ({})\n\n",
        format_bench_name(baseline_name),
        format_bench_name(current_name),
        format_bench_name(baseline_name),
        format_gas_value(baseline_gas_used),
        format_bench_name(current_name),
        format_gas_value(current_gas_used),
        format_colored_signed_diff_u64(current_gas_used, baseline_gas_used),
        format_colored_percent_change(current_gas_used, baseline_gas_used),
    );

    let mut names = baseline_instructions.keys().collect::<Vec<_>>();
    for name in current_instructions.keys() {
        if !baseline_instructions.contains_key(name) {
            names.push(name);
        }
    }

    if names.is_empty() {
        output.push_str("No instructions captured.\n");
        return output;
    }

    names.sort_by(|left, right| {
        let left_diff = instruction_total_diff_abs(
            baseline_instructions.get(*left),
            current_instructions.get(*left),
        );
        let right_diff = instruction_total_diff_abs(
            baseline_instructions.get(*right),
            current_instructions.get(*right),
        );
        right_diff
            .cmp(&left_diff)
            .then_with(|| left.as_str().cmp(right.as_str()))
    });

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Instruction",
            "Baseline",
            "Current",
            "Diff",
            "%",
            "Base Count",
            "Current Count",
        ]);

    for name in names {
        let baseline = baseline_instructions.get(name);
        let current = current_instructions.get(name);
        let baseline_total = baseline.map_or(0, |row| row.total_gas);
        let current_total = current.map_or(0, |row| row.total_gas);
        let color = diff_color(current_total, baseline_total);

        table.add_row(vec![
            TableCell::new(name)
                .set_alignment(CellAlignment::Left)
                .fg(color),
            right_cell(baseline_total).fg(Color::Grey),
            right_cell(current_total).fg(color),
            TableCell::new(format_signed_diff_u64(current_total, baseline_total))
                .set_alignment(CellAlignment::Right)
                .fg(color),
            TableCell::new(format_percent_change(current_total, baseline_total))
                .set_alignment(CellAlignment::Right)
                .fg(color),
            right_cell(baseline.map_or(0, |row| row.count)).fg(Color::Grey),
            right_cell(current.map_or(0, |row| row.count)).fg(color),
        ]);
    }

    push_table(&mut output, &table);
    output
}

fn instruction_total_diff_abs(
    baseline: Option<&BenchInstructionInfoCell>,
    current: Option<&BenchInstructionInfoCell>,
) -> u64 {
    let baseline = baseline.map_or(0, |row| row.total_gas);
    let current = current.map_or(0, |row| row.total_gas);
    current.abs_diff(baseline)
}

fn right_cell(value: impl ToString) -> TableCell {
    TableCell::new(value.to_string()).set_alignment(CellAlignment::Right)
}

fn diff_color(current: u64, baseline: u64) -> Color {
    match current.cmp(&baseline) {
        Ordering::Greater => Color::DarkRed,
        Ordering::Less => Color::DarkGreen,
        Ordering::Equal => Color::Grey,
    }
}

fn format_bench_name(name: &str) -> String {
    format!("\"{name}\"").green().to_string()
}

fn format_gas_value(value: u64) -> String {
    value.to_string().yellow().to_string()
}

fn push_table(output: &mut String, table: &Table) {
    let rendered = table.to_string();
    for line in rendered.lines() {
        output.push_str(line.trim_end());
        output.push('\n');
    }
}

fn format_signed_diff_u64(current: u64, baseline: u64) -> String {
    match current.cmp(&baseline) {
        Ordering::Greater => format!("+{}", current - baseline),
        Ordering::Less => format!("-{}", baseline - current),
        Ordering::Equal => "0".to_string(),
    }
}

fn format_colored_signed_diff_u64(current: u64, baseline: u64) -> String {
    color_diff_value(format_signed_diff_u64(current, baseline), current, baseline)
}

fn format_percent_change(current: u64, baseline: u64) -> String {
    if baseline == 0 {
        if current == 0 {
            return "0.00%".to_string();
        }
        return "n/a".to_string();
    }

    let percent = ((current as f64 - baseline as f64) / baseline as f64) * 100.0;
    format!("{percent:+.2}%")
}

fn format_colored_percent_change(current: u64, baseline: u64) -> String {
    color_diff_value(format_percent_change(current, baseline), current, baseline)
}

fn color_diff_value(value: String, current: u64, baseline: u64) -> String {
    match current.cmp(&baseline) {
        Ordering::Greater => value.red().to_string(),
        Ordering::Less => value.green().to_string(),
        Ordering::Equal => value.bright_black().to_string(),
    }
}

fn bench_instruction_name(instr_name: &str) -> String {
    if instr_name.starts_with("implicit ") {
        return instr_name.to_string();
    }

    instr_name
        .split_whitespace()
        .next()
        .unwrap_or(instr_name)
        .to_string()
}

#[derive(Debug, Default)]
struct InstructionStats {
    count: u32,
    total_gas: u64,
    min_gas: u64,
    max_gas: u64,
}

#[derive(Debug)]
struct BenchInstructionInfoCell {
    id: u32,
    name: String,
    count: u32,
    total_gas: u64,
    min_gas: u64,
    max_gas: u64,
}

impl BenchInstructionInfoCell {
    fn avg_gas(&self) -> u64 {
        average_gas(self.total_gas, self.count)
    }
}

impl Store for BenchInstructionInfoCell {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        _context: &dyn CellContext,
    ) -> Result<(), Error> {
        let mut name_tuple = Tuple::empty();
        name_tuple.push_string(&self.name);
        let Some(TupleItem::Cell(name_cell)) = name_tuple.0.pop() else {
            return Err(Error::InvalidData);
        };

        builder.store_u32(self.id)?;
        builder.store_reference(name_cell)?;
        builder.store_u32(self.count)?;
        builder.store_u64(self.total_gas)?;
        builder.store_u64(self.min_gas)?;
        builder.store_u64(self.max_gas)?;
        Ok(())
    }
}

impl<'a> Load<'a> for BenchInstructionInfoCell {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let id = u32::load_from(slice)?;
        let name_cell = slice.load_reference_cloned()?;
        let name = Tuple::parse_snake_string(&name_cell).ok_or(Error::InvalidData)?;
        let count = u32::load_from(slice)?;
        let total_gas = u64::load_from(slice)?;
        let min_gas = u64::load_from(slice)?;
        let max_gas = u64::load_from(slice)?;

        Ok(Self {
            id,
            name,
            count,
            total_gas,
            min_gas,
            max_gas,
        })
    }
}

fn average_gas(total_gas: u64, count: u32) -> u64 {
    if count == 0 {
        return 0;
    }
    ((total_gas as f64) / f64::from(count)).round() as u64
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        300 => measure : 5,
        301 => format_profile : 3,
        302 => format_diff : 6,
    });
}
