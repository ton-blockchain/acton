pub mod error_fmt {
    use crate::config::ActonConfig;
    use owo_colors::OwoColorize;

    pub fn contract_not_found(config: &ActonConfig, name: &str) -> String {
        let available = available_contracts(config);
        format!(
            "Contract '{}' not found in Acton.toml\nAvailable contracts:\n{}",
            name, available
        )
    }

    pub fn available_contracts(config: &ActonConfig) -> String {
        let contracts = config.contracts();
        if contracts.is_none() || contracts.as_ref().map(|c| c.is_empty()).unwrap_or(false) {
            return "no contracts defined yet".to_string();
        }
        contracts
            .map(|contracts| {
                contracts
                    .keys()
                    .map(|s| format!(" {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "none".to_string())
    }
}
