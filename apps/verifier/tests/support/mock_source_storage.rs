use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use verifier::source_storage::{
    SourceBundleManifest, SourceMapData, SourceStorage, SourceStorageError, SourceStorageReceipt,
    StoreSourceBundleRequest, StoredSourceBundle, StoredSourceFile,
};

const MOCK_VERIFIED_AT: u64 = 1_700_000_000;

pub struct MockSourceStorage {
    outcome: MockSourceStorageOutcome,
    recorded_requests: Arc<Mutex<Vec<RecordedSourceStorageRequest>>>,
    stored_bundles: Arc<Mutex<Vec<StoredSourceBundle>>>,
}

impl MockSourceStorage {
    pub fn confirmed() -> Self {
        Self {
            outcome: MockSourceStorageOutcome::Confirmed(SourceStorageReceipt {
                revision: "mock-revision".to_owned(),
                created: true,
            }),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            stored_bundles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: MockSourceStorageOutcome::Failed(message.to_owned()),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            stored_bundles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn recorded_requests(&self) -> Arc<Mutex<Vec<RecordedSourceStorageRequest>>> {
        Arc::clone(&self.recorded_requests)
    }
}

#[async_trait]
impl SourceStorage for MockSourceStorage {
    async fn store_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError> {
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded source storage requests mutex should not be poisoned");
            recorded_requests.push(RecordedSourceStorageRequest::from_request(&request));
        }

        match &self.outcome {
            MockSourceStorageOutcome::Confirmed(receipt) => {
                let mut stored_bundles = self
                    .stored_bundles
                    .lock()
                    .expect("stored source bundles mutex should not be poisoned");
                if stored_bundles
                    .iter()
                    .any(|stored| stored.manifest.code_hash == request.code_hash)
                {
                    return Ok(SourceStorageReceipt {
                        revision: receipt.revision.clone(),
                        created: false,
                    });
                }
                stored_bundles.push(stored_bundle_from_request(&request, receipt));
                drop(stored_bundles);
                Ok(receipt.clone())
            }
            MockSourceStorageOutcome::Failed(message) => {
                Err(SourceStorageError::Operation(message.clone()))
            }
        }
    }

    async fn load_bundle(
        &self,
        code_hash: &str,
    ) -> Result<Option<StoredSourceBundle>, SourceStorageError> {
        let stored_bundles = self
            .stored_bundles
            .lock()
            .expect("stored source bundles mutex should not be poisoned");
        Ok(stored_bundles
            .iter()
            .find(|bundle| bundle.manifest.code_hash == code_hash)
            .cloned())
    }

    async fn list_code_hashes(&self) -> Result<Vec<String>, SourceStorageError> {
        let mut code_hashes = {
            let stored_bundles = self
                .stored_bundles
                .lock()
                .expect("stored source bundles mutex should not be poisoned");
            stored_bundles
                .iter()
                .map(|bundle| bundle.manifest.code_hash.clone())
                .collect::<Vec<_>>()
        };
        code_hashes.sort();
        code_hashes.dedup();
        Ok(code_hashes)
    }

    async fn current_revision(&self) -> Result<Option<String>, SourceStorageError> {
        Ok(Some("mock-revision".to_owned()))
    }
}

#[derive(Clone)]
pub struct RecordedSourceStorageRequest {
    pub code_hash: String,
    pub source_bundle_hash: String,
    pub source_map: Option<SourceMapData>,
    pub files: Vec<(String, String)>,
}

impl RecordedSourceStorageRequest {
    fn from_request(request: &StoreSourceBundleRequest) -> Self {
        Self {
            code_hash: request.code_hash.clone(),
            source_bundle_hash: request.source_bundle_hash.clone(),
            source_map: request.source_map.clone(),
            files: request
                .files
                .iter()
                .map(|file| (file.path.clone(), file.content.clone()))
                .collect(),
        }
    }
}

enum MockSourceStorageOutcome {
    Confirmed(SourceStorageReceipt),
    Failed(String),
}

fn stored_bundle_from_request(
    request: &StoreSourceBundleRequest,
    receipt: &SourceStorageReceipt,
) -> StoredSourceBundle {
    let mut files = request
        .files
        .iter()
        .map(|file| {
            let content_hash = hex::encode(Sha256::digest(file.content.as_bytes()));
            StoredSourceFile {
                path: file.path.clone(),
                content_hash,
                content: file.content.clone(),
                include_in_command: file.include_in_command,
                is_stdlib: file.is_stdlib,
                has_include_directives: file.has_include_directives,
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));

    StoredSourceBundle {
        storage_revision: receipt.revision.clone(),
        manifest: SourceBundleManifest {
            code_hash: request.code_hash.clone(),
            source_bundle_hash: request.source_bundle_hash.clone(),
            verified_at: MOCK_VERIFIED_AT,
            compiler: request.compiler.clone(),
            source_map: request.source_map.clone(),
        },
        files,
    }
}
