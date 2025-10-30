use crate::context::{BuildCache, Emulations};
use crate::vmtrace::build_vm_trace;
use comfy_table::{Cell as TableCell, CellAlignment, Color, ContentArrangement, Table};
use emulator::emulator::SendMessageResult;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tolkc::source_map::DebugLocation;

#[derive(Debug, Clone)]
pub struct Coverage {
    pub files: Vec<FileCoverage>,
}

#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub file: String,
    pub executable_lines: i64,
    pub covered_lines: i64,
    pub line_hits: HashMap<i64, u64>, // line number -> hit count
    pub executable_line_numbers: Vec<i64>, // all executable line numbers
}

pub fn collect_coverage(emulations: &Emulations, build_cache: &BuildCache) -> Coverage {
    let all_results = emulations.results.iter().flat_map(|result| result);
    let successful_results = all_results.filter_map(|result| match result {
        SendMessageResult::Success(result) => Some(result),
        SendMessageResult::Error(_) => None,
    });

    let mut whole_executable_locations_per_file: HashMap<String, Vec<DebugLocation>> =
        HashMap::new();

    let mut whole_trace = vec![];

    for emulation in successful_results {
        let Some(result) = build_cache.result_for_code(&emulation.code) else {
            continue;
        };

        let source_map = result.1.source_map;
        let logs = &emulation.vm_log;

        let mut trace = build_vm_trace(logs, &source_map);
        whole_trace.append(&mut trace);

        let mut executable_locations_per_file: HashMap<String, Vec<DebugLocation>> = HashMap::new();
        let source_maps_locations = source_map.high_level.locations;
        let executable_locations = source_maps_locations;
        for loc in executable_locations {
            if loc.loc.file.contains("@stdlib/")
                || loc.loc.file.is_empty()
                || loc.loc.file.contains("/lib/")
                || loc.loc.file.contains("/.acton/")
                || loc.loc.file.contains("_test.tolk")
            {
                continue;
            }

            let file = loc.loc.file.clone();
            if whole_executable_locations_per_file.contains_key(&file) {
                // we already have executable lines for this file
                continue;
            }

            let entry = executable_locations_per_file
                .entry(file)
                .or_insert_with(Vec::new);
            entry.push(loc);
        }

        for (path, locs) in &executable_locations_per_file {
            if whole_executable_locations_per_file.contains_key(path) {
                // we already have executable lines for this file
                continue;
            }
            whole_executable_locations_per_file.insert(path.clone(), locs.clone());
        }
    }

    let mut coverages: Vec<FileCoverage> = vec![];

    let mut line_hits_per_file: HashMap<String, HashMap<i64, u64>> = HashMap::new();
    let mut executable_lines_per_file: HashMap<String, Vec<i64>> = HashMap::new();

    for (file, locations) in &whole_executable_locations_per_file {
        let mut unique_lines = HashSet::new();
        for loc in locations {
            unique_lines.insert(loc.loc.line);
        }
        let executable_line_numbers: Vec<i64> = unique_lines.into_iter().collect();
        executable_lines_per_file.insert(file.clone(), executable_line_numbers);
    }

    for loc in &whole_trace {
        let file = &loc.loc.file;
        let line = loc.loc.line;
        let entry = line_hits_per_file
            .entry(file.clone())
            .or_insert_with(HashMap::new);
        *entry.entry(line).or_insert(0) += 1;
    }

    let executed_lines = whole_trace
        .iter()
        .map(|loc| loc.loc.line)
        .collect::<HashSet<_>>();

    for (file, locations) in &whole_executable_locations_per_file {
        let executable_lines = locations.len() as i64;
        let mut covered_lines = 0;

        for file_loc in locations {
            let covered = executed_lines.contains(&file_loc.loc.line);
            if covered {
                covered_lines += 1
            }
        }

        let line_hits = line_hits_per_file.get(file).cloned().unwrap_or_default();
        let executable_line_numbers = executable_lines_per_file
            .get(file)
            .cloned()
            .unwrap_or_default();

        coverages.push(FileCoverage {
            file: file.clone(),
            executable_lines,
            covered_lines,
            line_hits,
            executable_line_numbers,
        })
    }

    Coverage { files: coverages }
}

pub fn print_coverage_summary(coverages: &Vec<Coverage>, teamcity: bool) {
    if teamcity {
        return;
    }

    if coverages.iter().all(|coverage| coverage.files.is_empty()) {
        // Empty coverage info, likely compilation error
        return;
    }

    println!("\n{} {}\n", " COVERAGE ".bold().on_cyan(), "".dimmed());

    let mut table = Table::new();
    table
        .load_preset("  ─  ──      ─     ")
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["File", "Covered Lines", "Total Lines", "% Lines"]);

    let mut total_executable_lines = 0i64;
    let mut total_covered_lines = 0i64;

    for coverage in coverages {
        for file_coverage in &coverage.files {
            total_executable_lines += file_coverage.executable_lines;
            total_covered_lines += file_coverage.covered_lines;
        }
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
            TableCell::new(format!("{:.1}%", total_percentage))
                .set_alignment(CellAlignment::Right)
                .fg(total_percentage_color),
        ]);
    }

    for coverage in coverages {
        for file_coverage in &coverage.files {
            let percentage = if file_coverage.executable_lines > 0 {
                (file_coverage.covered_lines as f64 / file_coverage.executable_lines as f64 * 100.0)
            } else {
                0.0
            };

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
                TableCell::new(file_coverage.covered_lines.to_string())
                    .set_alignment(CellAlignment::Right)
                    .fg(covered_color),
                TableCell::new(file_coverage.executable_lines.to_string())
                    .set_alignment(CellAlignment::Right),
                TableCell::new(format!("{:.1}%", percentage))
                    .set_alignment(CellAlignment::Right)
                    .fg(percentage_color),
            ]);
        }
    }

    println!("{}", table);
}

pub fn generate_lcov_file(
    coverages: &Vec<Coverage>,
    output_path: &str,
) -> Result<(), std::io::Error> {
    let mut lcov_content = String::new();

    for coverage in coverages {
        for file_coverage in &coverage.files {
            if file_coverage.line_hits.is_empty() {
                continue;
            }

            // SF: source file
            lcov_content.push_str(&format!("SF:{}\n", file_coverage.file));

            // DA: line data (line number, execution count)
            for &line_number in &file_coverage.executable_line_numbers {
                let hit_count = file_coverage
                    .line_hits
                    .get(&line_number)
                    .copied()
                    .unwrap_or(0);
                lcov_content.push_str(&format!("DA:{},{}\n", line_number + 1, hit_count));
            }

            // LF: lines found (total executable lines)
            lcov_content.push_str(&format!("LF:{}\n", file_coverage.executable_lines));

            // LH: lines hit (covered lines)
            lcov_content.push_str(&format!("LH:{}\n", file_coverage.covered_lines));

            lcov_content.push_str("end_of_record\n");
        }
    }

    fs::write(output_path, lcov_content)
}
