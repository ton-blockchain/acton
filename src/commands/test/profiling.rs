use crate::commands::test::TestRunner;
use acton_config::color::OwoColorize;
use chrono;
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use ton_emulator::emulator::SendMessageResultSuccess;
use tycho_types::models::{ComputePhase, MsgInfo, TxInfo};

const SIGNIFICANT_PERCENT_CHANGE: f64 = 5.0;

pub(super) fn collect_profile(runner: &TestRunner) -> anyhow::Result<()> {
    let gas_per_opcode = collect_opcode_gas(runner);
    let trace_chain_stats = collect_trace_chain_stats(runner);

    if gas_per_opcode.is_empty() && trace_chain_stats.is_empty() {
        return Ok(());
    }

    let current_snapshot = create_gas_snapshot(&gas_per_opcode, &trace_chain_stats)
        .map_err(|err| anyhow::anyhow!("Failed to create gas snapshot: {err}"))?;

    let baseline_snapshot = if let Some(baseline_path) = &runner.config.baseline_snapshot {
        match load_gas_snapshot(&runner.project_root, baseline_path) {
            Ok(snapshot) => Some(snapshot),
            Err(err) => {
                if runner.config.fail_on_diff {
                    anyhow::bail!("Failed to load baseline gas snapshot '{baseline_path}': {err}");
                }
                eprintln!("Warning: Failed to load baseline gas snapshot '{baseline_path}': {err}",);
                None
            }
        }
    } else {
        None
    };

    print_opcode_gas_table(
        &gas_per_opcode,
        baseline_snapshot.as_ref(),
        runner.config.baseline_snapshot.as_deref(),
    );
    print_trace_chain_table(
        &trace_chain_stats,
        baseline_snapshot.as_ref(),
        runner.config.baseline_snapshot.as_deref(),
    );

    // we don't want to override previous snapshot in compare mode
    if let Some(snapshot_filename) = &runner.config.snapshot
        && runner.config.baseline_snapshot.is_none()
        && let Err(err) =
            save_gas_snapshot(&current_snapshot, &runner.project_root, snapshot_filename)
    {
        anyhow::bail!("Failed to save gas snapshot: {err}")
    }

    if runner.config.fail_on_diff
        && let Some(baseline_path) = runner.config.baseline_snapshot.as_deref()
    {
        let Some(baseline_snapshot) = baseline_snapshot.as_ref() else {
            anyhow::bail!("Failed to load baseline gas snapshot '{baseline_path}'");
        };

        if snapshots_differ(&current_snapshot, baseline_snapshot) {
            anyhow::bail!("Profiling drift detected against baseline snapshot '{baseline_path}'");
        }
    }

    Ok(())
}

fn collect_opcode_gas(runner: &TestRunner) -> HashMap<String, Vec<u64>> {
    let mut gas_per_opcode = HashMap::new();

    for result in runner.emulations.messages() {
        let Some(opcode) = result.opcode() else {
            continue;
        };
        let Some(used_gas) = result.used_gas() else {
            continue;
        };

        let opcode_name = resolve_opcode_name(runner, result, opcode);
        gas_per_opcode
            .entry(opcode_name)
            .or_insert_with(Vec::new)
            .push(used_gas);
    }

    gas_per_opcode
}

fn resolve_opcode_name(
    runner: &TestRunner,
    result: &SendMessageResultSuccess,
    opcode: u32,
) -> String {
    if let Some((_, build_info)) = runner.build_cache.result_for_code(&result.code)
        && let Some(abi) = &build_info.abi
        && let Some(message) = abi.find_type_by_opcode(opcode)
    {
        return message.name;
    }

    for build_info in runner.build_cache.built.values() {
        if let Some(abi) = &build_info.abi
            && let Some(message) = abi.find_type_by_opcode(opcode)
        {
            return message.name;
        }
    }

    format!("0x{opcode:08x}")
}

