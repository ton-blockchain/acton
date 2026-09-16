use std::{future::Future, sync::Arc};

use thiserror::Error;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;
use tracing::instrument::WithSubscriber;

use crate::{
    blockchain::{BlockchainClient, MultiNetworkToncenterClient},
    compilation_queue::{CompilationQueue, CompilationStatus},
    compilers::{
        CompileOutput, CompileRequest, CompilerError, CompilerService, NodeCompilerService,
    },
    config::{Config, DEFAULT_MAX_CONCURRENT_COMPILATIONS, DEFAULT_MAX_REQUEST_BYTES},
    payment::{OnchainPaymentVerifier, PaymentError, PaymentVerifier},
    registry::{SourceVerificationRegistry, VerificationRegistry},
    registry_index::{SqliteVerificationIndex, VerificationIndexError},
    source_storage::GitSourceStorage,
    verification::VerificationService,
};

#[derive(Clone)]
pub struct AppState {
    api_key: Option<String>,
    read_only: bool,
    compiler_service: Arc<dyn CompilerService>,
    verification_registry: Arc<dyn VerificationRegistry>,
    verification_service: VerificationService,
    payment_verifier: Arc<dyn PaymentVerifier>,
    compilation_queue: CompilationQueue,
    max_request_bytes: usize,
    background_tasks: TaskTracker,
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
            Arc::new(MultiNetworkToncenterClient::from_config(config)),
            Arc::new(NodeCompilerService::from_config(config)),
            verification_registry,
            payment_verifier,
        )
        .with_api_key(config.api_key())
        .with_read_only(config.read_only())
        .with_max_concurrent_compilations(config.max_concurrent_compilations())
        .with_max_request_bytes(config.max_request_bytes()))
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
            read_only: false,
            compiler_service,
            verification_registry,
            verification_service: VerificationService::new(blockchain_client),
            payment_verifier,
            compilation_queue: CompilationQueue::new(Some(DEFAULT_MAX_CONCURRENT_COMPILATIONS)),
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            background_tasks: TaskTracker::new(),
        }
    }

    #[must_use]
    pub fn with_api_key(mut self, api_key: Option<&str>) -> Self {
        self.api_key = api_key.map(ToOwned::to_owned);
        self
    }

    #[must_use]
    pub const fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub const fn with_max_request_bytes(mut self, max_request_bytes: usize) -> Self {
        self.max_request_bytes = max_request_bytes;
        self
    }

    #[must_use]
    pub fn with_max_concurrent_compilations(
        mut self,
        max_concurrent_compilations: Option<usize>,
    ) -> Self {
        self.compilation_queue = CompilationQueue::new(max_concurrent_compilations);
        self
    }

    #[must_use]
    pub const fn max_request_bytes(&self) -> usize {
        self.max_request_bytes
    }

    #[must_use]
    pub fn api_key_matches(&self, api_key: Option<&str>) -> bool {
        self.api_key
            .as_deref()
            .zip(api_key)
            .is_some_and(|(expected, actual)| expected == actual)
    }

    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    pub(crate) async fn compile(
        &self,
        code_hash: &str,
        request: CompileRequest,
    ) -> Result<CompileOutput, CompilerError> {
        let _permit = self.compilation_queue.acquire(code_hash).await?;
        self.compiler_service.compile(request).await
    }

    #[must_use]
    pub(crate) fn compilation_status(&self, code_hash: &str) -> Option<CompilationStatus> {
        self.compilation_queue.status(code_hash)
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

    /// Rebuilds payment replay state from TON history.
    ///
    /// # Errors
    ///
    /// Returns an error when blockchain history or the payment ledger is unavailable.
    pub async fn recover_payment_history(
        &self,
        published_transaction_hashes: &[String],
    ) -> Result<(), StateError> {
        self.payment_verifier
            .recover(published_transaction_hashes)
            .await?;
        Ok(())
    }

    /// Returns payments referenced by source bundles in the current Git revision.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry index is unavailable.
    pub async fn published_payment_transaction_hashes(&self) -> Result<Vec<String>, StateError> {
        Ok(self
            .verification_registry
            .payment_transaction_hashes()
            .await?)
    }

    pub(crate) fn spawn_background_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.background_tasks.spawn(task.with_current_subscriber())
    }

    /// Closes the tracker and waits for background verification tasks to finish.
    pub async fn wait_for_background_tasks(&self) {
        self.background_tasks.close();
        self.background_tasks.wait().await;
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
