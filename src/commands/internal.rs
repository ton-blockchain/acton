use acton_config::config::manifest_path;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, value};

pub fn internal_register_contract(path: &str, id: Option<String>) -> Result<()> {
    let config_path = manifest_path();
    if !config_path.exists() {
        anyhow::bail!("Acton.toml not found");
    }

    let content = fs::read_to_string(config_path)?;
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| anyhow::anyhow!("Failed to parse Acton.toml: {e}\nContent:\n{content}"))?;

    let contracts = doc
        .entry("contracts")
        .or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        })
        .as_table_mut()
        .context("contracts is not a table")?;

    let file_path = Path::new(path);
    let base_id = id.unwrap_or_else(|| {
        file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("contract")
            .to_string()
    });

    let mut contract_id = base_id.clone();
    let mut counter = 1;
    while contracts.contains_key(&contract_id) {
        contract_id = format!("{base_id}{counter}");
        counter += 1;
    }

    let mut contract_table = Table::new();
    contract_table["name"] = value(contract_id.clone());
    contract_table["src"] = value(path.to_string());

    contracts.insert(&contract_id, Item::Table(contract_table));

    fs::write(config_path, doc.to_string())?;

    println!("Contract '{contract_id}' registered successfully");

    Ok(())
}
