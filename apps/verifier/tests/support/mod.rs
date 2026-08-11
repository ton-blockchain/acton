#![allow(dead_code)]

use std::sync::Mutex;
use std::{borrow::Cow, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, header::CONTENT_TYPE},
    response::Response,
};
use serde::de::DeserializeOwned;
use tower::ServiceExt;
use verifier::{
    app,
    blockchain::ToncenterClient,
    compilers::{CompileGeneratedSource, CompileRequest, NodeCompilerService},
    config::Config,
    payment::{
        HistorySort, OnchainPaymentVerifier, PaymentAttemptOutcome, PaymentBlockchainClient,
        PaymentClaim, PaymentError, PaymentLedger, PaymentMessage, PaymentQuote,
        PaymentTransaction, PaymentVerifier,
    },
    registry::SourceVerificationRegistry,
    registry_index::SqliteVerificationIndex,
    source_storage::{SharedSourceStorage, SourceMapData},
    state::AppState,
};

mod mock_blockchain;
mod mock_compiler;
mod mock_source_storage;

const MULTIPART_BOUNDARY: &str = "verifier-test-boundary";
pub const PAYMENT_ADDRESS: &str =
    "0:1111111111111111111111111111111111111111111111111111111111111111";
pub const PAYMENT_TX_HASH: &str =
    "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b";

pub fn app_state(code_hashes: &[(&str, &str)], compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = Arc::new(mock_source_storage::MockSourceStorage::confirmed());
    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        source_storage,
    )
}

pub fn recovering_payment_app_state(compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    app_state_from_parts_with_payment(
        Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
        Arc::new(MockPaymentVerifier {
            ready: false,
            outcomes: None,
            claim_error: Mutex::new(None),
        }),
    )
}

pub fn recording_payment_app_state(
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<PaymentAttemptOutcome>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let (payment_verifier, outcomes) = recording_payment_verifier();
    (
        app_state_from_parts_with_payment(
            Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
            Arc::new(compiler_service),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
            payment_verifier,
        ),
        outcomes,
    )
}

pub fn payment_error_app_state(compiled_code_hash: &str, error: PaymentError) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    app_state_from_parts_with_payment(
        Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
        Arc::new(MockPaymentVerifier {
            ready: true,
            outcomes: None,
            claim_error: Mutex::new(Some(error)),
        }),
    )
}

pub async fn fail_once_source_storage_app_state(
    compiled_code_hash: &str,
    code_hash: &str,
) -> (
    AppState,
    Arc<Mutex<Vec<mock_source_storage::RecordedSourceStorageRequest>>>,
) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = mock_source_storage::MockSourceStorage::failing_once(
        "source storage internal test details",
    );
    let recorded_requests = source_storage.recorded_requests();
    let client = Arc::new(StaticPaymentBlockchainClient::new(
        Some(payment_transaction(PAYMENT_TX_HASH, code_hash)),
        Vec::new(),
    ));
    let payment_verifier = Arc::new(OnchainPaymentVerifier::new(
        client,
        PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
        PAYMENT_ADDRESS.to_owned(),
        1_000_000,
    ));
    payment_verifier
        .recover()
        .await
        .expect("empty payment history recovery should succeed");

    (
        app_state_from_parts_with_payment(
            Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
            Arc::new(compiler_service),
            Arc::new(source_storage),
            payment_verifier,
        ),
        recorded_requests,
    )
}

pub fn app_state_with_api_key(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
    api_key: &str,
) -> AppState {
    app_state(code_hashes, compiled_code_hash).with_api_key(Some(api_key))
}

pub fn real_compiler_app_state(code_hashes: &[(&str, &str)]) -> AppState {
    let source_storage = Arc::new(mock_source_storage::MockSourceStorage::confirmed());
    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(NodeCompilerService::from_config(&Config::default())),
        source_storage,
    )
}

pub fn toncenter_app_state(base_url: &str, compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = Arc::new(mock_source_storage::MockSourceStorage::confirmed());
    app_state_from_parts(
        Arc::new(ToncenterClient::new(base_url.to_owned(), None)),
        Arc::new(compiler_service),
        source_storage,
    )
}

