use crate::commands::test::TestRunner;
use chrono;
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use ton_abi::ContractAbi;

pub(super) fn collect_profile(runner: &TestRunner, abi: &ContractAbi) -> anyhow::Result<()> {
    let mut gas_per_opcode = HashMap::new();

    for result in runner.emulations.messages() {
        let Some(opcode) = result.opcode() else {
            continue;
        };
        let Some(msg_abi) = abi.find_type_by_opcode(opcode) else {
            continue;
        };
        let Some(used_gas) = result.used_gas() else {
            continue;
        };

        gas_per_opcode
            .entry(msg_abi.name)
            .or_insert_with(Vec::new)
            .push(used_gas);
    }

    if gas_per_opcode.is_empty() {
        return Ok(());
    }

    let mut table = Table::new();

    let baseline_snapshot = if let Some(baseline_path) = &runner.config.baseline_snapshot {
        match load_gas_snapshot(baseline_path) {
            Ok(snapshot) => {
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
                    baseline_path,
                    snapshot.opcodes.len(),
                    formatted_time
                );
                println!();

                table
                    .load_preset("  ─  ──      ─     ")
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .set_header(vec!["Opcode", "Baseline", "Current", "Diff", "% Change"]);
                Some(snapshot)
            }
            Err(err) => {
                eprintln!("Warning: Failed to load baseline gas snapshot '{baseline_path}': {err}",);
                println!("\n{} {}\n", " GAS USAGE ".bold().on_blue(), "".dimmed());
                table
                    .load_preset("  ─  ──      ─     ")
                    .set_content_arrangement(ContentArrangement::Dynamic)
                    .set_header(vec!["Opcode", "Min Gas", "Max Gas", "Avg Gas"]);
                None
            }
        }
    } else {
        println!("\n{} {}\n", " GAS USAGE ".bold().on_blue(), "".dimmed());
        table
            .load_preset("  ─  ──      ─     ")
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Opcode", "Min Gas", "Max Gas", "Avg Gas"]);
        None
    };

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

    if let Some(baseline) = &baseline_snapshot {
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
            let color = if comparison.baseline_avg == 0 {
                Color::DarkCyan
            } else if comparison.change_percent < -5.0 {
                Color::DarkGreen
            } else if comparison.change_percent > 5.0 {
                Color::DarkRed
            } else {
                Color::Grey
            };

            let diff_str = if comparison.diff > 0 {
                format!("+{}", comparison.diff)
            } else {
                comparison.diff.to_string()
            };

            let percent_str = if comparison.baseline_avg == 0 {
                "NEW".to_string()
            } else {
                format!("{:+.1}%", comparison.change_percent)
            };

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
                TableCell::new(diff_str)
                    .set_alignment(CellAlignment::Right)
                    .fg(color),
                TableCell::new(percent_str)
                    .set_alignment(CellAlignment::Right)
                    .fg(color),
            ]);
        }
    } else {
        let avg_values = opcode_gas_stats.iter().map(|(_, _, _, avg)| *avg);
        let len = avg_values.count();
        if len > 0 {
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
    }

    println!("{table}\n");

    // we don't want to override previous snapshot in compare mode
    if let Some(snapshot_filename) = &runner.config.snapshot
        && runner.config.baseline_snapshot.is_none()
    {
        let snapshot = create_gas_snapshot(&gas_per_opcode);
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(err) => {
                anyhow::bail!("Failed to create gas snapshot: {err}")
            }
        };
        if let Err(err) = save_gas_snapshot(&snapshot, snapshot_filename) {
            anyhow::bail!("Failed to save gas snapshot: {err}")
        }
    }

    Ok(())
}

fn create_gas_snapshot(gas_per_opcode: &HashMap<String, Vec<u64>>) -> anyhow::Result<GasSnapshot> {
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

    Ok(GasSnapshot { timestamp, opcodes })
}

fn save_gas_snapshot(snapshot: &GasSnapshot, filename: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(snapshot)?;
    fs::write(filename, json)?;
    println!("Gas snapshot saved to {filename}");
    Ok(())
}

fn load_gas_snapshot(filename: &str) -> anyhow::Result<GasSnapshot> {
    let content = fs::read_to_string(filename)?;
    let snapshot: GasSnapshot = serde_json::from_str(&content)?;
    Ok(snapshot)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GasSnapshot {
    pub timestamp: u64,
    pub opcodes: HashMap<String, OpcodeGasStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct OpcodeGasStats {
    pub min_gas: u64,
    pub max_gas: u64,
    pub avg_gas: u64,
    pub samples: usize,
    pub all_values: Vec<u64>,
}

#[derive(Debug)]
struct GasComparison {
    opcode: String,
    current_avg: u64,
    baseline_avg: u64,
    diff: i64,
    change_percent: f64,
}

impl GasComparison {
    fn new(opcode: String, current_avg: u64, baseline_avg: u64) -> Self {
        let diff = current_avg as i64 - baseline_avg as i64;
        let change_percent = if baseline_avg > 0 {
            (diff as f64 / baseline_avg as f64) * 100.0
        } else {
            0.0
        };

        Self {
            opcode,
            current_avg,
            baseline_avg,
            diff,
            change_percent,
        }
    }
}
