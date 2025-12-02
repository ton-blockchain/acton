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

    pub fn wallet_not_found(config: &ActonConfig, name: &str) -> String {
        let wallets = config.wallets();
        if wallets.is_none() || wallets.as_ref().map(|c| c.is_empty()).unwrap_or(false) {
            return "no wallets defined yet".to_string();
        }
        let available = wallets
            .map(|contracts| {
                contracts
                    .keys()
                    .map(|s| format!(" {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "none".to_string());
        format!(
            "Wallet '{}' not found in Acton.toml\nAvailable wallets:\n{}",
            name, available
        )
    }

    pub fn file_not_found(path: &str) -> String {
        if path.is_empty() {
            return "Empty file path is not allowed".to_string();
        }
        let cwd = std::env::current_dir().unwrap_or(".".into());
        format!(
            "Cannot find file or directory {}",
            format!("{}/{path}", cwd.display()).yellow(),
        )
    }

    pub fn invalid_address(addr: &str) -> String {
        let hint = if (addr.starts_with("U") || addr.starts_with("E") || addr.starts_with("k"))
            && addr.len() == 47
        {
            "Did you miss the last symbol of the address (expected length is 48 but address length is 47)? "
        } else {
            ""
        };

        color_print::cformat!(
            "Address <yellow>{addr}</> is not a valid address. {hint}Enter valid address in user-friendly <green>EQ...</> or raw format <green>0:abcd...</>"
        )
    }
}