pub fn toncenter_app_state_with_source_storage(
    base_url: &str,
    compiled_code_hash: &str,
    source_storage: SharedSourceStorage,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    app_state_from_parts(
        Arc::new(ToncenterClient::new(base_url.to_owned(), None)),
        Arc::new(compiler_service),
        source_storage,
    )
}

pub fn recording_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<CompileRequest>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let recorded_requests = compiler_service.recorded_requests();

    (
        app_state_from_parts(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
        ),
        recorded_requests,
    )
}

pub fn mapped_compiler_app_state(compilers: &[(&str, &str, &str)]) -> AppState {
    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
        Arc::new(mock_compiler::MockCompilerService::by_compiler(compilers)),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn recording_source_storage_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (
    AppState,
    Arc<Mutex<Vec<mock_source_storage::RecordedSourceStorageRequest>>>,
) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = mock_source_storage::MockSourceStorage::confirmed();
    let recorded_requests = source_storage.recorded_requests();

    (
        app_state_from_parts(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(source_storage),
        ),
        recorded_requests,
    )
}

pub fn recording_source_storage_app_state_with_generated_sources(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
    generated_sources: Vec<CompileGeneratedSource>,
) -> (
    AppState,
    Arc<Mutex<Vec<mock_source_storage::RecordedSourceStorageRequest>>>,
) {
    let compiler_service = mock_compiler::MockCompilerService::with_generated_sources(
        compiled_code_hash,
        generated_sources,
    );
    let source_storage = mock_source_storage::MockSourceStorage::confirmed();
    let recorded_requests = source_storage.recorded_requests();

    (
        app_state_from_parts(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(source_storage),
        ),
        recorded_requests,
    )
}

pub fn recording_source_storage_app_state_with_source_map_data(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
    source_map: SourceMapData,
) -> (
    AppState,
    Arc<Mutex<Vec<mock_source_storage::RecordedSourceStorageRequest>>>,
) {
    let compiler_service =
        mock_compiler::MockCompilerService::with_source_map_data(compiled_code_hash, source_map);
    let source_storage = mock_source_storage::MockSourceStorage::confirmed();
    let recorded_requests = source_storage.recorded_requests();

    (
        app_state_from_parts(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(source_storage),
        ),
        recorded_requests,
    )
}

pub fn unverified_app_state(code_hashes: &[(&str, &str)], compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn failing_source_storage_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);

    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::failing(
            "source storage failed",
        )),
    )
}

pub fn failing_source_storage_app_state_with_payment_outcomes(
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<PaymentAttemptOutcome>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let (payment_verifier, outcomes) = recording_payment_verifier();
    (
        app_state_from_parts_with_payment(
            Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
            Arc::new(compiler_service),
            Arc::new(mock_source_storage::MockSourceStorage::failing_io(
                "source storage failed",
            )),
            payment_verifier,
        ),
        outcomes,
    )
}

