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
    registry::SourceVerificationRegistry,
    registry_index::SqliteVerificationIndex,
    source_storage::{SharedSourceStorage, SourceMapData},
    state::AppState,
};

mod mock_blockchain;
mod mock_compiler;
mod mock_source_storage;

const MULTIPART_BOUNDARY: &str = "verifier-test-boundary";

pub fn app_state(code_hashes: &[(&str, &str)], compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = Arc::new(mock_source_storage::MockSourceStorage::confirmed());
    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        source_storage,
    )
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

pub fn failing_compiler_app_state(code_hashes: &[(&str, &str)], error: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::failing(error);

    app_state_from_parts(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
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
    let verification_index =
        Arc::new(SqliteVerificationIndex::in_memory().expect("SQLite index should open"));
    AppState::new(
        blockchain_client,
        compiler_service,
        Arc::new(SourceVerificationRegistry::new(
            source_storage,
            verification_index,
        )),
    )
}

pub async fn post_verify(state: AppState, parts: Vec<MultipartPart>) -> Response {
    let body = multipart_body(parts);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/verify")
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
        )
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