fn collect_trace_chain_stats(runner: &TestRunner) -> Vec<TraceChainStats> {
    let mut test_names = runner.emulations.results.keys().collect::<Vec<_>>();
    test_names.sort();

    let mut rows = Vec::new();

    for test_name in test_names {
        let Some(emulations) = runner.emulations.results.get(test_name) else {
            continue;
        };

        for (trace_index, trace_transactions) in emulations.messages.iter().enumerate() {
            let trace_name = emulations
                .trace_name(trace_transactions)
                .map_or_else(|| format!("Trace {}", trace_index + 1), ToString::to_string);
            let mut stats = TraceChainStats::new(
                format!("{test_name}::trace#{}", trace_index + 1),
                test_name.clone(),
                trace_name,
                trace_index + 1,
                trace_transactions.len(),
            );

            for transaction in trace_transactions {
                stats.total_fees += u128::from(transaction.transaction.total_fees.tokens);

                if let Ok(TxInfo::Ordinary(info)) = transaction.transaction.load_info()
                    && let ComputePhase::Executed(compute) = info.compute_phase
                {
                    stats.total_gas_used += u64::from(compute.gas_used);
                    stats.total_gas_fees += u128::from(compute.gas_fees);
                }

                if let Ok(Some(in_message)) = transaction.transaction.load_in_msg()
                    && let MsgInfo::Int(message_info) = in_message.info
                {
                    stats.total_forward_fees += u128::from(message_info.fwd_fee);
                }
            }

            rows.push(stats);
        }
    }

    rows
}