pub fn failing_compiler_app_state(code_hashes: &[(&str, &str)], error: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::failing(error);

    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn failing_compiler_app_state_with_payment_outcomes(
    error: &str,
) -> (AppState, Arc<Mutex<Vec<PaymentAttemptOutcome>>>) {
    let compiler_service = mock_compiler::MockCompilerService::failing(error);
    let (payment_verifier, outcomes) = recording_payment_verifier();
    (
        app_state_from_parts_with_payment(
            Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
            Arc::new(compiler_service),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
            payment_verifier,
        ),
        outcomes,
    )
}

pub fn timing_out_compiler_app_state_with_payment_outcomes(
    timeout_ms: u128,
) -> (AppState, Arc<Mutex<Vec<PaymentAttemptOutcome>>>) {
    let compiler_service = mock_compiler::MockCompilerService::timing_out(timeout_ms);
    let (payment_verifier, outcomes) = recording_payment_verifier();
    (
        app_state_from_parts_with_payment(
            Arc::new(mock_blockchain::MockBlockchainClient::new(&[])),
            Arc::new(compiler_service),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
            payment_verifier,
        ),
        outcomes,
    )
}

pub async fn get(state: AppState, path: &str) -> Response {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .expect("GET request should be valid");

    app::router_with_state(state)
        .oneshot(request)
        .await
        .expect("router should handle GET request")
}

fn app_state_from_parts(
    blockchain_client: Arc<dyn verifier::blockchain::BlockchainClient>,
    compiler_service: Arc<dyn verifier::compilers::CompilerService>,
    source_storage: SharedSourceStorage,
) -> AppState {
    app_state_from_parts_with_payment(
        blockchain_client,
        compiler_service,
        source_storage,
        Arc::new(MockPaymentVerifier {
            ready: true,
            outcomes: None,
            claim_error: Mutex::new(None),
        }),
    )
}

fn app_state_from_parts_with_payment(
    blockchain_client: Arc<dyn verifier::blockchain::BlockchainClient>,
    compiler_service: Arc<dyn verifier::compilers::CompilerService>,
    source_storage: SharedSourceStorage,
    payment_verifier: Arc<dyn PaymentVerifier>,
) -> AppState {
    let verification_index =
        Arc::new(SqliteVerificationIndex::in_memory().expect("SQLite index should open"));
    AppState::new(
        blockchain_client,
        compiler_service,
        Arc::new(SourceVerificationRegistry::new(
            source_storage,
            verification_index,
        )),
        payment_verifier,
    )
}

struct MockPaymentVerifier {
    ready: bool,
    outcomes: Option<Arc<Mutex<Vec<PaymentAttemptOutcome>>>>,
    claim_error: Mutex<Option<PaymentError>>,
}

fn recording_payment_verifier() -> (
    Arc<dyn PaymentVerifier>,
    Arc<Mutex<Vec<PaymentAttemptOutcome>>>,
) {
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(MockPaymentVerifier {
            ready: true,
            outcomes: Some(Arc::clone(&outcomes)),
            claim_error: Mutex::new(None),
        }),
        outcomes,
    )
}

#[async_trait::async_trait]
impl PaymentVerifier for MockPaymentVerifier {
    fn quote(&self, code_hash: &str) -> PaymentQuote {
        PaymentQuote {
            payment_address: "0:1111111111111111111111111111111111111111111111111111111111111111"
                .to_owned(),
            amount_nano: "10000000".to_owned(),
            comment: format!("acton-verify:v1:{code_hash}"),
        }
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    async fn recover(&self) -> Result<(), PaymentError> {
        Ok(())
    }

    async fn claim(
        &self,
        transaction_hash: &str,
        _code_hash: &str,
    ) -> Result<PaymentClaim, PaymentError> {
        let claim_error = self
            .claim_error
            .lock()
            .expect("payment claim error mutex should not be poisoned")
            .take();
        if let Some(error) = claim_error {
            return Err(error);
        }
        Ok(PaymentClaim {
            transaction_hash: transaction_hash.to_owned(),
            claim_version: 1,
        })
    }

    fn finish(
        &self,
        _claim: &PaymentClaim,
        outcome: PaymentAttemptOutcome,
    ) -> Result<(), PaymentError> {
        if let Some(outcomes) = &self.outcomes {
            outcomes
                .lock()
                .expect("payment outcomes mutex should not be poisoned")
                .push(outcome);
        }
        Ok(())
    }
}

pub struct StaticPaymentBlockchainClient {
    transaction: Option<PaymentTransaction>,
    history: Vec<PaymentTransaction>,
}

impl StaticPaymentBlockchainClient {
    pub const fn new(
        transaction: Option<PaymentTransaction>,
        history: Vec<PaymentTransaction>,
    ) -> Self {
        Self {
            transaction,
            history,
        }
    }
}

#[async_trait::async_trait]
impl PaymentBlockchainClient for StaticPaymentBlockchainClient {
    async fn transaction_by_hash(
        &self,
        _transaction_hash: &str,
    ) -> Result<Option<PaymentTransaction>, PaymentError> {
        Ok(self.transaction.clone())
    }

