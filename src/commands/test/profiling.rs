use crate::commands::test::TestRunner;
use acton_config::color::{OwoColorize, colors_enabled};
use acton_config::test::GasProfileFormat;
use acton_debug::replayer::{CallFrameInfo, StepMode, Tick, TolkReplayer};
use chrono;
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tolk_compiler::SourceMap;
use ton_emulator::emulator::SendMessageResultSuccess;
use ton_executor::get::DEFAULT_GET_METHOD_GAS_LIMIT;
use ton_retrace::trace::{Trace, TraceStep};
use tycho_types::boc::Boc;
use tycho_types::models::{ComputePhase, MsgInfo, TxInfo};

const SIGNIFICANT_PERCENT_CHANGE: f64 = 5.0;

pub(super) fn collect_profile(runner: &TestRunner) -> anyhow::Result<Option<UiGasProfileReport>> {
    let collect_snapshot_stats =
        runner.config.snapshot.is_some() || runner.config.baseline_snapshot.is_some();
    let mut ui_gas_profile = None;

    if collect_snapshot_stats {
        let gas_per_opcode = collect_opcode_gas(runner);
        let trace_chain_stats = collect_trace_chain_stats(runner);

        if gas_per_opcode.is_empty() && trace_chain_stats.is_empty() {
            if runner.config.gas_profile.is_none() {
                return Ok(None);
            }
        } else {
            let current_snapshot = create_gas_snapshot(&gas_per_opcode, &trace_chain_stats)
                .map_err(|err| anyhow::anyhow!("Failed to create gas snapshot: {err}"))?;

            let baseline_snapshot = if let Some(baseline_path) = &runner.config.baseline_snapshot {
                Some(
                    load_gas_snapshot(&runner.project_root, baseline_path).map_err(|err| {
                        anyhow::anyhow!(
                            "Failed to load baseline gas snapshot '{baseline_path}': {err}"
                        )
                    })?,
                )
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
                && let (Some(baseline_path), Some(baseline_snapshot)) = (
                    runner.config.baseline_snapshot.as_deref(),
                    baseline_snapshot.as_ref(),
                )
                && snapshots_differ(&current_snapshot, baseline_snapshot)
            {
                anyhow::bail!(
                    "Profiling drift detected against baseline snapshot '{baseline_path}'"
                );
            }
        }
    }

    if let Some(gas_profile_filename) = &runner.config.gas_profile {
        let executions = collect_profile_executions(runner);
        let execution_samples = collect_profile_execution_samples(&executions);
        save_execution_profile(
            &execution_samples,
            runner.config.gas_profile_format,
            &runner.project_root,
            gas_profile_filename,
        )?;
        if runner.config.ui {
            ui_gas_profile = Some(build_ui_gas_profile(&execution_samples));
        }
    }

    Ok(ui_gas_profile)
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
    runner
        .build_cache
        .message_name_by_opcode(
            opcode,
            runner
                .build_cache
                .result_for_code(&result.code)
                .map(|(_, result)| result),
        )
        .unwrap_or_else(|| format!("0x{opcode:08x}"))
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

        print_section_header("GAS USAGE COMPARISON");
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
        print_section_header("GAS USAGE");
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

        print_section_header("CHAIN GAS & FEES SUMMARY COMPARISON");
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
        print_section_header("CHAIN GAS & FEES SUMMARY");
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
    print_section_header(&format!("CHAIN GAS & FEES · {test_name}"));

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
    print_section_header(&format!("CHAIN GAS & FEES · {test_name}"));

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

fn print_section_header(title: &str) {
    let padded = if colors_enabled() {
        format!(" {title} ")
    } else {
        format!(" {title}")
    };
    println!("\n{}\n", padded.bold().on_blue());
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
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
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

fn save_execution_profile(
    execution_samples: &[ProfileExecutionSamples],
    format: GasProfileFormat,
    project_root: &Path,
    filename: &str,
) -> anyhow::Result<()> {
    let content = match format {
        GasProfileFormat::Cpuprofile => {
            serde_json::to_string_pretty(&build_devtools_cpu_profile(execution_samples))?
        }
        GasProfileFormat::Collapsed => build_collapsed_profile(execution_samples),
    };
    let path = resolve_snapshot_path(project_root, filename);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    println!("Gas profile saved to {filename}");
    Ok(())
}

fn build_devtools_cpu_profile(execution_samples: &[ProfileExecutionSamples]) -> DevToolsCpuProfile {
    let mut builder = DevToolsCpuProfileBuilder::new();

    for execution in execution_samples {
        for sample in &execution.samples {
            builder.add_sample(&sample.frames, sample.weight);
        }
    }

    builder.finish()
}

fn build_collapsed_profile(execution_samples: &[ProfileExecutionSamples]) -> String {
    let mut samples_by_stack = BTreeMap::<(String, Vec<String>), u64>::new();

    for execution in execution_samples {
        for sample in &execution.samples {
            let stack = sample
                .frames
                .iter()
                .map(|frame| sanitize_collapsed_name(&frame.function_name))
                .collect::<Vec<_>>();

            *samples_by_stack
                .entry((sample.thread_name.clone(), stack))
                .or_insert(0) += sample.weight;
        }
    }

    let mut output = String::new();
    for ((thread_name, frames), weight) in samples_by_stack {
        output.push_str(&thread_name);
        for frame in frames {
            output.push(';');
            output.push_str(&frame);
        }
        output.push(' ');
        output.push_str(&weight.to_string());
        output.push('\n');
    }

    output
}

fn build_ui_gas_profile(execution_samples: &[ProfileExecutionSamples]) -> UiGasProfileReport {
    let (total_gas, contracts) =
        build_ui_gas_profile_contracts(execution_samples.iter(), UiGasProfileFrameFilter::All);
    let mut executions_by_test = BTreeMap::<String, Vec<&ProfileExecutionSamples>>::new();

    for execution in execution_samples {
        if execution.samples.is_empty() {
            continue;
        }

        executions_by_test
            .entry(execution.test_name.clone())
            .or_default()
            .push(execution);
    }

    let tests = executions_by_test
        .into_iter()
        .filter_map(|(name, executions)| {
            let (total_gas, contracts) = build_ui_gas_profile_contracts(
                executions,
                UiGasProfileFrameFilter::ExcludeActonRuntime,
            );
            (total_gas > 0).then_some(UiGasProfileTestReport {
                name,
                total_gas,
                contracts,
            })
        })
        .collect();

    UiGasProfileReport {
        total_gas,
        contracts,
        tests,
    }
}

fn build_ui_gas_profile_contracts<'a>(
    execution_samples: impl IntoIterator<Item = &'a ProfileExecutionSamples>,
    frame_filter: UiGasProfileFrameFilter,
) -> (u64, Vec<UiGasProfileContract>) {
    let mut contracts = BTreeMap::<String, UiGasProfileContract>::new();

    for execution in execution_samples {
        if execution.samples.is_empty() {
            continue;
        }

        let contract_name = execution
            .contract_display_name
            .clone()
            .unwrap_or_else(|| "Tests".to_string());
        let contract_prefix = format!("{contract_name}:");

        let contract =
            contracts
                .entry(contract_name.clone())
                .or_insert_with(|| UiGasProfileContract {
                    name: contract_name.clone(),
                    total_gas: 0,
                    sample_count: 0,
                    samples: Vec::new(),
                });

        for sample in &execution.samples {
            let Some(frames) = build_ui_sample_frames(sample, &contract_prefix, frame_filter)
            else {
                continue;
            };

            contract.total_gas += sample.weight;
            contract.sample_count += 1;
            contract.samples.push(UiGasProfileSample {
                weight: sample.weight,
                frames,
            });
        }
    }

    let total_gas = contracts.values().map(|contract| contract.total_gas).sum();
    let mut contracts = contracts.into_values().collect::<Vec<_>>();
    contracts.sort_by(|a, b| {
        b.total_gas
            .cmp(&a.total_gas)
            .then_with(|| a.name.cmp(&b.name))
    });

    (total_gas, contracts)
}

#[derive(Clone, Copy)]
enum UiGasProfileFrameFilter {
    All,
    ExcludeActonRuntime,
}

fn build_ui_sample_frames(
    sample: &ProfileSample,
    contract_prefix: &str,
    frame_filter: UiGasProfileFrameFilter,
) -> Option<Vec<UiGasProfileFrame>> {
    let exclude_acton_runtime =
        matches!(frame_filter, UiGasProfileFrameFilter::ExcludeActonRuntime);
    if exclude_acton_runtime
        && sample
            .frames
            .last()
            .is_some_and(|frame| is_acton_profile_source(&frame.url))
    {
        return None;
    }

    let frames = sample
        .frames
        .iter()
        .filter(|frame| !exclude_acton_runtime || !is_acton_profile_source(&frame.url))
        .map(|frame| UiGasProfileFrame {
            function_name: frame
                .function_name
                .strip_prefix(contract_prefix)
                .unwrap_or(&frame.function_name)
                .to_string(),
            url: frame.url.clone(),
            line_number: frame.line_number,
            column_number: frame.column_number,
        })
        .collect::<Vec<_>>();

    (!frames.is_empty()).then_some(frames)
}

fn is_acton_profile_source(url: &str) -> bool {
    let normalized = url.replace('\\', "/");
    let manifest_lib = format!("{}/lib/", env!("CARGO_MANIFEST_DIR").replace('\\', "/"));

    normalized == "@acton"
        || normalized.starts_with("@acton/")
        || normalized.contains("/@acton/")
        || normalized.contains("/.acton/")
        || normalized.starts_with(&manifest_lib)
}

fn collect_profile_executions(runner: &TestRunner) -> Vec<ProfileExecutionInput> {
    let mut test_names = runner
        .emulations
        .results
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    test_names.sort();

    let mut executions = Vec::new();

    for test_name in test_names {
        let Some(emulations) = runner.emulations.results.get(&test_name) else {
            continue;
        };

        for trace_transactions in &emulations.messages {
            for tx in trace_transactions {
                let Some((_, build_result)) = runner.build_cache.result_for_code(&tx.code) else {
                    continue;
                };

                executions.push(ProfileExecutionInput {
                    test_name: test_name.clone(),
                    vm_log: tx.vm_log.clone(),
                    initial_gas: tx.initial_gas(),
                    source_map: build_result.source_map.clone(),
                    contract_display_name: Some(build_result.display_name),
                });
            }
        }

        if runner.config.gas_profile_include_tests {
            for get_result in &emulations.get_methods {
                let Ok(code) = Boc::decode_base64(get_result.code.as_ref()) else {
                    continue;
                };
                let Some((_, build_result)) = runner.build_cache.result_for_code(&Some(code))
                else {
                    continue;
                };

                executions.push(ProfileExecutionInput {
                    test_name: test_name.clone(),
                    vm_log: get_result.vm_log.clone(),
                    initial_gas: Some(DEFAULT_GET_METHOD_GAS_LIMIT as u64),
                    source_map: build_result.source_map.clone(),
                    contract_display_name: None,
                });
            }
        }
    }

    executions
}

fn collect_profile_execution_samples(
    executions: &[ProfileExecutionInput],
) -> Vec<ProfileExecutionSamples> {
    executions
        .iter()
        .map(|execution| ProfileExecutionSamples {
            test_name: execution.test_name.clone(),
            contract_display_name: execution.contract_display_name.clone(),
            samples: collect_execution_samples(execution),
        })
        .collect()
}

fn collect_execution_samples(execution: &ProfileExecutionInput) -> Vec<ProfileSample> {
    let trace = Trace::new(
        &execution.vm_log,
        execution
            .initial_gas
            .and_then(|gas| usize::try_from(gas).ok()),
    );
    let execute_steps = trace
        .steps
        .iter()
        .filter_map(|step| match step {
            TraceStep::Execute { instr, gas, .. } => Some(InstructionGasStep {
                instr_name: instr.clone(),
                gas: *gas as u64,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    if execute_steps.is_empty() {
        return Vec::new();
    }

    let Ok(mut replayer) = TolkReplayer::new(execution.source_map.as_ref(), &execution.vm_log)
    else {
        return Vec::new();
    };
    let mut samples = Vec::new();
    let mut execute_idx = 0usize;

    replayer.step_with_callback(StepMode::RunUntilBreakpoint, |tick, state| match tick {
        Tick::TvmImplicitJmpRef => {
            if let Some(sample) = record_execution_sample(
                execution,
                &execute_steps,
                &mut execute_idx,
                state,
                Some("implicit JMPREF"),
            ) {
                samples.push(sample);
            }
        }
        Tick::TvmBeforeExecute => {
            while execute_steps
                .get(execute_idx)
                .is_some_and(|step| step.instr_name == "implicit JMPREF")
            {
                if let Some(sample) = record_execution_sample(
                    execution,
                    &execute_steps,
                    &mut execute_idx,
                    state,
                    Some("implicit JMPREF"),
                ) {
                    samples.push(sample);
                }
            }

            if let Some(sample) =
                record_execution_sample(execution, &execute_steps, &mut execute_idx, state, None)
            {
                samples.push(sample);
            }
        }
        _ => {}
    });

    samples
}

fn record_execution_sample(
    execution: &ProfileExecutionInput,
    execute_steps: &[InstructionGasStep],
    execute_idx: &mut usize,
    replayer: &TolkReplayer,
    expected_instr: Option<&str>,
) -> Option<ProfileSample> {
    let step = execute_steps.get(*execute_idx)?;

    if let Some(expected_instr) = expected_instr
        && step.instr_name != expected_instr
    {
        return None;
    }

    *execute_idx += 1;

    if step.gas == 0 {
        return None;
    }

    let frames = build_profile_frames(execution.contract_display_name.as_deref(), replayer);

    if frames.is_empty() {
        return None;
    }

    Some(ProfileSample {
        thread_name: "acton".to_string(),
        frames,
        weight: step.gas,
    })
}

fn build_profile_frames(
    contract_display_name: Option<&str>,
    replayer: &TolkReplayer,
) -> Vec<ProfileFrameSpec> {
    replayer
        .call_stack()
        .iter()
        .map(|frame| ProfileFrameSpec::from_call_frame(frame, contract_display_name, replayer))
        .collect()
}

#[derive(Debug, Clone)]
struct ProfileExecutionInput {
    test_name: String,
    vm_log: Arc<str>,
    initial_gas: Option<u64>,
    source_map: Arc<SourceMap>,
    contract_display_name: Option<String>,
}

#[derive(Debug, Clone)]
struct ProfileExecutionSamples {
    test_name: String,
    contract_display_name: Option<String>,
    samples: Vec<ProfileSample>,
}

#[derive(Debug, Clone)]
struct InstructionGasStep {
    instr_name: String,
    gas: u64,
}

#[derive(Debug, Clone)]
struct ProfileSample {
    thread_name: String,
    frames: Vec<ProfileFrameSpec>,
    weight: u64,
}

#[derive(Debug, Clone)]
struct ProfileFrameSpec {
    function_name: String,
    url: String,
    line_number: i64,
    column_number: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiGasProfileReport {
    pub total_gas: u64,
    pub contracts: Vec<UiGasProfileContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<UiGasProfileTestReport>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiGasProfileTestReport {
    pub name: String,
    pub total_gas: u64,
    pub contracts: Vec<UiGasProfileContract>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiGasProfileContract {
    pub name: String,
    pub total_gas: u64,
    pub sample_count: usize,
    pub samples: Vec<UiGasProfileSample>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiGasProfileSample {
    pub weight: u64,
    pub frames: Vec<UiGasProfileFrame>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UiGasProfileFrame {
    pub function_name: String,
    pub url: String,
    pub line_number: i64,
    pub column_number: i64,
}

impl ProfileFrameSpec {
    fn from_call_frame(
        frame: &CallFrameInfo,
        contract_display_name: Option<&str>,
        replayer: &TolkReplayer,
    ) -> Self {
        let location = frame
            .definition_loc
            .as_ref()
            .or(frame.call_site_loc.as_ref());
        let (url, line_number, column_number) = location.map_or_else(
            || (String::new(), -1, -1),
            |loc| frame_location(replayer, loc),
        );

        Self {
            function_name: format_profile_function_name(
                frame.f_name.as_str(),
                contract_display_name,
            ),
            url,
            line_number,
            column_number,
        }
    }
}

fn format_profile_function_name(
    function_name: &str,
    contract_display_name: Option<&str>,
) -> String {
    if matches!(
        function_name,
        "onInternalMessage" | "onExternalMessage" | "onBouncedMessage" | "onRunTickTock"
    ) && let Some(contract_display_name) = contract_display_name
    {
        return format!("{contract_display_name}:{function_name}");
    }

    function_name.to_string()
}

fn frame_location(
    replayer: &TolkReplayer,
    range: &tolk_compiler::source_map::SrcRange,
) -> (String, i64, i64) {
    let url = replayer.file_full_path(range.file_id()).unwrap_or_default();

    (
        url.to_string(),
        zero_based_line(range.start_line()),
        zero_based_column(range.start_col()),
    )
}

fn sanitize_collapsed_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            ';' => ':',
            '\n' | '\r' | '\t' => ' ',
            _ => ch,
        })
        .collect()
}

fn zero_based_line(line: usize) -> i64 {
    line.checked_sub(1).map_or(-1, |line| line as i64)
}

fn zero_based_column(column: usize) -> i64 {
    column.checked_sub(1).map_or(-1, |column| column as i64)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DevToolsFrameKey {
    function_name: String,
    script_id: String,
    url: String,
    line_number: i64,
    column_number: i64,
}

#[derive(Debug)]
struct DevToolsCpuProfileBuilder {
    nodes: Vec<DevToolsNodeState>,
    samples: Vec<u32>,
    time_deltas: Vec<u64>,
    next_script_id: u32,
    script_ids_by_url: HashMap<String, String>,
    total_gas: u64,
}

impl DevToolsCpuProfileBuilder {
    fn new() -> Self {
        let root = DevToolsNodeState {
            id: 1,
            frame: DevToolsFrameKey {
                function_name: "(root)".to_string(),
                script_id: "0".to_string(),
                url: String::new(),
                line_number: -1,
                column_number: -1,
            },
            hit_count: 0,
            children: Vec::new(),
            child_lookup: HashMap::new(),
        };

        Self {
            nodes: vec![root],
            samples: Vec::new(),
            time_deltas: Vec::new(),
            next_script_id: 1,
            script_ids_by_url: HashMap::new(),
            total_gas: 0,
        }
    }

    fn add_sample(&mut self, frames: &[ProfileFrameSpec], gas: u64) {
        let mut current_id = 1u32;

        for frame in frames {
            let frame_key = self.materialize_frame_key(frame);
            current_id = self.ensure_child(current_id, frame_key);
        }

        let leaf_idx = current_id as usize - 1;
        self.nodes[leaf_idx].hit_count += 1;
        self.samples.push(current_id);
        self.time_deltas.push(gas);
        self.total_gas += gas;
    }

    fn materialize_frame_key(&mut self, frame: &ProfileFrameSpec) -> DevToolsFrameKey {
        let script_id = if frame.url.is_empty() {
            "0".to_string()
        } else if let Some(script_id) = self.script_ids_by_url.get(&frame.url) {
            script_id.clone()
        } else {
            let script_id = self.next_script_id.to_string();
            self.next_script_id += 1;
            self.script_ids_by_url
                .insert(frame.url.clone(), script_id.clone());
            script_id
        };

        DevToolsFrameKey {
            function_name: frame.function_name.clone(),
            script_id,
            url: frame.url.clone(),
            line_number: frame.line_number,
            column_number: frame.column_number,
        }
    }

    fn ensure_child(&mut self, parent_id: u32, frame: DevToolsFrameKey) -> u32 {
        let parent_idx = parent_id as usize - 1;
        if let Some(existing) = self.nodes[parent_idx].child_lookup.get(&frame).copied() {
            return existing;
        }

        let child_id = self.nodes.len() as u32 + 1;
        self.nodes[parent_idx].children.push(child_id);
        self.nodes[parent_idx]
            .child_lookup
            .insert(frame.clone(), child_id);
        self.nodes.push(DevToolsNodeState {
            id: child_id,
            frame,
            hit_count: 0,
            children: Vec::new(),
            child_lookup: HashMap::new(),
        });
        child_id
    }

    fn finish(self) -> DevToolsCpuProfile {
        DevToolsCpuProfile {
            nodes: self
                .nodes
                .into_iter()
                .map(|node| DevToolsCpuProfileNode {
                    id: node.id,
                    call_frame: DevToolsCpuProfileCallFrame {
                        function_name: node.frame.function_name,
                        script_id: node.frame.script_id,
                        url: node.frame.url,
                        line_number: node.frame.line_number,
                        column_number: node.frame.column_number,
                    },
                    hit_count: node.hit_count,
                    children: node.children,
                })
                .collect(),
            start_time: 0,
            end_time: self.total_gas,
            samples: self.samples,
            time_deltas: self.time_deltas,
        }
    }
}

#[derive(Debug)]
struct DevToolsNodeState {
    id: u32,
    frame: DevToolsFrameKey,
    hit_count: u64,
    children: Vec<u32>,
    child_lookup: HashMap<DevToolsFrameKey, u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevToolsCpuProfile {
    nodes: Vec<DevToolsCpuProfileNode>,
    start_time: u64,
    end_time: u64,
    samples: Vec<u32>,
    time_deltas: Vec<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevToolsCpuProfileNode {
    id: u32,
    call_frame: DevToolsCpuProfileCallFrame,
    hit_count: u64,
    children: Vec<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevToolsCpuProfileCallFrame {
    function_name: String,
    script_id: String,
    url: String,
    line_number: i64,
    column_number: i64,
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
