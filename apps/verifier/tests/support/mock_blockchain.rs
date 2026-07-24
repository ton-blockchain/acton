use std::collections::HashMap;

use async_trait::async_trait;
use verifier::blockchain::{BlockchainClient, BlockchainError};

pub struct MockBlockchainClient {
    code_hashes: HashMap<String, String>,
}

impl MockBlockchainClient {
    pub fn new(code_hashes: &[(&str, &str)]) -> Self {
        Self {
            code_hashes: code_hashes
                .iter()
                .map(|(address, code_hash)| ((*address).to_owned(), (*code_hash).to_owned()))
                .collect(),
        }
    }
}

#[async_trait]
impl BlockchainClient for MockBlockchainClient {
    async fn get_code_hash(&self, address: &str) -> Result<Option<String>, BlockchainError> {
        Ok(self.code_hashes.get(address).cloned())
    }
}
