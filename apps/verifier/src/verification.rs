use std::sync::Arc;

use crate::blockchain::{BlockchainClient, BlockchainError, normalize_code_hash};
use thiserror::Error;

#[derive(Clone)]
pub struct VerificationService {
    blockchain: Arc<dyn BlockchainClient>,
}

impl VerificationService {
    #[must_use]
    pub const fn new(blockchain: Arc<dyn BlockchainClient>) -> Self {
        Self { blockchain }
    }

    pub async fn resolve_target(
        &self,
        target: VerificationTarget,
    ) -> Result<ResolvedVerificationTarget, VerificationError> {
        match (target.address, target.code_hash) {
            (None, None) => Err(VerificationError::MissingTarget),
            (None, Some(code_hash)) => Ok(ResolvedVerificationTarget {
                address: None,
                code_hash: normalize_code_hash(&code_hash),
            }),
            (Some(address), None) => {
                let fetched_code_hash = self.fetch_code_hash(&address).await?;
                Ok(ResolvedVerificationTarget {
                    address: Some(address),
                    code_hash: fetched_code_hash,
                })
            }
            (Some(address), Some(provided_code_hash)) => {
                let provided_code_hash = normalize_code_hash(&provided_code_hash);
                let fetched_code_hash = self.fetch_code_hash(&address).await?;

                if fetched_code_hash != provided_code_hash {
                    return Err(VerificationError::CodeHashMismatch {
                        address,
                        provided: provided_code_hash,
                        actual: fetched_code_hash,
                    });
                }

                Ok(ResolvedVerificationTarget {
                    address: Some(address),
                    code_hash: provided_code_hash,
                })
            }
        }
    }

    async fn fetch_code_hash(&self, address: &str) -> Result<String, VerificationError> {
        self.blockchain
            .get_code_hash(address)
            .await?
            .ok_or_else(|| VerificationError::CodeHashNotFound {
                address: address.to_owned(),
            })
    }
}

pub struct VerificationTarget {
    pub address: Option<String>,
    pub code_hash: Option<String>,
}

pub struct ResolvedVerificationTarget {
    pub address: Option<String>,
    pub code_hash: String,
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("missing verification target: provide address or code_hash")]
    MissingTarget,
    #[error("code_hash was not found for address {address}")]
    CodeHashNotFound { address: String },
    #[error("code_hash mismatch for address {address}: provided={provided}, actual={actual}")]
    CodeHashMismatch {
        address: String,
        provided: String,
        actual: String,
    },
    #[error(transparent)]
    Blockchain(#[from] BlockchainError),
}