fn print_opcode_gas_table(
    gas_per_opcode: &HashMap<String, Vec<u64>>,
    baseline_snapshot: Option<&GasSnapshot>,
    baseline_path: Option<&str>,
) {
    if gas_per_opcode.is_empty() {
        return;
    }

    let baseline_with_opcodes = baseline_snapshot.filter(|snapshot| !snapshot.opcodes.is_empty());

    let mut table = Table::new();
    if let Some(snapshot) = baseline_with_opcodes {
        let baseline_label = baseline_path.unwrap_or("<unknown>");
        let datetime = chrono::DateTime::from_timestamp(snapshot.timestamp as i64, 0)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH);
        let formatted_time = datetime.format("%Y-%m-%d %H:%M:%S UTC");

        println!(
            "\n{} {}\n",
            " GAS USAGE COMPARISON ".bold().on_blue(),
            "".dimmed()
        );
        println!(
            "Baseline: {} ({} opcodes, captured {})",
            baseline_label,
            snapshot.opcodes.len(),
            formatted_time
        );
        println!();

        table
            .load_preset("  ─  ──      ─     ")
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Opcode", "Baseline", "Current", "Diff", "% Change"]);
    } else {
        println!("\n{} {}\n", " GAS USAGE ".bold().on_blue(), "".dimmed());
        table
            .load_preset("  ─  ──      ─     ")
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Opcode", "Min Gas", "Max Gas", "Avg Gas"]);
    }

    let mut opcode_gas_stats: Vec<(String, u64, u64, u64)> = gas_per_opcode
        .iter()
        .map(|(opcode, gas_costs)| {
            let min_gas = *gas_costs.iter().min().unwrap_or(&0);
            let max_gas = *gas_costs.iter().max().unwrap_or(&0);
            let avg_gas =
                (gas_costs.iter().sum::<u64>() as f64 / gas_costs.len() as f64).round() as u64;
            (opcode.clone(), min_gas, max_gas, avg_gas)
        })
        .collect();
    opcode_gas_stats.sort_by(|a, b| b.3.cmp(&a.3));

    if let Some(baseline) = baseline_with_opcodes {
        let mut comparisons: Vec<GasComparison> = opcode_gas_stats
            .iter()
            .filter_map(|(opcode, _, _, current_avg)| {
                baseline.opcodes.get(opcode).map(|baseline_stats| {
                    GasComparison::new(opcode.clone(), *current_avg, baseline_stats.avg_gas)
                })
            })
            .collect();

        for (opcode, _, _, current_avg) in &opcode_gas_stats {
            if !baseline.opcodes.contains_key(opcode) {
                comparisons.push(GasComparison::new(opcode.clone(), *current_avg, 0));
            }
        }

        comparisons.sort_by(|a, b| b.diff.abs().cmp(&a.diff.abs()));

        for comparison in comparisons {
            let color = color_by_percent(comparison.change_percent);
            table.add_row(vec![
                TableCell::new(&comparison.opcode)
                    .set_alignment(CellAlignment::Left)
                    .fg(color),
                TableCell::new(comparison.baseline_avg.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(color),
                TableCell::new(comparison.current_avg.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(color),
                TableCell::new(format_signed_diff_u64(
                    comparison.current_avg,
                    comparison.baseline_avg,
                ))
                .set_alignment(CellAlignment::Right)
                .fg(color),
                TableCell::new(format_percent_change(comparison.change_percent))
                    .set_alignment(CellAlignment::Right)
                    .fg(color),
            ]);
        }
    } else {
        for (opcode, min_gas, max_gas, avg_gas) in opcode_gas_stats {
            table.add_row(vec![
                TableCell::new(opcode).set_alignment(CellAlignment::Left),
                TableCell::new(min_gas.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(Color::DarkMagenta),
                TableCell::new(max_gas.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(Color::DarkCyan),
                TableCell::new(avg_gas.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(Color::DarkGreen),
            ]);
        }
    }

    println!("{table}\n");
}

fn print_trace_chain_table(
    trace_chain_stats: &[TraceChainStats],
    baseline_snapshot: Option<&GasSnapshot>,
    baseline_path: Option<&str>,
) {
    if trace_chain_stats.is_empty() {
        return;
    }

    let baseline_with_traces =
        baseline_snapshot.filter(|snapshot| !snapshot.trace_chains.is_empty());
    let current_by_test = group_current_trace_stats_by_test(trace_chain_stats);
    let baseline_by_test = baseline_with_traces.map(group_baseline_trace_stats_by_test);

    print_trace_summary_table(
        &current_by_test,
        baseline_by_test.as_ref(),
        baseline_with_traces,
        baseline_path,
    );

    for (test_name, traces) in &current_by_test {
        let mut traces = traces.clone();
        traces.sort_by_key(|trace| trace.trace_index);

        if let Some(baseline_by_test) = &baseline_by_test {
            let baseline_rows = baseline_by_test.get(test_name);
            print_test_trace_table_comparison(test_name, &traces, baseline_rows);
        } else {
            print_test_trace_table_current(test_name, &traces);
        }
    }
}

type BaselineTraceByTest<'a> = BTreeMap<String, Vec<(&'a str, &'a TraceChainSnapshotStats)>>;

fn group_current_trace_stats_by_test(
    trace_chain_stats: &[TraceChainStats],
) -> BTreeMap<String, Vec<&TraceChainStats>> {
    let mut grouped = BTreeMap::new();
    for trace in trace_chain_stats {
        grouped
            .entry(trace.test_name.clone())
            .or_insert_with(Vec::new)
            .push(trace);
    }
    grouped
}

fn group_baseline_trace_stats_by_test(snapshot: &GasSnapshot) -> BaselineTraceByTest<'_> {
    let mut grouped: BaselineTraceByTest<'_> = BTreeMap::new();
    for (snapshot_key, trace) in &snapshot.trace_chains {
        grouped
            .entry(trace.test_name.clone())
            .or_default()
            .push((snapshot_key.as_str(), trace));
    }
    grouped
}

fn print_trace_summary_table(
    current_by_test: &BTreeMap<String, Vec<&TraceChainStats>>,
    baseline_by_test: Option<&BaselineTraceByTest<'_>>,
    baseline_snapshot: Option<&GasSnapshot>,
    baseline_path: Option<&str>,
) {
    let mut table = Table::new();

    if let Some(snapshot) = baseline_snapshot {
        let baseline_label = baseline_path.unwrap_or("<unknown>");
        let datetime = chrono::DateTime::from_timestamp(snapshot.timestamp as i64, 0)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH);
        let formatted_time = datetime.format("%Y-%m-%d %H:%M:%S UTC");

        println!(
            "\n{} {}\n",
            " CHAIN GAS & FEES SUMMARY COMPARISON ".bold().on_blue(),
            "".dimmed()
        );
        println!(
            "Baseline: {} ({} traces, captured {})",
            baseline_label,
            snapshot.trace_chains.len(),
            formatted_time
        );
        println!();

        table
            .load_preset("  ─  ──      ─     ")
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "Test",
                "Traces",
                "Tx",
                "Gas Used",
                "Gas Fee",
                "Total Fee",
            ]);

        for (test_name, traces) in current_by_test {
            let current = summarize_current_trace_stats(traces);
            let baseline = baseline_by_test
                .and_then(|by_test| by_test.get(test_name))
                .map(|rows| summarize_baseline_trace_stats(rows));

            if let Some(baseline) = baseline {
                let is_unchanged = current.traces_count == baseline.traces_count
                    && current.tx_count == baseline.tx_count
                    && current.total_gas_used == baseline.total_gas_used
                    && current.total_gas_fees == baseline.total_gas_fees
                    && current.total_fees == baseline.total_fees;

                if is_unchanged {
                    let color = Some(Color::DarkGrey);
                    table.add_row(vec![
                        apply_optional_row_color(
                            TableCell::new(test_name).set_alignment(CellAlignment::Left),
                            color,
                        ),
                        apply_optional_row_color(
                            TableCell::new(current.traces_count.to_string())
                                .set_alignment(CellAlignment::Right),
                            color,
                        ),
                        apply_optional_row_color(
                            TableCell::new(current.tx_count.to_string())
                                .set_alignment(CellAlignment::Right),
                            color,
                        ),
                        apply_optional_row_color(
                            TableCell::new(current.total_gas_used.to_string())
                                .set_alignment(CellAlignment::Right),
                            color,
                        ),
                        apply_optional_row_color(
                            TableCell::new(format_ton(current.total_gas_fees))
                                .set_alignment(CellAlignment::Right),
                            color,
                        ),
                        apply_optional_row_color(
                            TableCell::new(format_ton(current.total_fees))
                                .set_alignment(CellAlignment::Right),
                            color,
                        ),
                    ]);
                } else {
                    table.add_row(vec![
                        TableCell::new(test_name).set_alignment(CellAlignment::Left),
                        TableCell::new(baseline.traces_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(baseline.tx_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(baseline.total_gas_used.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(format_ton(baseline.total_gas_fees))
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(format_ton(baseline.total_fees))
                            .set_alignment(CellAlignment::Right),
                    ]);

                    table.add_row(vec![
                        TableCell::new("").set_alignment(CellAlignment::Left),
                        TableCell::new(current.traces_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(current.tx_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(current.total_gas_used.to_string())
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(format_ton(current.total_gas_fees))
                            .set_alignment(CellAlignment::Right),
                        TableCell::new(format_ton(current.total_fees))
                            .set_alignment(CellAlignment::Right),
                    ]);

                    table.add_row(vec![
                        TableCell::new("").set_alignment(CellAlignment::Left),
                        diff_cell_usize(current.traces_count, baseline.traces_count),
                        diff_cell_usize(current.tx_count, baseline.tx_count),
                        diff_cell_u64(current.total_gas_used, baseline.total_gas_used),
                        diff_cell_ton(current.total_gas_fees, baseline.total_gas_fees),
                        diff_cell_ton(current.total_fees, baseline.total_fees),
                    ]);
                }
            } else {
                let color = Some(Color::DarkCyan);
                table.add_row(vec![
                    apply_optional_row_color(
                        TableCell::new(test_name).set_alignment(CellAlignment::Left),
                        color,
                    ),
                    apply_optional_row_color(
                        TableCell::new(current.traces_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        color,
                    ),
                    apply_optional_row_color(
                        TableCell::new(current.tx_count.to_string())
                            .set_alignment(CellAlignment::Right),
                        color,
                    ),
                    apply_optional_row_color(
                        TableCell::new(current.total_gas_used.to_string())
                            .set_alignment(CellAlignment::Right),
                        color,
                    ),
                    apply_optional_row_color(
                        TableCell::new(format_ton(current.total_gas_fees))
                            .set_alignment(CellAlignment::Right),
                        color,
                    ),
                    apply_optional_row_color(
                        TableCell::new(format_ton(current.total_fees))
                            .set_alignment(CellAlignment::Right),
                        color,
                    ),
                ]);
            }
        }
    } else {
        println!(
            "\n{} {}\n",
            " CHAIN GAS & FEES SUMMARY ".bold().on_blue(),
            "".dimmed()
        );
        table
            .load_preset("  ─  ──      ─     ")
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "Test",
                "Traces",
                "Tx",
                "Gas Used",
                "Gas Fee",
                "Forward Fee",
                "Total Fee",
            ]);

        for (test_name, traces) in current_by_test {
            let current = summarize_current_trace_stats(traces);
            table.add_row(vec![
                TableCell::new(test_name).set_alignment(CellAlignment::Left),
                TableCell::new(current.traces_count.to_string())
                    .set_alignment(CellAlignment::Right),
                TableCell::new(current.tx_count.to_string()).set_alignment(CellAlignment::Right),
                TableCell::new(current.total_gas_used.to_string())
                    .set_alignment(CellAlignment::Right),
                TableCell::new(format_ton(current.total_gas_fees))
                    .set_alignment(CellAlignment::Right),
                TableCell::new(format_ton(current.total_forward_fees))
                    .set_alignment(CellAlignment::Right),
                TableCell::new(format_ton(current.total_fees)).set_alignment(CellAlignment::Right),
            ]);
        }
    }

    println!("{table}\n");
}

fn print_test_trace_table_current(test_name: &str, traces: &[&TraceChainStats]) {
    println!(
        "\n{} {}\n",
        format!(" CHAIN GAS & FEES · {test_name} ").bold().on_blue(),
        "".dimmed()
    );

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Trace",
            "Tx Count",
            "Gas Used",
            "Gas Fee",
            "Forward Fee",
            "Total Fee",
        ]);

    for trace in traces {
        table.add_row(vec![
            TableCell::new(&trace.trace_name).set_alignment(CellAlignment::Left),
            TableCell::new(trace.tx_count.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(trace.total_gas_used.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(trace.total_gas_fees)).set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(trace.total_forward_fees))
                .set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(trace.total_fees)).set_alignment(CellAlignment::Right),
        ]);
    }

    println!("{table}\n");
}

fn print_test_trace_table_comparison(
    test_name: &str,
    traces: &[&TraceChainStats],
    baseline_rows: Option<&Vec<(&str, &TraceChainSnapshotStats)>>,
) {
    println!(
        "\n{} {}\n",
        format!(" CHAIN GAS & FEES · {test_name} ").bold().on_blue(),
        "".dimmed()
    );

    let baseline_by_key = baseline_rows
        .map(|rows| {
            rows.iter()
                .map(|(snapshot_key, trace)| (*snapshot_key, *trace))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Trace",
            "Tx Count",
            "Gas Used",
            "Gas Fee",
            "Fwd Fee",
            "Total Fee",
        ]);

    for trace in traces {
        let baseline = baseline_by_key.get(trace.snapshot_key.as_str()).copied();
        let comparison = TraceChainComparison::new(trace, baseline);
        if comparison.baseline_tx_count.is_none() {
            let color = Some(Color::DarkCyan);
            table.add_row(vec![
                apply_optional_row_color(
                    TableCell::new(&trace.trace_name).set_alignment(CellAlignment::Left),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(comparison.current_tx_count.to_string())
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(comparison.current_total_gas_used.to_string())
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_gas_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_forward_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
            ]);
            continue;
        }

        if comparison.is_fully_unchanged() {
            let color = Some(Color::DarkGrey);
            table.add_row(vec![
                apply_optional_row_color(
                    TableCell::new(&trace.trace_name).set_alignment(CellAlignment::Left),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(comparison.current_tx_count.to_string())
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(comparison.current_total_gas_used.to_string())
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_gas_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_forward_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
                apply_optional_row_color(
                    TableCell::new(format_ton(comparison.current_total_fees))
                        .set_alignment(CellAlignment::Right),
                    color,
                ),
            ]);
            continue;
        }

        let baseline_tx_count = comparison.baseline_tx_count.unwrap_or_default();
        let baseline_total_gas_used = comparison.baseline_total_gas_used.unwrap_or_default();
        let baseline_total_gas_fees = comparison.baseline_total_gas_fees.unwrap_or_default();
        let baseline_total_forward_fees =
            comparison.baseline_total_forward_fees.unwrap_or_default();
        let baseline_total_fees = comparison.baseline_total_fees.unwrap_or_default();

        table.add_row(vec![
            TableCell::new(&trace.trace_name).set_alignment(CellAlignment::Left),
            TableCell::new(baseline_tx_count.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(baseline_total_gas_used.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(baseline_total_gas_fees)).set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(baseline_total_forward_fees))
                .set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(baseline_total_fees)).set_alignment(CellAlignment::Right),
        ]);

        table.add_row(vec![
            TableCell::new("").set_alignment(CellAlignment::Left),
            TableCell::new(comparison.current_tx_count.to_string())
                .set_alignment(CellAlignment::Right),
            TableCell::new(comparison.current_total_gas_used.to_string())
                .set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(comparison.current_total_gas_fees))
                .set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(comparison.current_total_forward_fees))
                .set_alignment(CellAlignment::Right),
            TableCell::new(format_ton(comparison.current_total_fees))
                .set_alignment(CellAlignment::Right),
        ]);

        table.add_row(vec![
            TableCell::new("").set_alignment(CellAlignment::Left),
            diff_cell_usize(comparison.current_tx_count, baseline_tx_count),
            diff_cell_u64(comparison.current_total_gas_used, baseline_total_gas_used),
            diff_cell_ton(comparison.current_total_gas_fees, baseline_total_gas_fees),
            diff_cell_ton(
                comparison.current_total_forward_fees,
                baseline_total_forward_fees,
            ),
            diff_cell_ton(comparison.current_total_fees, baseline_total_fees),
        ]);
    }

    println!("{table}\n");
}

