use crate::commands::common::symlink_global_wallets;
use crate::config::{ActonConfig, ContractConfig, ContractsConfig};
use include_dir::{Dir, include_dir};
use owo_colors::OwoColorize;
use std::collections::BTreeMap;
use std::fs;
use tree_sitter::Node;
use walkdir::WalkDir;

static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");
static TOLK_STDLIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/crates/tolkc/assets/tolk-stdlib");

pub fn init_cmd() -> anyhow::Result<()> {
    if std::path::Path::new("Acton.toml").exists() {
        println!("{}", "Acton.toml already exists!".yellow());
        return Ok(());
    }

    let mut config = ActonConfig::default();

    let discovered_contracts = discover_contracts();
    let contract_count = discovered_contracts.len();

    if !discovered_contracts.is_empty() {
        println!(
            "Discovered {} contract{}:",
            contract_count,
            if contract_count == 1 { "" } else { "s" }
        );
        for (key, contract) in &discovered_contracts {
            println!("  {} ({})", contract.name.cyan(), key);
        }
        println!();
        config.contracts = Some(ContractsConfig {
            contracts: discovered_contracts,
        });
    } else {
        println!("No contracts found in the current directory.");
    }

    config.save()?;

    fs::create_dir_all(".acton/tolk-stdlib")?;
    LIB_DIR.extract(".acton")?;
    TOLK_STDLIB_DIR.extract(".acton/tolk-stdlib")?;

    println!("{}", "✓ Initialized new Acton project".green().bold());

    patch_or_create_gitignore()?;

    if let Err(e) = symlink_global_wallets() {
        println!(
            "  {} Failed to symlink global wallets: {}",
            "Warning:".yellow().bold(),
            e
        );
    }

    println!("Created {} with project configuration", "Acton.toml".cyan());
    println!(
        "Created {} directory with standard library",
        ".acton/".cyan()
    );
    println!(
        "Created {} directory with Tolk standard library",
        ".acton/tolk-stdlib".cyan()
    );

    Ok(())
}

fn patch_or_create_gitignore() -> anyhow::Result<()> {
    let content = if fs::exists(".gitignore").unwrap_or(false) {
        fs::read_to_string(".gitignore")?
    } else {
        String::new()
    };
    let lines = content.lines().map(|l| l.trim()).collect::<Vec<_>>();

    let mut to_add = String::new();

    if !lines.contains(&".acton/") {
        to_add.push_str("\n# Acton main directory\n.acton/\n");
    }

    let wallet_patterns = ["*.mnemonic", "wallets.toml", "global.wallets.toml"];
    let missing_wallets: Vec<_> = wallet_patterns
        .iter()
        .filter(|p| !lines.contains(p))
        .collect();

    if !missing_wallets.is_empty() {
        to_add.push_str("\n# Mnemonic and wallet files\n");
        for p in missing_wallets {
            to_add.push_str(p);
            to_add.push('\n');
        }
    }

    if !to_add.is_empty() {
        let mut new_content = content.clone();
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&to_add);
        fs::write(".gitignore", new_content)?;
        println!("Patched {} with Acton patterns", ".gitignore".cyan());
    }
    Ok(())
}

fn discover_contracts() -> BTreeMap<String, ContractConfig> {
    let mut contracts = BTreeMap::new();

    for entry in WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !file_name.starts_with('.')
                && !matches!(file_name, "node_modules" | "target" | ".git" | ".acton")
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension() != Some("tolk".as_ref()) {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let tree = match tolk_parser::parser::parse(&content) {
            Ok(tree) => tree,
            Err(_) => continue,
        };

        // treat all files with onInternalMessage as a contract entry file
        if !has_on_internal_message_function(&tree.root_node(), &content) {
            continue;
        }

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let relative_path = path
            .strip_prefix(".")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let contract_key = file_stem.replace("-", "_");
        let contract_name = format_contract_name(file_stem);

        let contract_config = ContractConfig {
            name: contract_name,
            src: relative_path,
            depends: Some(vec![]),
            output: None,
        };

        contracts.insert(contract_key, contract_config);
    }

    contracts
}

fn has_on_internal_message_function(root_node: &Node, content: &str) -> bool {
    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.kind() == "function_declaration"
            && let Some(name_node) = child.child_by_field_name("name")
        {
            let name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
            if name == "onInternalMessage" {
                return true;
            }
        }
    }
    false
}

fn format_contract_name(file_stem: &str) -> String {
    file_stem
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => "".to_owned(),
                Some(first) => first.to_uppercase().chain(chars.as_str().chars()).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
