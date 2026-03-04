use acton_config::color::OwoColorize;
use anyhow::{Context, Result, anyhow};
use tasm::spec::{SpecInstruction, load_tvm_specification};

const FIELD_LABEL_WIDTH: usize = 14;

#[derive(serde::Serialize)]
struct FindQueryResult {
    query: String,
    matches: Vec<String>,
}

pub fn doc_tvm_cmd(
    instruction_names: &[String],
    json: bool,
    find: bool,
    search_in_description: bool,
) -> Result<()> {
    if instruction_names.is_empty() {
        anyhow::bail!("Instruction name cannot be empty");
    }

    if search_in_description && !find {
        anyhow::bail!("--description can only be used together with --find");
    }

    let spec = load_tvm_specification().context("Failed to load built-in TVM specification")?;

    if find {
        let mut results = Vec::new();
        for query in instruction_names {
            let normalized_search_query = normalize_search_text(query);
            if normalized_search_query.is_empty() {
                anyhow::bail!("Instruction name cannot be empty");
            }

            let matches = find_instruction_names(
                &spec.instructions,
                &normalized_search_query,
                search_in_description,
            );
            if matches.is_empty() {
                anyhow::bail!(
                    "No TVM instructions found for query '{query}' in the built-in specification"
                );
            }

            results.push(FindQueryResult {
                query: query.clone(),
                matches,
            });
        }

        if json {
            println!("{}", serde_json::to_string_pretty(&results)?);
            return Ok(());
        }

        for (idx, result) in results.iter().enumerate() {
            if idx > 0 {
                println!();
            }
            println!(
                "Found {} instruction(s) for query '{}':",
                result.matches.len(),
                result.query
            );
            for name in &result.matches {
                println!("- {name}");
            }
        }
        return Ok(());
    }

    let mut resolved_instructions = Vec::new();
    for instruction_name in instruction_names {
        let normalized_name_query = normalize_instruction_name(instruction_name);
        if normalized_name_query.is_empty() {
            anyhow::bail!("Instruction name cannot be empty");
        }

        let instruction =
            find_instruction(&spec.instructions, &normalized_name_query).ok_or_else(|| {
                instruction_not_found_error(
                    instruction_name,
                    &spec.instructions,
                    &normalized_name_query,
                )
            })?;
        resolved_instructions.push(instruction);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&resolved_instructions)?);
        return Ok(());
    }

    for (idx, instruction) in resolved_instructions.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        print_text_instruction(instruction);
    }
    Ok(())
}

fn find_instruction<'a>(
    instructions: &'a [SpecInstruction],
    normalized_name_query: &str,
) -> Option<&'a SpecInstruction> {
    instructions
        .iter()
        .find(|instruction| normalize_instruction_name(&instruction.name) == normalized_name_query)
}

fn instruction_not_found_error(
    raw_query: &str,
    instructions: &[SpecInstruction],
    normalized_name_query: &str,
) -> anyhow::Error {
    let suggestions = suggest_instruction_names(instructions, normalized_name_query, 5);

    if suggestions.is_empty() {
        anyhow!("TVM instruction '{raw_query}' not found in the built-in specification")
    } else {
        anyhow!(
            "TVM instruction '{raw_query}' not found in the built-in specification. Did you mean: {}?",
            suggestions.join(", ")
        )
    }
}

fn suggest_instruction_names(
    instructions: &[SpecInstruction],
    normalized_name_query: &str,
    limit: usize,
) -> Vec<String> {
    find_instruction_names(instructions, normalized_name_query, false)
        .into_iter()
        .take(limit)
        .collect()
}

fn find_instruction_names(
    instructions: &[SpecInstruction],
    normalized_query: &str,
    search_in_description: bool,
) -> Vec<String> {
    if normalized_query.is_empty() {
        return Vec::new();
    }

    let threshold = fuzzy_distance_threshold(normalized_query.len());
    let min_similarity = fuzzy_similarity_threshold(normalized_query.len());
    let mut ranked: Vec<(u8, usize, f64, &str)> = instructions
        .iter()
        .filter_map(|instruction| {
            let normalized_name = normalize_instruction_name(&instruction.name);
            let distance = strsim::levenshtein(normalized_query, &normalized_name);
            let similarity = strsim::jaro_winkler(normalized_query, &normalized_name);

            let (tier, score_distance, score_similarity) = if normalized_name == normalized_query {
                Some((0, distance, -similarity))
            } else if normalized_name.starts_with(normalized_query) {
                Some((1, distance, -similarity))
            } else if normalized_name.contains(normalized_query) {
                Some((2, distance, -similarity))
            } else if distance <= threshold {
                Some((3, distance, -similarity))
            } else if similarity >= min_similarity {
                Some((4, distance, -similarity))
            } else if search_in_description {
                let normalized_description = instruction_description_search_text(instruction);
                description_match_score(
                    normalized_query,
                    &normalized_description,
                    threshold,
                    min_similarity,
                )
            } else {
                None
            }?;

            Some((
                tier,
                score_distance,
                score_similarity,
                instruction.name.as_str(),
            ))
        })
        .collect();

    ranked.sort_unstable_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.total_cmp(&right.2))
            .then_with(|| left.3.cmp(right.3))
    });

    ranked
        .into_iter()
        .map(|(_, _, _, name)| name.to_string())
        .collect()
}