#[derive(Debug, Clone, Copy, Default)]
struct AggregatedTraceStats {
    traces_count: usize,
    tx_count: usize,
    total_gas_used: u64,
    total_gas_fees: u128,
    total_forward_fees: u128,
    total_fees: u128,
}

fn summarize_current_trace_stats(rows: &[&TraceChainStats]) -> AggregatedTraceStats {
    let mut stats = AggregatedTraceStats::default();
    for row in rows {
        stats.traces_count += 1;
        stats.tx_count += row.tx_count;
        stats.total_gas_used += row.total_gas_used;
        stats.total_gas_fees += row.total_gas_fees;
        stats.total_forward_fees += row.total_forward_fees;
        stats.total_fees += row.total_fees;
    }
    stats
}

fn summarize_baseline_trace_stats(
    rows: &[(&str, &TraceChainSnapshotStats)],
) -> AggregatedTraceStats {
    let mut stats = AggregatedTraceStats::default();
    for (_, row) in rows {
        stats.traces_count += 1;
        stats.tx_count += row.tx_count;
        stats.total_gas_used += row.total_gas_used;
        stats.total_gas_fees += row.total_gas_fees;
        stats.total_forward_fees += row.total_forward_fees;
        stats.total_fees += row.total_fees;
    }
    stats
}

