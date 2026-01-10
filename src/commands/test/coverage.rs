use crate::context::{BuildCache, EmulationsState};
use crate::vmtrace::{HighLevelTrace, HighLevelTraceStep};
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use owo_colors::OwoColorize;
use retrace::trace::Trace;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use ton_source_map::{EntryContextDescription, SourceMap};
use tycho_types::boc::Boc;

#[derive(Debug, Clone)]
pub struct Coverage {
    pub files: Vec<FileCoverage>,
}

#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub file: String,
    pub covered_lines_count: usize,
    pub line_hits: BTreeMap<i64, u64>, // line number -> hit count
    pub executable_lines_count: usize,
    pub executable_lines: BTreeSet<i64>, // all executable line numbers
}

pub fn collect_coverage(emulations: &EmulationsState, build_cache: &BuildCache) -> Coverage {
    // Для построения coverage нам нужны две вещи, карты исходников и логи виртуальной машины.
    //
    // Первое обеспечивает нам необходимую информацию о том какие строки в исходном коде являются
    // исполняемыми и для которых можно собрать покрытие, а также информацию о том как конкретные
    // локации в биткоде соотносятся со строками исходного кода.
    //
    // Второе обеспечивает нам трассу исполнения по которой мы можем выяснить какие инструкции
    // были исполнены во время исполнения тестов. Благодаря картам исходников мы можем соотнести
    // инструкцию, которая была выполнена, исходному коду который ее породил. И так как инструкция
    // оказалась в трассе исполнения, мы можем сказать что те строки исходного кода были исполнены,
    // а значит покрыты тестом.
    let data = collect_source_data(emulations, build_cache);
    // Когда у нас есть нужные компоненты мы можем построить высокоуровневую трассу исполнения
    // которая содержит все шаги исполнения отраженные на исходном коде.
    let traces = build_high_level_traces(&data);
    // Не все строки кода в исходном коде могут быть исполнены, например, определение структуры
    // или комментарии. Мы собираем исполняемые строки файлов используя тот факт, что карта исходников
    // содержит отображение каждой исполняемой строки на инструкции в биткоде, что означает что мы
    // можем собрать мапу по файлам которая укажет является ли конкретная строка исполняемой.
    let executable_lines_per_file = build_executable_lines_per_files(&data);
    // Имея высокоуровневые трейсы мы можем обойти их и собрать какие конкретно строки исходного
    // кода были выполнены. Это даст нам информацию о покрытых строках.
    let line_hits_per_file = collect_executed_lines_per_files(&traces);

    // Теперь имея всю эту информацию мы можем тривиально выяснить сколько в файле исполняемых
    // строк, сколько их них было фактически исполнено, тем самым собирая нужное нам покрытие.
    let mut files: Vec<FileCoverage> = vec![];

    for (file, executable_lines) in executable_lines_per_file {
        let executable_lines_count = executable_lines.len();
        let line_hits = line_hits_per_file.get(&file).cloned().unwrap_or_default();

        let mut covered_lines_count = 0;

        for line in &executable_lines {
            let Some(line_hits) = line_hits.get(line) else {
                continue;
            };
            if line_hits > &0 {
                covered_lines_count += 1
            }
        }

        files.push(FileCoverage {
            file: file.clone(),
            executable_lines_count,
            covered_lines_count,
            line_hits,
            executable_lines,
        })
    }

    Coverage { files }
}

struct SourceMapAndLogs<'a> {
    source_map: SourceMap,
    logs: &'a String,
}

/// Собирает все карты исходников и логи которые затем будут использоваться для подсчета покрытия.
fn collect_source_data<'a>(
    emulations: &'a EmulationsState,
    build_cache: &'a BuildCache,
) -> Vec<SourceMapAndLogs<'a>> {
    let mut data: Vec<SourceMapAndLogs> = vec![];
    for message in emulations.messages() {
        let Some(build_result) = build_cache.result_for_code(&message.code) else {
            continue;
        };

        let source_map = build_result.1.source_map;
        let logs = &message.vm_log;

        data.push(SourceMapAndLogs { source_map, logs })
    }

    for get_result in emulations.get_methods() {
        let Ok(code) = Boc::decode_base64(&get_result.code) else {
            continue;
        };
        let Some(build_result) = build_cache.result_for_code(&Some(code)) else {
            continue;
        };

        let source_map = build_result.1.source_map;
        let logs = &get_result.vm_log;

        data.push(SourceMapAndLogs { source_map, logs })
    }
    data
}

/// Строит трассы исполнения по исходному коду.
fn build_high_level_traces(data: &Vec<SourceMapAndLogs>) -> Vec<HighLevelTrace> {
    data.iter()
        .map(|SourceMapAndLogs { source_map, logs }| {
            let trace = Trace::new(logs, Some(1_000_000));
            HighLevelTrace::new(trace, source_map)
        })
        .collect::<Vec<_>>()
}

