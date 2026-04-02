use crate::context::{BuildCache, EmulationsState};
use acton_config::color::OwoColorize;
use acton_debug::replayer::{StepMode, Tick, TolkReplayer};
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolkc::{
    TolkSourceMap,
    source_map::{DebugMark, SrcRange},
};
use tycho_types::boc::Boc;
use vmlogs::parser::VmStackValue;

#[derive(Debug, Clone)]
pub(super) struct Coverage {
    pub files: Vec<FileCoverage>,
}

#[derive(Debug, Clone)]
pub(super) struct FileCoverage {
    pub file: String,
    pub covered_lines_count: usize,
    pub line_hits: BTreeMap<i64, u64>, // line number -> hit count
    pub executable_lines_count: usize,
    pub executable_lines: BTreeSet<i64>, // all executable line numbers
    pub branch_hits: HashMap<i64, BranchHits>,
}

pub(super) fn collect_coverage(
    emulations: &EmulationsState,
    build_cache: &BuildCache,
    wrapper_roots: &[PathBuf],
) -> Coverage {
    // To build coverage we need two things: source maps and virtual machine logs.
    //
    // The first provides us with the necessary information about which lines in the source code are
    // executable and can be covered.
    //
    // The second provides us with the executed VM log stream. Instead of mapping raw
    // `(cell_hash, offset)` pairs back to source lines manually, we replay those logs through
    // `TolkReplayer`, which already contains the source-level debug reconstruction logic used by
    // debugger flows. That lets coverage consume the exact lines the replayer visited.
    let data = collect_source_data(emulations, build_cache);
    // Not all lines of code in source code can be executed, for example, struct definitions
    // or comments. We collect executable lines from the same stoppable debug marks the
    // replayer relies on, so the denominator matches the source-level lines we can actually hit.
    let executable_lines_per_file = build_executable_lines_per_files(&data, wrapper_roots);
    // Having source-level replay over VM logs, we can collect visited lines and branch hits.
    let result = collect_executed_lines_per_files(&data, wrapper_roots);
    let (line_hits_per_file, branch_hits_per_file) = (result.lines, result.branches);

    // Now having all this information, we can trivially determine how many executable
    // lines are in a file, how many of them were actually executed, thereby collecting the coverage we need.
    let mut files: Vec<FileCoverage> = vec![];

    for (file, executable_lines) in executable_lines_per_file {
        let executable_lines_count = executable_lines.len();
        let line_hits = line_hits_per_file.get(&file).cloned().unwrap_or_default();
        let branch_hits = branch_hits_per_file.get(&file).cloned().unwrap_or_default();

        let mut covered_lines_count = 0;

        for line in &executable_lines {
            let Some(line_hits) = line_hits.get(line) else {
                continue;
            };
            if line_hits > &0 {
                covered_lines_count += 1;
            }
        }

        files.push(FileCoverage {
            file: file.clone(),
            executable_lines_count,
            covered_lines_count,
            line_hits,
            executable_lines,
            branch_hits,
        });
    }

    files.sort_by(|left, right| left.file.cmp(&right.file));

    Coverage { files }
}

struct SourceMapAndLogs {
    build_path: PathBuf,
    source_map: Arc<TolkSourceMap>,
    logs: Arc<str>,
}

/// Collects all source maps and logs that will then be used for coverage calculation.
fn collect_source_data(
    emulations: &EmulationsState,
    build_cache: &BuildCache,
) -> Vec<SourceMapAndLogs> {
    let mut data: Vec<SourceMapAndLogs> = vec![];
    for message in emulations.messages() {
        let Some(build_result) = build_cache.result_for_code(&message.code) else {
            continue;
        };

        let build_path = build_result.0;
        let source_map = build_result.1.source_map;
        let logs = message.vm_log.clone();

        data.push(SourceMapAndLogs {
            build_path,
            source_map,
            logs,
        });
    }

    for get_result in emulations.get_methods() {
        let Ok(code) = Boc::decode_base64(get_result.code.as_ref()) else {
            continue;
        };
        let Some(build_result) = build_cache.result_for_code(&Some(code)) else {
            continue;
        };

        let build_path = build_result.0;
        let source_map = build_result.1.source_map;
        let logs = get_result.vm_log.clone();

        data.push(SourceMapAndLogs {
            build_path,
            source_map,
            logs,
        });
    }
    data
}

