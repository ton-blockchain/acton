use std::collections::HashMap;

/// Helper for checking compilation order in build output
pub struct CompilationOrder {
    positions: HashMap<String, usize>,
}

impl CompilationOrder {
    /// Extract compilation order from build stdout
    pub fn from_stdout(stdout: &str) -> Self {
        let mut positions = HashMap::new();
        let compiled = extract_compiled_contracts(stdout);
        for contract in compiled {
            if let Some(pos) = stdout.find(&format!("Compiling {}", contract)) {
                positions.insert(contract, pos);
            }
        }
        Self { positions }
    }

    /// Assert that first contract was compiled before second
    pub fn assert_before(&self, first: &str, second: &str) {
        let first_pos = self
            .positions
            .get(first)
            .unwrap_or_else(|| panic!("Contract '{}' was not compiled", first));
        let second_pos = self
            .positions
            .get(second)
            .unwrap_or_else(|| panic!("Contract '{}' was not compiled", second));

        assert!(
            first_pos < second_pos,
            "{} (at {}) should be compiled before {} (at {})",
            first,
            first_pos,
            second,
            second_pos
        );
    }

    /// Assert that contracts were compiled in the given order
    pub fn assert_chain(&self, contracts: &[&str]) {
        for i in 0..contracts.len() - 1 {
            self.assert_before(contracts[i], contracts[i + 1]);
        }
    }

    /// Get number of compiled contracts
    pub fn count(&self) -> usize {
        self.positions.len()
    }

    /// Check if specific contract was compiled
    pub fn contains(&self, contract: &str) -> bool {
        self.positions.contains_key(contract)
    }
}

/// Extract list of compiled contracts from build stdout
pub fn extract_compiled_contracts(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            if line.contains("Compiling contracts") {
                return None;
            }

            if line.contains("Compiling ") {
                line.split("Compiling ")
                    .nth(1)
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .collect()
}