    async fn transactions(
        &self,
        _account: &str,
        limit: usize,
        offset: usize,
        sort: HistorySort,
    ) -> Result<Vec<PaymentTransaction>, PaymentError> {
        let mut history = self.history.clone();
        history.sort_by(|left, right| (left.lt, &left.hash).cmp(&(right.lt, &right.hash)));
        if matches!(sort, HistorySort::Descending) {
            history.reverse();
        }
        Ok(history.into_iter().skip(offset).take(limit).collect())
    }
}

pub fn payment_transaction(transaction_hash: &str, code_hash: &str) -> PaymentTransaction {
    PaymentTransaction {
        account: PAYMENT_ADDRESS.to_owned(),
        hash: transaction_hash.to_owned(),
        lt: 42,
        timestamp: 1_700_000_000,
        emulated: false,
        finality: "finalized".to_owned(),
        aborted: false,
        incoming: Some(PaymentMessage {
            destination: Some(PAYMENT_ADDRESS.to_owned()),
            value: Some(1_000_000),
            bounced: false,
            comment: Some(format!("acton-verify:v1:{code_hash}")),
        }),
    }
}

pub async fn post_verify(state: AppState, parts: Vec<MultipartPart>) -> Response {
    post_verify_request(state, parts, None, true).await
}

pub async fn post_verify_without_payment(state: AppState, parts: Vec<MultipartPart>) -> Response {
    post_verify_request(state, parts, None, false).await
}

pub async fn post_verify_with_api_key(
    state: AppState,
    parts: Vec<MultipartPart>,
    api_key: &str,
) -> Response {
    post_verify_request(state, parts, Some(api_key), true).await
}

async fn post_verify_request(
    state: AppState,
    mut parts: Vec<MultipartPart>,
    api_key: Option<&str>,
    include_payment: bool,
) -> Response {
    if include_payment {
        parts.push(text_part("tx_hash", PAYMENT_TX_HASH));
    }
    let body = multipart_body(parts);
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/verify")
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
        );
    if let Some(api_key) = api_key {
        request = request.header("X-Verifier-Key", api_key);
    }
    let request = request
        .body(Body::from(body))
        .expect("POST /api/v1/verify request should be valid");

    app::router_with_state(state)
        .oneshot(request)
        .await
        .expect("router should handle POST /api/v1/verify request")
}

pub async fn response_json<T>(response: Response) -> T
where
    T: DeserializeOwned,
{
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&bytes).expect("response body should be valid JSON")
}

pub const fn text_part(name: &'static str, value: &'static str) -> MultipartPart {
    MultipartPart::Text {
        name,
        value: Cow::Borrowed(value),
    }
}

pub fn owned_text_part(name: &'static str, value: impl Into<String>) -> MultipartPart {
    MultipartPart::Text {
        name,
        value: Cow::Owned(value.into()),
    }
}

pub const fn file_part(
    name: &'static str,
    file_name: &'static str,
    content_type: &'static str,
    content: &'static str,
) -> MultipartPart {
    MultipartPart::File {
        name,
        file_name: Cow::Borrowed(file_name),
        content_type,
        content: Cow::Borrowed(content),
    }
}

pub fn owned_file_part(
    name: &'static str,
    file_name: impl Into<String>,
    content_type: &'static str,
    content: impl Into<String>,
) -> MultipartPart {
    MultipartPart::File {
        name,
        file_name: Cow::Owned(file_name.into()),
        content_type,
        content: Cow::Owned(content.into()),
    }
}

fn multipart_body(parts: Vec<MultipartPart>) -> Vec<u8> {
    let mut body = Vec::new();

    for part in parts {
        body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}\r\n").as_bytes());

        match part {
            MultipartPart::Text { name, value } => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                );
                body.extend_from_slice(value.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
            MultipartPart::File {
                name,
                file_name,
                content_type,
                content,
            } => {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n"
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
                body.extend_from_slice(content.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
        }
    }

    body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}--\r\n").as_bytes());
    body
}

pub enum MultipartPart {
    Text {
        name: &'static str,
        value: Cow<'static, str>,
    },
    File {
        name: &'static str,
        file_name: Cow<'static, str>,
        content_type: &'static str,
        content: Cow<'static, str>,
    },
}