fn create_gas_snapshot(
    gas_per_opcode: &HashMap<String, Vec<u64>>,
    trace_chain_stats: &[TraceChainStats],
) -> anyhow::Result<GasSnapshot> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let mut opcodes = HashMap::new();

    for (opcode, gas_costs) in gas_per_opcode {
        let min_gas = *gas_costs.iter().min().unwrap_or(&0);
        let max_gas = *gas_costs.iter().max().unwrap_or(&0);
        let avg_gas =
            (gas_costs.iter().sum::<u64>() as f64 / gas_costs.len() as f64).round() as u64;
        let samples = gas_costs.len();

        opcodes.insert(
            opcode.clone(),
            OpcodeGasStats {
                min_gas,
                max_gas,
                avg_gas,
                samples,
                all_values: gas_costs.clone(),
            },
        );
    }

    let mut trace_chains = HashMap::new();
    for chain in trace_chain_stats {
        trace_chains.insert(
            chain.snapshot_key.clone(),
            TraceChainSnapshotStats {
                test_name: chain.test_name.clone(),
                trace_name: chain.trace_name.clone(),
                tx_count: chain.tx_count,
                total_gas_used: chain.total_gas_used,
                total_gas_fees: chain.total_gas_fees,
                total_forward_fees: chain.total_forward_fees,
                total_fees: chain.total_fees,
            },
        );
    }

    Ok(GasSnapshot {
        timestamp,
        opcodes,
        trace_chains,
    })
}

