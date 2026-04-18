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
    pub branch_sites: BTreeMap<BranchSiteId, BranchSiteCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BranchSiteId {
    pub cell_hash: String,
    pub offset: i32,
}

#[derive(Debug, Clone)]
pub(super) struct BranchSiteCoverage {
    pub line: i64,
    pub hits: BranchHits,
}

pub(super) fn collect_coverage(
    emulations: &EmulationsState,
    build_cache: &BuildCache,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
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
    let executable_lines_per_file =
        build_executable_lines_per_files(&data, wrapper_roots, include_wrappers, include_tests);
    // Having source-level replay over VM logs, we can collect visited lines and branch hits.
    let result =
        collect_executed_lines_per_files(&data, wrapper_roots, include_wrappers, include_tests);
    let (line_hits_per_file, branch_sites_per_file) = (result.lines, result.branches);

    // Now having all this information, we can trivially determine how many executable
    // lines are in a file, how many of them were actually executed, thereby collecting the coverage we need.
    let mut files: Vec<FileCoverage> = vec![];

    for (file, executable_lines) in executable_lines_per_file {
        let executable_lines_count = executable_lines.len();
        let line_hits = line_hits_per_file.get(&file).cloned().unwrap_or_default();
        let branch_sites = branch_sites_per_file
            .get(&file)
            .cloned()
            .unwrap_or_default();

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
            branch_sites,
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
    pub condition_true: u64,
    pub condition_false: u64,
    pub guard_throw: u64,
    pub guard_continue: u64,
}

pub(super) struct ExecutedLinesForFile {
    pub lines: HashMap<String, BTreeMap<i64, u64>>,
    pub branches: HashMap<String, BTreeMap<BranchSiteId, BranchSiteCoverage>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchInstructionKind {
    Condition,
    Throw { throw_on_true: bool },
}

/// Collects all source code lines and branches that were executed in all execution traces
/// that was collected in [`collect_source_data`].
fn collect_executed_lines_per_files(
    data: &[SourceMapAndLogs],
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
) -> ExecutedLinesForFile {
    let mut line_hits_per_file: HashMap<String, BTreeMap<i64, u64>> = HashMap::new();
    let mut branch_sites_per_file: HashMap<String, BTreeMap<BranchSiteId, BranchSiteCoverage>> =
        HashMap::new();

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
        let mut last_coverage_loc: Option<(String, i64)> = None;

        while !replayer.is_finished() {
            replayer.step_with_callback(StepMode::StepInto, |tick, replayer| match tick {
                Tick::Loc { .. } => {
                    if let Some(loc) = record_current_line_hit(
                        &mut line_hits_per_file,
                        &mut last_recorded_loc,
                        replayer,
                        wrapper_roots,
                        include_wrappers,
                        include_tests,
                    ) {
                        last_coverage_loc = Some(loc);
                    }
                }
                Tick::TvmStackValues { values } => {
                    last_stack_values = values.clone();
                }
                Tick::TvmAfterExecute { instr_name } => {
                    process_branch_instruction(
                        &mut branch_sites_per_file,
                        replayer,
                        &last_stack_values,
                        &last_coverage_loc,
                        instr_name,
                        wrapper_roots,
                        include_wrappers,
                        include_tests,
                    );
                }
                _ => {}
            });
        }
    }

    ExecutedLinesForFile {
        lines: line_hits_per_file,
        branches: branch_sites_per_file,
    }
}

fn record_current_line_hit(
    line_hits_per_file: &mut HashMap<String, BTreeMap<i64, u64>>,
    last_recorded_loc: &mut Option<(String, i64)>,
    replayer: &TolkReplayer,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
) -> Option<(String, i64)> {
    let (file, line) =
        current_coverage_loc(replayer, wrapper_roots, include_wrappers, include_tests)?;

    if last_recorded_loc.as_ref() == Some(&(file.clone(), line)) {
        return Some((file, line));
    }

    *last_recorded_loc = Some((file.clone(), line));

    let entry = line_hits_per_file.entry(file.clone()).or_default();
    *entry.entry(line).or_insert(0) += 1;

    Some((file, line))
}

fn current_coverage_loc(
    replayer: &TolkReplayer,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
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

    if is_ignored_coverage_file(&file, wrapper_roots, include_wrappers, include_tests) {
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

#[allow(clippy::too_many_arguments)]
fn process_branch_instruction(
    branch_sites_per_file: &mut HashMap<String, BTreeMap<BranchSiteId, BranchSiteCoverage>>,
    replayer: &TolkReplayer,
    stack_values: &[VmStackValue],
    last_coverage_loc: &Option<(String, i64)>,
    instr_name: &str,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
) {
    let Some(kind) = classify_branch_instruction(instr_name) else {
        return;
    };
    let Some((file, line)) = branch_coverage_loc(
        replayer,
        wrapper_roots,
        last_coverage_loc,
        instr_name,
        include_wrappers,
        include_tests,
    ) else {
        return;
    };
    let Some(site_id) = current_branch_site_id(replayer) else {
        return;
    };

    let Some(condition_is_true) = stack_condition_is_true(instr_name, stack_values) else {
        return;
    };

    let entry = branch_sites_per_file.entry(file).or_default();
    let entry = entry.entry(site_id).or_insert_with(|| BranchSiteCoverage {
        line,
        hits: BranchHits::default(),
    });
    let hits = &mut entry.hits;

    match kind {
        BranchInstructionKind::Condition => {
            if condition_is_true {
                hits.condition_true += 1;
            } else {
                hits.condition_false += 1;
            }
        }
        BranchInstructionKind::Throw { throw_on_true } => {
            if condition_is_true == throw_on_true {
                hits.guard_throw += 1;
            } else {
                hits.guard_continue += 1;
            }
        }
    }
}

fn build_executable_lines_per_files(
    data: &[SourceMapAndLogs],
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
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

        build_executable_lines_per_file(
            &mut executable_lines_per_file,
            source_map,
            wrapper_roots,
            include_wrappers,
            include_tests,
        );
    }

    executable_lines_per_file
}

fn build_executable_lines_per_file(
    executable_lines_per_file: &mut HashMap<String, BTreeSet<i64>>,
    source_map: &TolkSourceMap,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
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
            _ => None,
        }) else {
            continue;
        };
        let Some((file, line)) = coverage_location_for_range(source_map, range) else {
            continue;
        };

        if is_ignored_coverage_file(&file, wrapper_roots, include_wrappers, include_tests) {
            continue;
        }

        executable_lines_per_file
            .entry(file)
            .or_default()
            .insert(line);
    }
}