const fn fuzzy_distance_threshold(query_len: usize) -> usize {
    match query_len {
        0..=4 => 1,
        5..=8 => 2,
        _ => 3,
    }
}

const fn fuzzy_similarity_threshold(query_len: usize) -> f64 {
    match query_len {
        0..=4 => 0.92,
        5..=8 => 0.88,
        _ => 0.84,
    }
}

fn description_match_score(
    normalized_query: &str,
    normalized_description: &str,
    threshold: usize,
    min_similarity: f64,
) -> Option<(u8, usize, f64)> {
    if normalized_description.contains(normalized_query) {
        return Some((5, 0, -1.0));
    }

    if normalized_query.contains(' ') {
        return None;
    }

    let mut best_match: Option<(usize, f64)> = None;
    for token in normalized_description.split_whitespace() {
        let distance = strsim::levenshtein(normalized_query, token);
        let similarity = strsim::jaro_winkler(normalized_query, token);
        if distance > threshold && similarity < min_similarity {
            continue;
        }

        match best_match {
            Some((best_distance, best_similarity))
                if distance > best_distance
                    || (distance == best_distance && similarity <= best_similarity) => {}
            _ => {
                best_match = Some((distance, similarity));
            }
        }
    }

    best_match.map(|(distance, similarity)| (6, distance, -similarity))
}

fn instruction_description_search_text(instruction: &SpecInstruction) -> String {
    let mut text = String::new();
    text.push_str(&instruction.description.short);
    text.push(' ');
    text.push_str(&instruction.description.long);
    if !instruction.description.operands.is_empty() {
        text.push(' ');
        text.push_str(&instruction.description.operands.join(" "));
    }
    if !instruction.description.tags.is_empty() {
        text.push(' ');
        text.push_str(&instruction.description.tags.join(" "));
    }
    normalize_search_text(&text)
}

fn normalize_search_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch.to_ascii_uppercase()
            } else if ch == '-' || ch == '#' {
                '_'
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_instruction_name(name: &str) -> String {
    normalize_search_text(name).replace(' ', "")
}

fn print_text_instruction(instruction: &SpecInstruction) {
    print_field("Instruction", &instruction.name);

    let category_value = if instruction.sub_category.trim().is_empty() {
        instruction.category.clone()
    } else {
        format!("{} / {}", instruction.category, instruction.sub_category)
    };
    print_field("Category", category_value);

    print_field(
        "Opcode",
        format!(
            "{} (tlb: {})",
            instruction.layout.prefix_str, instruction.layout.tlb
        ),
    );

    if let Some(stack_string) = instruction
        .signature
        .as_ref()
        .and_then(|signature| signature.stack_string.as_ref())
    {
        print_field("Stack", stack_string);
    }

    if !instruction.description.operands.is_empty() {
        print_list_section(
            "Operands",
            instruction.description.operands.iter().map(String::as_str),
        );
    }

    if !instruction.effects.is_empty() {
        print_list_section("Effects", instruction.effects.iter().map(String::as_str));
    }

    if !instruction.description.short.trim().is_empty() {
        print_section("Summary", instruction.description.short.trim());
    }

    if !instruction.description.long.trim().is_empty() {
        print_section("Description", instruction.description.long.trim());
    }

    if !instruction.description.exit_codes.is_empty() {
        let exit_codes = instruction
            .description
            .exit_codes
            .iter()
            .map(|exit_code| format!("{}: {}", exit_code.errno, exit_code.condition))
            .collect::<Vec<_>>();
        print_list_section("Exit codes", exit_codes.iter().map(String::as_str));
    }

    if !instruction.description.related_instructions.is_empty() {
        print_list_section(
            "Related",
            instruction
                .description
                .related_instructions
                .iter()
                .map(String::as_str),
        );
    }

    if !instruction.description.tags.is_empty() {
        print_list_section(
            "Tags",
            instruction.description.tags.iter().map(String::as_str),
        );
    }
}

fn print_field(label: &str, value: impl std::fmt::Display) {
    let label = format!("{label}:");
    let padded_label = format!("{label:<FIELD_LABEL_WIDTH$}");
    println!("{} {}", padded_label.bold(), value);
}

fn print_section(title: &str, content: &str) {
    println!();
    println!("{}", format!("{title}:").bold());
    for line in content.lines() {
        println!("{line}");
    }
}

fn print_list_section<'a>(title: &str, items: impl Iterator<Item = &'a str>) {
    println!();
    println!("{}", format!("{title}:").bold());
    for item in items {
        println!("- {item}");
    }
}
