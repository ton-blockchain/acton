use std::sync::Arc;

use thiserror::Error;

use crate::{
    blockchain::{BlockchainClient, ToncenterClient},
    compilers::{CompilerService, NodeCompilerService},
    config::Config,
    registry::{SourceVerificationRegistry, VerificationRegistry},
    registry_index::{SqliteVerificationIndex, VerificationIndexError},
    source_storage::GitSourceStorage,
    verification::VerificationService,
};

#[derive(Clone)]
pub struct AppState {
    compiler_service: Arc<dyn CompilerService>,
    verification_registry: Arc<dyn VerificationRegistry>,
    verification_service: VerificationService,
}

impl AppState {
    /// Builds application state from runtime configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry index cannot be opened.
    pub fn from_config(config: &Config) -> Result<Self, StateError> {
        let source_storage = Arc::new(GitSourceStorage::from_config(config));
        let verification_index =
            Arc::new(SqliteVerificationIndex::open(config.registry_index_path())?);
        let verification_registry = Arc::new(SourceVerificationRegistry::new(
            source_storage,
            verification_index,
        ));

        Ok(Self::new(
            Arc::new(ToncenterClient::from_config(config)),
            Arc::new(NodeCompilerService::from_config(config)),
            verification_registry,
        ))
    }

    #[must_use]
    pub fn new(
        blockchain_client: Arc<dyn BlockchainClient>,
        compiler_service: Arc<dyn CompilerService>,
        verification_registry: Arc<dyn VerificationRegistry>,
    ) -> Self {
        Self {
            compiler_service,
            verification_registry,
            verification_service: VerificationService::new(blockchain_client),
        }
    }

    #[must_use]
    pub fn compiler_service(&self) -> &dyn CompilerService {
        self.compiler_service.as_ref()
    }

    #[must_use]
    pub fn verification_registry(&self) -> &dyn VerificationRegistry {
        self.verification_registry.as_ref()
    }

    #[must_use]
    pub const fn verification_service(&self) -> &VerificationService {
        &self.verification_service
    }

    /// Rebuilds or refreshes the registry index when it is behind source storage.
    ///
    /// # Errors
    ///
    /// Returns an error when source storage or registry index access fails.
    pub async fn ensure_registry_current(&self) -> Result<(), StateError> {
        self.verification_registry.ensure_current().await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error(transparent)]
    Registry(#[from] crate::registry::RegistryError),
    #[error(transparent)]
    VerificationIndex(#[from] VerificationIndexError),
}
