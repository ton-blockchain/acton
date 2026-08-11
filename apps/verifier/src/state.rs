use std::sync::Arc;

use thiserror::Error;

use crate::{
    blockchain::{BlockchainClient, ToncenterClient},
    compilers::{CompilerService, NodeCompilerService},
    config::Config,
    payment::{OnchainPaymentVerifier, PaymentError, PaymentVerifier},
    registry::{SourceVerificationRegistry, VerificationRegistry},
    registry_index::{SqliteVerificationIndex, VerificationIndexError},
    source_storage::GitSourceStorage,
    verification::VerificationService,
};

#[derive(Clone)]
pub struct AppState {
    api_key: Option<String>,
    compiler_service: Arc<dyn CompilerService>,
    verification_registry: Arc<dyn VerificationRegistry>,
    verification_service: VerificationService,
    payment_verifier: Arc<dyn PaymentVerifier>,
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
        let payment_verifier = Arc::new(OnchainPaymentVerifier::from_config(config)?);

        Ok(Self::new(
            Arc::new(ToncenterClient::from_config(config)),
            Arc::new(NodeCompilerService::from_config(config)),
            verification_registry,
            payment_verifier,
        )
        .with_api_key(config.api_key()))
    }

    #[must_use]
    pub fn new(
        blockchain_client: Arc<dyn BlockchainClient>,
        compiler_service: Arc<dyn CompilerService>,
        verification_registry: Arc<dyn VerificationRegistry>,
        payment_verifier: Arc<dyn PaymentVerifier>,
    ) -> Self {
        Self {
            api_key: None,
            compiler_service,
            verification_registry,
            verification_service: VerificationService::new(blockchain_client),
            payment_verifier,
        }
    }

    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<&str>) -> Self {
        self.api_key = api_key.map(ToOwned::to_owned);
        self
    }

    #[must_use]
    pub fn api_key_matches(&self, api_key: Option<&str>) -> bool {
        self.api_key
            .as_deref()
            .zip(api_key)
            .is_some_and(|(expected, actual)| expected == actual)
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

    #[must_use]
    pub fn payment_verifier(&self) -> &dyn PaymentVerifier {
        self.payment_verifier.as_ref()
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

    /// Rebuilds payment replay state from TON testnet history.
    ///
    /// # Errors
    ///
    /// Returns an error when blockchain history or the payment ledger is unavailable.
    pub async fn recover_payment_history(&self) -> Result<(), StateError> {
        self.payment_verifier.recover().await?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error(transparent)]
    Registry(#[from] crate::registry::RegistryError),
    #[error(transparent)]
    VerificationIndex(#[from] VerificationIndexError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
}