#[derive(Debug, Clone, Default)]
pub(super) struct BranchHits {
    pub if_true: u64,
    pub if_false: u64,
}

pub(super) struct ExecutedLinesForFile {
    pub lines: HashMap<String, BTreeMap<i64, u64>>,
    pub branches: HashMap<String, HashMap<i64, BranchHits>>,
}

/// Collects all source code lines and branches that were executed in all execution traces
/// that was collected in [`collect_source_data`].
fn collect_executed_lines_per_files(
    data: &[SourceMapAndLogs],
    wrapper_roots: &[PathBuf],
) -> ExecutedLinesForFile {
    let mut line_hits_per_file: HashMap<String, BTreeMap<i64, u64>> = HashMap::new();
    let mut branch_hits_per_file: HashMap<String, HashMap<i64, BranchHits>> = HashMap::new();

    for SourceMapAndLogs {
        source_map, logs, ..
    } in data
    {
        let vm_lines = vmlogs::parser::parse_lines(logs);
        let Ok(mut replayer) = TolkReplayer::new(source_map, &vm_lines) else {
            continue;
        };
        let mut last_stack_values = Vec::new();
        let mut last_recorded_loc: Option<(String, i64)> = None;

        while !replayer.is_finished() {
            replayer.step_with_callback(StepMode::StepInto, |tick, replayer| match tick {
                Tick::Loc { .. } | Tick::AtFunReturn { .. } => {
                    record_current_line_hit(
                        &mut line_hits_per_file,
                        &mut last_recorded_loc,
                        replayer,
                        wrapper_roots,
                    );
                }
                Tick::TvmStackValues { values } => {
                    last_stack_values = values.clone();
                }
                Tick::TvmAfterExecute { instr_name } if is_throw_branch_instruction(instr_name) => {
                    process_throw_instruction(
                        &mut branch_hits_per_file,
                        replayer,
                        &last_stack_values,
                        wrapper_roots,
                    );
                }
                _ => {}
            });
        }
    }

    ExecutedLinesForFile {
        lines: line_hits_per_file,
        branches: branch_hits_per_file,
    }
}

fn record_current_line_hit(
    line_hits_per_file: &mut HashMap<String, BTreeMap<i64, u64>>,
    last_recorded_loc: &mut Option<(String, i64)>,
    replayer: &TolkReplayer,
    wrapper_roots: &[PathBuf],
) {
    let Some((file, line)) = current_coverage_loc(replayer, wrapper_roots) else {
        return;
    };

    if last_recorded_loc.as_ref() == Some(&(file.clone(), line)) {
        return;
    }

    *last_recorded_loc = Some((file.clone(), line));

    let entry = line_hits_per_file.entry(file).or_default();
    *entry.entry(line).or_insert(0) += 1;
}

fn current_coverage_loc(
    replayer: &TolkReplayer,
    wrapper_roots: &[PathBuf],
) -> Option<(String, i64)> {
    let line = replayer.current_line();
    if line == 0 {
        return None;
    }

    let file_id = replayer.current_file_id();
    let file = replayer
        .file_full_path(file_id)
        .unwrap_or_else(|| replayer.current_file_name())
        .to_owned();

    if is_ignored_coverage_file(&file, wrapper_roots) {
        return None;
    }

    Some((file, zero_based_line(line)))
}

fn coverage_location_for_range(
    source_map: &tolkc::SourceMap,
    range: &SrcRange,
) -> Option<(String, i64)> {
    let file = source_map
        .resolve_file_full_path(range.file_id())
        .unwrap_or_else(|| source_map.resolve_file_name(range.file_id()))
        .to_owned();

    Some((file, zero_based_line(range.start_line())))
}