fn snapshots_differ(current: &GasSnapshot, baseline: &GasSnapshot) -> bool {
    current.opcodes != baseline.opcodes || current.trace_chains != baseline.trace_chains
}

fn save_gas_snapshot(
    snapshot: &GasSnapshot,
    project_root: &Path,
    filename: &str,
) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(snapshot)?;
    let path = resolve_snapshot_path(project_root, filename);
    fs::write(path, json)?;
    println!("Gas snapshot saved to {filename}");
    Ok(())
}

fn load_gas_snapshot(project_root: &Path, filename: &str) -> anyhow::Result<GasSnapshot> {
    let path = resolve_snapshot_path(project_root, filename);
    let content = fs::read_to_string(path)?;
    let snapshot: GasSnapshot = serde_json::from_str(&content)?;
    Ok(snapshot)
}

fn resolve_snapshot_path(project_root: &Path, filename: &str) -> PathBuf {
    let path = Path::new(filename);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct GasSnapshot {
    pub timestamp: u64,
    pub opcodes: HashMap<String, OpcodeGasStats>,
    #[serde(default)]
    pub trace_chains: HashMap<String, TraceChainSnapshotStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct OpcodeGasStats {
    pub min_gas: u64,
    pub max_gas: u64,
    pub avg_gas: u64,
    pub samples: usize,
    pub all_values: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct TraceChainSnapshotStats {
    pub test_name: String,
    pub trace_name: String,
    pub tx_count: usize,
    pub total_gas_used: u64,
    pub total_gas_fees: u128,
    pub total_forward_fees: u128,
    pub total_fees: u128,
}

#[derive(Debug)]
struct GasComparison {
    opcode: String,
    current_avg: u64,
    baseline_avg: u64,
    diff: i64,
    change_percent: Option<f64>,
}

impl GasComparison {
    fn new(opcode: String, current_avg: u64, baseline_avg: u64) -> Self {
        let diff = current_avg as i64 - baseline_avg as i64;
        let change_percent = calculate_percent_change_u64(current_avg, baseline_avg);

        Self {
            opcode,
            current_avg,
            baseline_avg,
            diff,
            change_percent,
        }
    }
}

#[derive(Debug)]
struct TraceChainStats {
    snapshot_key: String,
    test_name: String,
    trace_name: String,
    trace_index: usize,
    tx_count: usize,
    total_gas_used: u64,
    total_gas_fees: u128,
    total_forward_fees: u128,
    total_fees: u128,
}

impl TraceChainStats {
    const fn new(
        snapshot_key: String,
        test_name: String,
        trace_name: String,
        trace_index: usize,
        tx_count: usize,
    ) -> Self {
        Self {
            snapshot_key,
            test_name,
            trace_name,
            trace_index,
            tx_count,
            total_gas_used: 0,
            total_gas_fees: 0,
            total_forward_fees: 0,
            total_fees: 0,
        }
    }
}

#[derive(Debug)]
struct TraceChainComparison {
    baseline_tx_count: Option<usize>,
    current_tx_count: usize,
    baseline_total_gas_used: Option<u64>,
    current_total_gas_used: u64,
    baseline_total_gas_fees: Option<u128>,
    current_total_gas_fees: u128,
    baseline_total_forward_fees: Option<u128>,
    current_total_forward_fees: u128,
    baseline_total_fees: Option<u128>,
    current_total_fees: u128,
}

impl TraceChainComparison {
    fn new(current: &TraceChainStats, baseline: Option<&TraceChainSnapshotStats>) -> Self {
        let baseline_tx_count = baseline.map(|it| it.tx_count);
        let baseline_total_gas_used = baseline.map(|it| it.total_gas_used);
        let baseline_total_gas_fees = baseline.map(|it| it.total_gas_fees);
        let baseline_total_forward_fees = baseline.map(|it| it.total_forward_fees);
        let baseline_total_fees = baseline.map(|it| it.total_fees);

        Self {
            baseline_tx_count,
            current_tx_count: current.tx_count,
            baseline_total_gas_used,
            current_total_gas_used: current.total_gas_used,
            baseline_total_gas_fees,
            current_total_gas_fees: current.total_gas_fees,
            baseline_total_forward_fees,
            current_total_forward_fees: current.total_forward_fees,
            baseline_total_fees,
            current_total_fees: current.total_fees,
        }
    }

    const fn is_fully_unchanged(&self) -> bool {
        let Some(baseline_tx_count) = self.baseline_tx_count else {
            return false;
        };
        let Some(baseline_total_gas_used) = self.baseline_total_gas_used else {
            return false;
        };
        let Some(baseline_total_gas_fees) = self.baseline_total_gas_fees else {
            return false;
        };
        let Some(baseline_total_forward_fees) = self.baseline_total_forward_fees else {
            return false;
        };
        let Some(baseline_total_fees) = self.baseline_total_fees else {
            return false;
        };

        self.current_tx_count == baseline_tx_count
            && self.current_total_gas_used == baseline_total_gas_used
            && self.current_total_gas_fees == baseline_total_gas_fees
            && self.current_total_forward_fees == baseline_total_forward_fees
            && self.current_total_fees == baseline_total_fees
    }
}

fn format_ton(nanotons: u128) -> String {
    const NANO_PER_TON: u128 = 1_000_000_000;

    let whole = nanotons / NANO_PER_TON;
    let fraction = nanotons % NANO_PER_TON;
    if fraction == 0 {
        return format!("{whole} TON");
    }

    let fraction = format!("{fraction:09}");
    let fraction = fraction.trim_end_matches('0');
    format!("{whole}.{fraction} TON")
}

fn apply_optional_row_color(cell: TableCell, color: Option<Color>) -> TableCell {
    if let Some(color) = color {
        cell.fg(color)
    } else {
        cell
    }
}

fn diff_cell_usize(current: usize, baseline: usize) -> TableCell {
    match current.cmp(&baseline) {
        Ordering::Equal => TableCell::new("").set_alignment(CellAlignment::Right),
        Ordering::Greater => TableCell::new(format_signed_diff_usize(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkRed),
        Ordering::Less => TableCell::new(format_signed_diff_usize(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkGreen),
    }
}

fn diff_cell_u64(current: u64, baseline: u64) -> TableCell {
    match current.cmp(&baseline) {
        Ordering::Equal => TableCell::new("").set_alignment(CellAlignment::Right),
        Ordering::Greater => TableCell::new(format_signed_diff_u64(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkRed),
        Ordering::Less => TableCell::new(format_signed_diff_u64(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkGreen),
    }
}

fn diff_cell_ton(current: u128, baseline: u128) -> TableCell {
    match current.cmp(&baseline) {
        Ordering::Equal => TableCell::new("").set_alignment(CellAlignment::Right),
        Ordering::Greater => TableCell::new(format_signed_diff_u128(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkRed),
        Ordering::Less => TableCell::new(format_signed_diff_u128(current, baseline))
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkGreen),
    }
}

fn format_signed_diff_usize(current: usize, baseline: usize) -> String {
    if current >= baseline {
        format!("+{}", current - baseline)
    } else {
        format!("-{}", baseline - current)
    }
}

fn format_signed_diff_u64(current: u64, baseline: u64) -> String {
    if current >= baseline {
        format!("+{}", current - baseline)
    } else {
        format!("-{}", baseline - current)
    }
}

fn format_signed_diff_u128(current: u128, baseline: u128) -> String {
    if current >= baseline {
        format!("+{}", format_ton(current - baseline))
    } else {
        format!("-{}", format_ton(baseline - current))
    }
}

fn calculate_percent_change_u64(current: u64, baseline: u64) -> Option<f64> {
    if baseline == 0 {
        return None;
    }

    Some((current as f64 - baseline as f64) / baseline as f64 * 100.0)
}

fn format_percent_change(change_percent: Option<f64>) -> String {
    change_percent.map_or_else(|| "NEW".to_string(), |v| format!("{v:+.1}%"))
}

fn color_by_percent(change_percent: Option<f64>) -> Color {
    match change_percent {
        None => Color::DarkCyan,
        Some(percent) if percent < -SIGNIFICANT_PERCENT_CHANGE => Color::DarkGreen,
        Some(percent) if percent > SIGNIFICANT_PERCENT_CHANGE => Color::DarkRed,
        Some(_) => Color::Grey,
    }
}