/// Собирает все строки исходного кода которые были исполнены во всех трассах исполнения,
/// что мы собрали в [`collect_source_data`].
fn collect_executed_lines_per_files(
    traces: &Vec<HighLevelTrace>,
) -> HashMap<String, BTreeMap<i64, u64>> {
    let mut line_hits_per_file: HashMap<String, BTreeMap<i64, u64>> = HashMap::new();

    for trace in traces {
        for step in &trace.steps {
            match step {
                HighLevelTraceStep::Mapped(step) => {
                    for loc in &step.locs {
                        let file = &loc.loc.file;
                        let line = loc.loc.line;
                        let entry = line_hits_per_file.entry(file.clone()).or_default();

                        *entry.entry(line).or_insert(0) += 1;
                    }
                }
                HighLevelTraceStep::Unmapped(_) => {}
            }
        }
    }
    line_hits_per_file
}

fn build_executable_lines_per_files(
    data: &Vec<SourceMapAndLogs>,
) -> HashMap<String, BTreeSet<i64>> {
    let mut seen_source_maps = HashSet::new();
    let mut executable_lines_per_file: HashMap<String, BTreeSet<i64>> = HashMap::new();

    for SourceMapAndLogs { source_map, .. } in data {
        if !seen_source_maps.insert(source_map.hash()) {
            continue;
        }

        build_executable_lines_per_file(&mut executable_lines_per_file, source_map);
    }

    executable_lines_per_file
}

fn build_executable_lines_per_file(
    executable_lines_per_file: &mut HashMap<String, BTreeSet<i64>>,
    source_map: &SourceMap,
) {
    let mut local_executable_lines_per_file: HashMap<String, BTreeSet<i64>> = HashMap::new();
    let source_maps_locations = &source_map.high_level.locations;
    let executable_locations = source_maps_locations;

    for loc in executable_locations {
        let file = &loc.loc.file;

        // ignore stdlib and test files completely
        // we also don't need to collect executable lines for Acton stdlib
        if file.contains("@stdlib/")
            || file.is_empty()
            || file.contains("/lib/")
            || file.contains("/.acton/")
            || file.contains(".test.tolk")
        {
            continue;
        }

        let file = file.clone();
        if executable_lines_per_file.contains_key(&file) {
            // we already have executable lines for this file
            continue;
        }

        if let EntryContextDescription::Basic { ast_kind } = &loc.context.description
            && ast_kind == "ast_block_statement"
        {
            // skip block statements
            continue;
        }

        let entry = local_executable_lines_per_file.entry(file).or_default();
        entry.insert(loc.loc.line);
    }

    for (path, locs) in &local_executable_lines_per_file {
        if executable_lines_per_file.contains_key(path) {
            // we already have executable lines for this file
            continue;
        }
        executable_lines_per_file.insert(path.clone(), locs.clone());
    }
}

#[allow(dead_code)] // maybe for command like coverage merge
pub fn merge_coverages(coverages: &Vec<Coverage>) -> Coverage {
    let mut merged_files: HashMap<String, FileCoverage> = HashMap::new();

    for coverage in coverages {
        for file_coverage in &coverage.files {
            let file = &file_coverage.file;
            if let Some(existing) = merged_files.get_mut(file) {
                // Если в одном покрытии строки были покрыты как: 1, 1, 0, 1,
                //                                а в другом как: 1, 1, 1, 0,
                //                    то мы получим в результате: 2, 2, 1, 1.
                for (&line, &hits) in &file_coverage.line_hits {
                    *existing.line_hits.entry(line).or_insert(0) += hits;
                }

                // Если по какой-то причине между покрытиями у конкретного файла другое количество
                // исполняемых строк, то добавляем все исполняемые строки из второго покрытия, чтобы
                // в результате исполняемые строки были объединением всех исполняемых строк.
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

pub fn print_coverage_summary(coverage: &Coverage) {
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

    for (percentage, file_coverage) in files_with_percentage {
        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
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

pub fn generate_lcov_file(coverage: &Coverage, output_path: &str) -> Result<(), std::io::Error> {
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

        lcov_content.push_str("end_of_record\n");
    }

    fs::write(output_path, lcov_content)
}

pub fn generate_text_file(coverage: &Coverage, output_path: &str) -> Result<(), std::io::Error> {
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
        "Lines: {}/{} ({:.2}%)\n",
        covered_lines, total_lines, coverage_percentage
    ));

    let mut total_hits = 0u64;
    for file_coverage in &coverage.files {
        for &hits in file_coverage.line_hits.values() {
            total_hits += hits;
        }
    }

    result.push_str(&format!("Total Hits: {}\n", total_hits));
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
                let line_number_padded =
                    format!("{:>width$}", line_number, width = max_line_number_width);

                let is_executable = file_coverage.executable_lines.contains(&(line_idx as i64));

                if is_executable {
                    let hits = file_coverage
                        .line_hits
                        .get(&(line_idx as i64))
                        .copied()
                        .unwrap_or(0);
                    let status = if hits > 0 { "✓ " } else { "✗ " };
                    let hits_info = format!(" hits:{}", hits);

                    let padding = " ".repeat(code_width.saturating_sub(line.len()));
                    result.push_str(&format!(
                        "{} {}| {}{}|{}\n",
                        line_number_padded, status, line, padding, hits_info
                    ));
                } else {
                    let padding = " ".repeat(code_width.saturating_sub(line.len()));
                    result.push_str(&format!(
                        "{}   | {}{}|\n",
                        line_number_padded, line, padding
                    ));
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