fn process_throw_instruction(
    branch_hits_per_file: &mut HashMap<String, HashMap<i64, BranchHits>>,
    replayer: &TolkReplayer,
    stack_values: &[VmStackValue],
    wrapper_roots: &[PathBuf],
) {
    let Some((file, line)) = current_coverage_loc(replayer, wrapper_roots) else {
        return;
    };

    if let [.., VmStackValue::Integer(value)] = stack_values {
        let taken = value == "0";
        let entry = branch_hits_per_file.entry(file).or_default();
        let entry = entry.entry(line).or_default();

        if taken {
            entry.if_true += 1;
        } else {
            entry.if_false += 1;
        }
    }
}

fn build_executable_lines_per_files(
    data: &[SourceMapAndLogs],
    wrapper_roots: &[PathBuf],
) -> HashMap<String, BTreeSet<i64>> {
    let mut seen_source_maps = HashSet::new();
    let mut executable_lines_per_file: HashMap<String, BTreeSet<i64>> = HashMap::new();

    for SourceMapAndLogs {
        build_path,
        source_map,
        ..
    } in data
    {
        if !seen_source_maps.insert(build_path.clone()) {
            continue;
        }

        build_executable_lines_per_file(&mut executable_lines_per_file, source_map, wrapper_roots);
    }

    executable_lines_per_file
}

fn build_executable_lines_per_file(
    executable_lines_per_file: &mut HashMap<String, BTreeSet<i64>>,
    source_map: &TolkSourceMap,
    wrapper_roots: &[PathBuf],
) {
    let source_map = &source_map.source_map;

    for mark_id in 0..source_map.debug_marks_count() {
        let Some(range) = (match source_map.get_debug_mark(mark_id) {
            DebugMark::Loc { range, .. } => Some(range),
            DebugMark::EnterFun {
                range,
                is_inlined: true,
                ..
            } => Some(range),
            DebugMark::LeaveFun { range, .. } => Some(range),
            _ => None,
        }) else {
            continue;
        };
        let Some((file, line)) = coverage_location_for_range(source_map, range) else {
            continue;
        };

        if is_ignored_coverage_file(&file, wrapper_roots) {
            continue;
        }

        executable_lines_per_file
            .entry(file)
            .or_default()
            .insert(line);
    }
}

fn is_throw_branch_instruction(instr_name: &str) -> bool {
    instr_name.contains("THROWANYIFNOT")
        || instr_name.contains("THROWIFNOT")
        || instr_name.contains("THROWIFNOT_SHORT")
}

fn is_ignored_coverage_file(file: &str, wrapper_roots: &[PathBuf]) -> bool {
    let path = Path::new(file);

    file.is_empty()
        || file.contains("@stdlib/")
        || file.contains("/lib/")
        || file.contains("/.acton/")
        || file.contains(".test.tolk")
        || wrapper_roots.iter().any(|root| path.starts_with(root))
        || path
            .components()
            .any(|component| component.as_os_str() == "wrappers")
}

const fn zero_based_line(line: usize) -> i64 {
    line.saturating_sub(1) as i64
}

#[allow(dead_code)] // maybe for command like coverage merge
pub(super) fn merge_coverages(coverages: &Vec<Coverage>) -> Coverage {
    let mut merged_files: HashMap<String, FileCoverage> = HashMap::new();

    for coverage in coverages {
        for file_coverage in &coverage.files {
            let file = &file_coverage.file;
            if let Some(existing) = merged_files.get_mut(file) {
                // If in one coverage the lines were covered as: 1, 1, 0, 1,
                //                            and in another as: 1, 1, 1, 0,
                //                        then we get as result: 2, 2, 1, 1.
                for (&line, &hits) in &file_coverage.line_hits {
                    *existing.line_hits.entry(line).or_insert(0) += hits;
                }

                // If for some reason between coverages a specific file has a different number
                // of executable lines, then we add all executable lines from the second coverage, so that
                // the executable lines in the result are the union of all executable lines.
                if file_coverage.executable_lines_count != existing.executable_lines_count {
                    for line in &file_coverage.executable_lines {
                        existing.executable_lines.insert(*line);
                    }
                    existing.executable_lines_count = existing.executable_lines.len();
                }
                existing.covered_lines_count = existing.line_hits.len();
            } else {
                merged_files.insert(file.clone(), file_coverage.clone());
            }
        }
    }

    Coverage {
        files: merged_files.into_values().collect(),
    }
}

