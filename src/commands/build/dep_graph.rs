use crate::config::{ContractConfig, DependencyKind};
use anyhow::anyhow;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::process::Command;

pub(crate) fn build_dependency_graph(
    contracts: &[(&String, &ContractConfig)],
) -> anyhow::Result<Vec<String>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();

    for (key, _) in contracts {
        graph.insert((*key).clone(), Vec::new());
        in_degree.insert((*key).clone(), 0);
    }

    for (key, config) in contracts {
        let Some(depends) = &config.depends else {
            continue;
        };

        for dep in depends {
            let dep_name = dep.name();
            if !graph.contains_key(dep_name) {
                return Err(anyhow!(
                    "Contract '{key}' depends on '{dep_name}' which is not defined in Acton.toml"
                ));
            }

            graph
                .get_mut(dep_name)
                .expect("cannot fail")
                .push((*key).clone());
            *in_degree.get_mut(*key).expect("cannot fail") += 1;
        }
    }

    let mut queue: VecDeque<String> = VecDeque::new();
    let mut result: Vec<String> = Vec::new();

    for (key, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(key.clone());
        }
    }

    while let Some(current) = queue.pop_front() {
        result.push(current.clone());

        for neighbor in &graph[&current] {
            let Some(degree) = in_degree.get_mut(neighbor) else {
                break;
            };
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(neighbor.clone());
            }
        }
    }

    if result.len() != contracts.len() {
        let remaining = in_degree
            .iter()
            .filter(|&(_, &degree)| degree > 0)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();

        return Err(anyhow!(
            "Circular dependency detected in contracts: {}",
            format_cycle_error(&remaining, &graph)
        ));
    }

    Ok(result)
}

pub(crate) fn collect_dependencies_for_contract(
    target_contract: &str,
    contracts: &HashMap<String, ContractConfig>,
) -> anyhow::Result<HashSet<String>> {
    let mut dependencies = HashSet::new();
    let mut to_visit = VecDeque::new();
    let mut visited = HashSet::new();

    to_visit.push_back(target_contract.to_string());

    while let Some(current) = to_visit.pop_front() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());

        let contract_config = contracts
            .get(&current)
            .ok_or_else(|| anyhow!("Contract '{current}' not found in Acton.toml"))?;

        if let Some(deps) = &contract_config.depends {
            for dep in deps {
                let dep_name = dep.name();
                dependencies.insert(dep_name.to_string());
                to_visit.push_back(dep_name.to_string());
            }
        }
    }

    Ok(dependencies)
}

pub(crate) fn filter_compilation_order_for_contract(
    target_contract: &str,
    compilation_order: &[String],
    contracts: &HashMap<String, ContractConfig>,
) -> anyhow::Result<Vec<String>> {
    let dependencies = collect_dependencies_for_contract(target_contract, contracts)?;

    let mut filtered_order = Vec::new();

    for contract in compilation_order {
        if dependencies.contains(contract) {
            filtered_order.push(contract.clone());
        }
    }

    filtered_order.push(target_contract.to_string());
    Ok(filtered_order)
}

pub(crate) fn generate_dependency_graph_svg(
    compilation_order: &Vec<String>,
    contracts: &HashMap<String, ContractConfig>,
    output_path: &str,
) -> anyhow::Result<()> {
    let mut dot_content = String::new();
    dot_content.push_str("digraph Dependencies {\n");
    dot_content.push_str("    rankdir=TB;\n");
    dot_content.push_str("    margin=1.0;\n");
    dot_content.push_str("    pad=0.5;\n");
    dot_content.push_str("    node [shape=box];\n\n");

    for key in compilation_order {
        let Some(config) = contracts.get(key) else {
            continue;
        };
        let label = format!("{}\\n({})", config.name, key);
        dot_content.push_str(&format!("    \"{key}\" [label=\"{label}\"];\n"));
    }

    dot_content.push('\n');

    for key in compilation_order {
        let Some(config) = contracts.get(key) else {
            continue;
        };
        if let Some(depends) = &config.depends {
            for dep in depends {
                let dep_name = dep.name();
                let dep_kind = dep.kind();

                let label = match dep_kind {
                    DependencyKind::EmbedCode => " embed code ",
                    DependencyKind::LibraryRef => " library ref ",
                };

                dot_content.push_str(&format!(
                    "    \"{key}\" -> \"{dep_name}\" [label=\"{label}\", labeldistance=3];\n"
                ));
            }
        }
    }

    dot_content.push_str("}\n");

    let dot_path = "deps.dot";
    fs::write(dot_path, &dot_content)?;

    let graphviz_check = Command::new("dot").arg("-V").output();

    match graphviz_check {
        Ok(output) if output.status.success() => {
            let svg_output = Command::new("dot")
                .args(["-Tsvg", dot_path, "-o", output_path])
                .output()?;

            if !svg_output.status.success() {
                let error_msg = String::from_utf8_lossy(&svg_output.stderr);
                return Err(anyhow!("Failed to generate SVG: {error_msg}"));
            }

            let _ = fs::remove_file(dot_path);

            println!(
                "   {} dependency graph: {}",
                "Generated".cyan(),
                output_path
            );
        }
        Ok(_) => {
            return Err(anyhow!(
                "Graphviz 'dot' command failed. Please ensure graphviz is properly installed."
            ));
        }
        Err(_) => {
            return Err(anyhow!(
                "Graphviz not found. Please install graphviz to generate dependency graphs.\n\
                On macOS: brew install graphviz\n\
                On Ubuntu/Debian: sudo apt-get install graphviz\n\
                On Windows: Download from https://graphviz.org/download/"
            ));
        }
    }

    Ok(())
}

fn format_cycle_error(remaining: &[String], graph: &HashMap<String, Vec<String>>) -> String {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut cycle_path = Vec::new();

    for node in remaining {
        if !visited.contains(node)
            && find_cycle_dfs(node, graph, &mut visited, &mut rec_stack, &mut cycle_path)
        {
            break;
        }
    }

    if cycle_path.is_empty() {
        remaining.join(", ").to_string()
    } else {
        cycle_path.reverse();
        format!("{} → {}", cycle_path.join(" → "), cycle_path[0])
    }
}

fn find_cycle_dfs(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    cycle_path: &mut Vec<String>,
) -> bool {
    visited.insert(node.to_string());
    rec_stack.insert(node.to_string());
    cycle_path.push(node.to_string());

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                if find_cycle_dfs(neighbor, graph, visited, rec_stack, cycle_path) {
                    return true;
                }
            } else if rec_stack.contains(neighbor) {
                if let Some(pos) = cycle_path.iter().position(|x| x == neighbor) {
                    cycle_path.drain(0..pos);
                }
                return true;
            }
        }
    }

    rec_stack.remove(node);
    cycle_path.pop();
    false
}