fn classify_branch_instruction(instr_name: &str) -> Option<BranchInstructionKind> {
    match instruction_opcode(instr_name) {
        "THROWANYIFNOT" | "THROWIFNOT" | "THROWIFNOT_SHORT" => Some(BranchInstructionKind::Throw {
            throw_on_true: false,
        }),
        "THROWANYIF" | "THROWIF" | "THROWIF_SHORT" => Some(BranchInstructionKind::Throw {
            throw_on_true: true,
        }),
        "IF" | "IFNOT" | "IFJMP" | "IFNOTJMP" | "IFRET" | "IFNOTRET" | "IFELSE" | "IFREF"
        | "IFNOTREF" | "IFJMPREF" | "IFNOTJMPREF" | "IFREFELSE" | "IFELSEREF" | "IFREFELSEREF"
        | "CONDSEL" | "CONDSELCHK" => Some(BranchInstructionKind::Condition),
        _ => None,
    }
}

fn instruction_opcode(instr_name: &str) -> &str {
    instr_name
        .split_whitespace()
        .find_map(|token| {
            let start = token.find(|ch: char| ch.is_ascii_uppercase() || ch == '_')?;
            let token = &token[start..];
            let end = token
                .find(|ch: char| !(ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_'))
                .unwrap_or(token.len());
            let opcode = &token[..end];
            (!opcode.is_empty()).then_some(opcode)
        })
        .unwrap_or("")
}

fn stack_condition_is_true(instr_name: &str, stack_values: &[VmStackValue]) -> Option<bool> {
    let opcode = instruction_opcode(instr_name);
    let value = match opcode {
        // `CONDSEL` uses the top stack triple `(cond, when_true, when_false)`, so in VM dumps the
        // condition is the third value from the end rather than "the first integer anywhere".
        "CONDSEL" | "CONDSELCHK" => {
            stack_values
                .iter()
                .nth_back(2)
                .and_then(|value| match value {
                    VmStackValue::Integer(value) => Some(value.as_str()),
                    _ => None,
                })?
        }
        _ => stack_values.iter().rev().find_map(|value| match value {
            VmStackValue::Integer(value) => Some(value.as_str()),
            _ => None,
        })?,
    };
    Some(value != "0")
}

fn branch_coverage_loc(
    replayer: &TolkReplayer,
    wrapper_roots: &[PathBuf],
    last_coverage_loc: &Option<(String, i64)>,
    instr_name: &str,
    include_wrappers: bool,
    include_tests: bool,
) -> Option<(String, i64)> {
    if let Some(loc) =
        current_coverage_loc(replayer, wrapper_roots, include_wrappers, include_tests)
    {
        return Some(loc);
    }

    if should_fallback_to_last_coverage_loc(instruction_opcode(instr_name)) {
        last_coverage_loc.clone()
    } else {
        None
    }
}

fn should_fallback_to_last_coverage_loc(opcode: &str) -> bool {
    matches!(
        opcode,
        "IFJMP"
            | "IFNOTJMP"
            | "IFJMPREF"
            | "IFNOTJMPREF"
            | "IFELSE"
            | "IFREFELSE"
            | "IFELSEREF"
            | "IFREFELSEREF"
    )
}

fn current_branch_site_id(replayer: &TolkReplayer) -> Option<BranchSiteId> {
    let (cell_hash, offset) = replayer.current_vm_position()?;
    Some(BranchSiteId {
        cell_hash: cell_hash.to_owned(),
        offset,
    })
}

fn is_ignored_coverage_file(
    file: &str,
    wrapper_roots: &[PathBuf],
    include_wrappers: bool,
    include_tests: bool,
) -> bool {
    let path = Path::new(file);
    let is_wrapper_file = wrapper_roots.iter().any(|root| path.starts_with(root))
        || path
            .components()
            .any(|component| component.as_os_str() == "wrappers");

    file.is_empty()
        || file.contains("@stdlib/")
        || file.contains("/lib/")
        || file.contains("/.acton/")
        || (!include_tests && file.contains(".test.tolk"))
        || (!include_wrappers && is_wrapper_file)
}

const fn zero_based_line(line: usize) -> i64 {
    line.saturating_sub(1) as i64
}

#[derive(Debug, Clone, Copy, Default)]
struct CoverageStats {
    lines_found: usize,
    lines_hit: usize,
    branches_found: usize,
    branches_hit: usize,
}

impl CoverageStats {
    fn line_percentage(self) -> f64 {
        coverage_percentage_or_zero(self.lines_hit, self.lines_found)
    }

    fn branch_percentage(self) -> Option<f64> {
        coverage_percentage(self.branches_hit, self.branches_found)
    }

    fn combined_score(self) -> f64 {
        coverage_percentage_or_zero(
            self.lines_hit + self.branches_hit,
            self.lines_found + self.branches_found,
        )
    }
}

fn coverage_percentage(covered: usize, total: usize) -> Option<f64> {
    (total > 0).then(|| covered as f64 / total as f64 * 100.0)
}

fn coverage_percentage_or_zero(covered: usize, total: usize) -> f64 {
    coverage_percentage(covered, total).unwrap_or(0.0)
}

fn branch_coverage_totals(hits: &BranchHits) -> (usize, usize) {
    let mut branches_found = 0usize;
    let mut branches_hit = 0usize;

    if hits.has_condition_branch() {
        branches_found += 2;
        branches_hit +=
            usize::from(hits.condition_true > 0) + usize::from(hits.condition_false > 0);
    }

    if hits.has_guard_branch() {
        branches_found += 2;
        branches_hit += usize::from(hits.guard_throw > 0) + usize::from(hits.guard_continue > 0);
    }

    (branches_found, branches_hit)
}

fn file_coverage_stats(file_coverage: &FileCoverage) -> CoverageStats {
    let mut stats = CoverageStats {
        lines_found: file_coverage.executable_lines_count,
        lines_hit: file_coverage.covered_lines_count,
        ..CoverageStats::default()
    };

    for branch_site in file_coverage.branch_sites.values() {
        let (branches_found, branches_hit) = branch_coverage_totals(&branch_site.hits);
        stats.branches_found += branches_found;
        stats.branches_hit += branches_hit;
    }

    stats
}

fn total_coverage_stats(coverage: &Coverage) -> CoverageStats {
    let mut stats = CoverageStats::default();

    for file_coverage in &coverage.files {
        let file_stats = file_coverage_stats(file_coverage);
        stats.lines_found += file_stats.lines_found;
        stats.lines_hit += file_stats.lines_hit;
        stats.branches_found += file_stats.branches_found;
        stats.branches_hit += file_stats.branches_hit;
    }

    stats
}

const fn coverage_color(percentage: f64) -> Color {
    match percentage as u32 {
        0..=50 => Color::DarkRed,
        51..=80 => Color::DarkYellow,
        _ => Color::DarkGreen,
    }
}

fn score_color(score: f64) -> Color {
    if score >= 85.0 {
        Color::DarkGreen
    } else if score >= 60.0 {
        Color::DarkYellow
    } else {
        Color::DarkRed
    }
}

pub(super) fn total_line_coverage_percentage(coverage: &Coverage) -> f64 {
    total_coverage_stats(coverage).line_percentage()
}

pub(super) fn total_coverage_score_percentage(coverage: &Coverage) -> f64 {
    total_coverage_stats(coverage).combined_score()
}

pub(super) fn print_coverage_summary(coverage: &Coverage) {
    if coverage.files.is_empty() {
        // Empty coverage info, likely compilation error
        return;
    }

    println!("\n{} {}\n", " COVERAGE ".bold().on_cyan(), "".dimmed());

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ──      ─       ──     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "File",
            "Covered Lines",
            "Total Lines",
            "% Lines",
            "Covered Branches",
            "Total Branches",
            "% Branches",
            "Score",
        ]);

    let total_stats = total_coverage_stats(coverage);

    if total_stats.lines_found > 0 {
        let total_line_percentage = total_stats.line_percentage();
        let total_branch_percentage = total_stats.branch_percentage();
        let total_score = total_stats.combined_score();
        let total_line_color = coverage_color(total_line_percentage);
        let total_branch_color = total_branch_percentage.map_or(Color::Grey, coverage_color);
        let total_score_color = score_color(total_score);

        table.add_row(vec![
            TableCell::new("All files")
                .set_alignment(CellAlignment::Left)
                .fg(total_score_color),
            TableCell::new(total_stats.lines_hit.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(total_line_color),
            TableCell::new(total_stats.lines_found.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(format!("{total_line_percentage:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(total_line_color),
            TableCell::new(total_stats.branches_hit.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(total_branch_color),
            TableCell::new(total_stats.branches_found.to_string())
                .set_alignment(CellAlignment::Right),
            TableCell::new(
                total_branch_percentage
                    .map_or_else(|| "n/a".to_string(), |value| format!("{value:.1}%")),
            )
            .set_alignment(CellAlignment::Right)
            .fg(total_branch_color),
            TableCell::new(format!("{total_score:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(total_score_color),
        ]);
    }

    let mut files_with_stats: Vec<(CoverageStats, &FileCoverage)> = coverage
        .files
        .iter()
        .map(|file_coverage| (file_coverage_stats(file_coverage), file_coverage))
        .collect();

    files_with_stats.sort_by(|(left_stats, left_file), (right_stats, right_file)| {
        left_stats
            .combined_score()
            .total_cmp(&right_stats.combined_score())
            .then_with(|| {
                right_file
                    .executable_lines_count
                    .cmp(&left_file.executable_lines_count)
            })
            .then_with(|| left_file.file.cmp(&right_file.file))
    });

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    for (stats, file_coverage) in files_with_stats {
        let relative_path = Path::new(&file_coverage.file)
            .strip_prefix(&cwd)
            .unwrap_or_else(|_| Path::new(&file_coverage.file))
            .display()
            .to_string();

        let line_percentage = stats.line_percentage();
        let branch_percentage = stats.branch_percentage();
        let combined_score = stats.combined_score();
        let line_color = coverage_color(line_percentage);
        let branch_color = branch_percentage.map_or(Color::Grey, coverage_color);
        let combined_score_color = score_color(combined_score);

        table.add_row(vec![
            TableCell::new(relative_path)
                .set_alignment(CellAlignment::Left)
                .fg(combined_score_color),
            TableCell::new(file_coverage.covered_lines_count.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(line_color),
            TableCell::new(file_coverage.executable_lines_count.to_string())
                .set_alignment(CellAlignment::Right),
            TableCell::new(format!("{line_percentage:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(line_color),
            TableCell::new(stats.branches_hit.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(branch_color),
            TableCell::new(stats.branches_found.to_string()).set_alignment(CellAlignment::Right),
            TableCell::new(
                branch_percentage.map_or_else(|| "n/a".to_string(), |value| format!("{value:.1}%")),
            )
            .set_alignment(CellAlignment::Right)
            .fg(branch_color),
            TableCell::new(format!("{combined_score:.1}%"))
                .set_alignment(CellAlignment::Right)
                .fg(combined_score_color),
        ]);
    }

    println!("{table}");
}

pub(super) fn generate_lcov_report(coverage: &Coverage) -> String {
    let mut lcov_content = String::new();

    for file_coverage in &coverage.files {
        if file_coverage.executable_lines_count == 0 {
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

        if !file_coverage.branch_sites.is_empty() {
            let mut branch_sites_by_line: BTreeMap<i64, Vec<(&BranchSiteId, &BranchSiteCoverage)>> =
                BTreeMap::new();
            for (site_id, site) in &file_coverage.branch_sites {
                branch_sites_by_line
                    .entry(site.line)
                    .or_default()
                    .push((site_id, site));
            }

            let mut branch_idx = 0usize;
            let mut branches_found = 0u64;
            let mut branches_hit = 0u64;
            for (line, mut sites) in branch_sites_by_line {
                sites.sort_by(compare_branch_sites);
                let line = line + 1;
                for (_, site) in sites {
                    let info = &site.hits;
                    if info.has_condition_branch() {
                        lcov_content.push_str(&format!(
                            "BRDA:{line},{branch_idx},0,{}\n",
                            info.condition_true
                        ));
                        lcov_content.push_str(&format!(
                            "BRDA:{line},{branch_idx},1,{}\n",
                            info.condition_false
                        ));
                        branches_found += 2;
                        branches_hit += u64::from(info.condition_true > 0)
                            + u64::from(info.condition_false > 0);
                        branch_idx += 1;
                    }
                    if info.has_guard_branch() {
                        lcov_content.push_str(&format!(
                            "BRDA:{line},{branch_idx},0,{}\n",
                            info.guard_throw
                        ));
                        lcov_content.push_str(&format!(
                            "BRDA:{line},{branch_idx},1,{}\n",
                            info.guard_continue
                        ));
                        branches_found += 2;
                        branches_hit +=
                            u64::from(info.guard_throw > 0) + u64::from(info.guard_continue > 0);
                        branch_idx += 1;
                    }
                }
            }

            // BRF: branches found, BRH: branches hit
            lcov_content.push_str(&format!("BRF:{branches_found}\n"));
            lcov_content.push_str(&format!("BRH:{branches_hit}\n"));
        }

        lcov_content.push_str("end_of_record\n");
    }

    lcov_content
}

pub(super) fn generate_lcov_file(
    coverage: &Coverage,
    output_path: &str,
) -> Result<(), std::io::Error> {
    ensure_output_parent_dir(output_path)?;
    fs::write(output_path, generate_lcov_report(coverage))
}

pub(super) fn generate_text_file(
    coverage: &Coverage,
    output_path: &str,
) -> Result<(), std::io::Error> {
    ensure_output_parent_dir(output_path)?;
    let text_content = generate_text_report(coverage);
    fs::write(output_path, text_content)
}

fn ensure_output_parent_dir(output_path: &str) -> Result<(), std::io::Error> {
    let path = Path::new(output_path);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn generate_text_report(coverage: &Coverage) -> String {
    let mut result = String::new();

    let total_lines: usize = coverage
        .files
        .iter()
        .map(|f| f.executable_lines_count)
        .sum();
    let covered_lines: usize = coverage.files.iter().map(|f| f.covered_lines_count).sum();
    let coverage_percentage = total_line_coverage_percentage(coverage);

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
        if file_coverage.executable_lines_count == 0 {
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
    let sites: Vec<_> = file_coverage
        .branch_sites
        .iter()
        .filter(|(_, site)| site.line == line)
        .collect();
    if sites.is_empty() {
        return String::new();
    }
    let mut sites = sites;
    sites.sort_by(compare_branch_sites);

    if sites.len() == 1 {
        let Some(text) = format_branch_hits(&sites[0].1.hits) else {
            return String::new();
        };
        return format!(" branches:{text}");
    }

    let mut parts = Vec::new();
    for (site_idx, (_, site)) in sites.into_iter().enumerate() {
        let Some(text) = format_branch_hits(&site.hits) else {
            continue;
        };
        parts.push(format!("site{site_idx} {text}"));
    }

    if parts.is_empty() {
        return String::new();
    }

    format!(" branches:{}", parts.join("; "))
}

fn format_branch_hits(info: &BranchHits) -> Option<String> {
    let mut parts = Vec::new();

    if info.has_condition_branch() {
        parts.push(format!(
            "true={} false={}",
            info.condition_true, info.condition_false
        ));
    }

    if info.has_guard_branch() {
        parts.push(format!(
            "throw={} continue={}",
            info.guard_throw, info.guard_continue
        ));
    }

    if parts.is_empty() {
        return None;
    }

    Some(parts.join(" "))
}

fn compare_branch_sites(
    left: &(&BranchSiteId, &BranchSiteCoverage),
    right: &(&BranchSiteId, &BranchSiteCoverage),
) -> Ordering {
    let left_hits = &left.1.hits;
    let right_hits = &right.1.hits;

    (
        left_hits.condition_true,
        left_hits.condition_false,
        left_hits.guard_throw,
        left_hits.guard_continue,
        left.0.offset,
        left.0.cell_hash.as_str(),
    )
        .cmp(&(
            right_hits.condition_true,
            right_hits.condition_false,
            right_hits.guard_throw,
            right_hits.guard_continue,
            right.0.offset,
            right.0.cell_hash.as_str(),
        ))
}

impl BranchHits {
    const fn has_condition_branch(&self) -> bool {
        self.condition_true > 0 || self.condition_false > 0
    }

    const fn has_guard_branch(&self) -> bool {
        self.guard_throw > 0 || self.guard_continue > 0
    }
}