pub(super) fn print_coverage_summary(coverage: &Coverage) {
    if coverage.files.is_empty() {
        // Empty coverage info, likely compilation error
        return;
    }

    println!("\n{} {}\n", " COVERAGE ".bold().on_cyan(), "".dimmed());

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["File", "Covered Lines", "Total Lines", "% Lines"]);

    let mut total_executable_lines = 0usize;
    let mut total_covered_lines = 0usize;

    for file_coverage in &coverage.files {
        total_executable_lines += file_coverage.executable_lines_count;
        total_covered_lines += file_coverage.covered_lines_count;
    }

    if total_executable_lines > 0 {
        let total_percentage = total_covered_lines as f64 / total_executable_lines as f64 * 100.0;
        let (total_covered_color, total_percentage_color) = match total_percentage as u32 {
            0..=50 => (Color::DarkRed, Color::DarkRed),
            51..=80 => (Color::DarkYellow, Color::DarkYellow),
            _ => (Color::DarkGreen, Color::DarkGreen),
        };

        table.add_row(vec![
            TableCell::new("All files")
                .set_alignment(CellAlignment::Left)
                .fg(total_percentage_color),
            TableCell::new(total_covered_lines.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(total_covered_color),
            TableCell::new(total_executable_lines.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(format!("{total_percentage:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(total_percentage_color),
        ]);
    }

    let mut files_with_percentage: Vec<(f64, &FileCoverage)> = coverage
        .files
        .iter()
        .map(|file_coverage| {
            let percentage = if file_coverage.executable_lines_count > 0 {
                file_coverage.covered_lines_count as f64
                    / file_coverage.executable_lines_count as f64
                    * 100.0
            } else {
                0.0
            };
            (percentage, file_coverage)
        })
        .collect();

    files_with_percentage.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    for (percentage, file_coverage) in files_with_percentage {
        let relative_path = Path::new(&file_coverage.file)
            .strip_prefix(&cwd)
            .unwrap_or_else(|_| Path::new(&file_coverage.file))
            .display()
            .to_string();

        let (covered_color, percentage_color) = match percentage as u32 {
            0..=50 => (Color::DarkRed, Color::DarkRed),
            51..=80 => (Color::DarkYellow, Color::DarkYellow),
            _ => (Color::DarkGreen, Color::DarkGreen),
        };

        table.add_row(vec![
            TableCell::new(relative_path)
                .set_alignment(CellAlignment::Left)
                .fg(percentage_color),
            TableCell::new(file_coverage.covered_lines_count.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(covered_color),
            TableCell::new(file_coverage.executable_lines_count.to_string())
                .set_alignment(CellAlignment::Right),
            TableCell::new(format!("{percentage:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(percentage_color),
        ]);
    }

    println!("{table}");
}

pub(super) fn generate_lcov_file(
    coverage: &Coverage,
    output_path: &str,
) -> Result<(), std::io::Error> {
    let mut lcov_content = String::new();

    for file_coverage in &coverage.files {
        if file_coverage.line_hits.is_empty() {
            continue;
        }

        // SF: source file
        lcov_content.push_str(&format!("SF:{}\n", file_coverage.file));

        // DA: line data (line number, execution count)
        for &line_number in &file_coverage.executable_lines {
            let hit_count = file_coverage
                .line_hits
                .get(&line_number)
                .copied()
                .unwrap_or(0);
            lcov_content.push_str(&format!("DA:{},{}\n", line_number + 1, hit_count));
        }

        // LF: lines found (total executable lines)
        lcov_content.push_str(&format!("LF:{}\n", file_coverage.executable_lines_count));

        // LH: lines hit (covered lines)
        lcov_content.push_str(&format!("LH:{}\n", file_coverage.covered_lines_count));

        if !file_coverage.branch_hits.is_empty() {
            let mut branch_lines: Vec<_> = file_coverage.branch_hits.iter().collect();
            branch_lines.sort_by_key(|(line, _)| **line);

            for (idx, (line, info)) in branch_lines.into_iter().enumerate() {
                let line = line + 1;
                lcov_content.push_str(&format!("BRDA:{line},{idx},0,{}\n", info.if_true));
                lcov_content.push_str(&format!("BRDA:{line},{idx},1,{}\n", info.if_false));
            }
        }

        lcov_content.push_str("end_of_record\n");
    }

    fs::write(output_path, lcov_content)
}

pub(super) fn generate_text_file(
    coverage: &Coverage,
    output_path: &str,
) -> Result<(), std::io::Error> {
    let text_content = generate_text_report(coverage);
    fs::write(output_path, text_content)
}

fn generate_text_report(coverage: &Coverage) -> String {
    let mut result = String::new();

    let total_lines: usize = coverage
        .files
        .iter()
        .map(|f| f.executable_lines_count)
        .sum();
    let covered_lines: usize = coverage.files.iter().map(|f| f.covered_lines_count).sum();
    let coverage_percentage = if total_lines > 0 {
        (covered_lines as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    result.push_str("Coverage Summary:\n");
    result.push_str(&format!(
        "Lines: {covered_lines}/{total_lines} ({coverage_percentage:.2}%)\n"
    ));

    let mut total_hits = 0u64;
    for file_coverage in &coverage.files {
        for &hits in file_coverage.line_hits.values() {
            total_hits += hits;
        }
    }

    result.push_str(&format!("Total Hits: {total_hits}\n"));
    result.push('\n');

    for file_coverage in &coverage.files {
        if file_coverage.line_hits.is_empty() {
            continue;
        }

        result.push_str(&format!("File: {}\n", file_coverage.file));

        if let Ok(source_content) = fs::read_to_string(&file_coverage.file) {
            let lines: Vec<&str> = source_content.lines().collect();
            let max_line_number_width = lines.len().to_string().len();

            let max_line_length = lines.iter().map(|line| line.len()).max().unwrap_or(0);
            let code_width = (max_line_length + 10).min(100); // Add some padding, max 100

            result.push_str("Annotated Code:\n");

            for (line_idx, line) in lines.iter().enumerate() {
                let line_number = line_idx + 1;
                let line_number_padded = format!("{line_number:>max_line_number_width$}");

                let is_executable = file_coverage.executable_lines.contains(&(line_idx as i64));

                if is_executable {
                    let hits = file_coverage
                        .line_hits
                        .get(&(line_idx as i64))
                        .copied()
                        .unwrap_or(0);
                    let status = if hits > 0 { "✓ " } else { "✗ " };
                    let hits_info = format!(
                        " hits:{hits}{}",
                        format_branch_hits_suffix(file_coverage, line_idx as i64)
                    );

                    let padding = " ".repeat(code_width.saturating_sub(line.len()));
                    result.push_str(&format!(
                        "{line_number_padded} {status}| {line}{padding}|{hits_info}\n"
                    ));
                } else {
                    let padding = " ".repeat(code_width.saturating_sub(line.len()));
                    result.push_str(&format!("{line_number_padded}   | {line}{padding}|\n"));
                }
            }
        } else {
            result.push_str("  (Could not read source file)\n");
            result.push_str(&format!(
                "  Executable lines: {}\n",
                file_coverage.executable_lines_count
            ));
            result.push_str(&format!(
                "  Covered lines: {}\n",
                file_coverage.covered_lines_count
            ));
        }

        result.push('\n');
    }

    result
}

fn format_branch_hits_suffix(file_coverage: &FileCoverage, line: i64) -> String {
    let Some(info) = file_coverage.branch_hits.get(&line) else {
        return String::new();
    };

    format!(
        " branches:throw={} continue={}",
        info.if_true, info.if_false
    )
}
