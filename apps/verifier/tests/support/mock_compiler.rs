use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use tokio::sync::Notify;
use verifier::compilers::{
    CompileGeneratedSource, CompileOutput, CompileRequest, CompilerError, CompilerService,
};
use verifier::source_storage::SourceMapData;

pub struct MockCompilerService {
    result: MockCompilerResult,
    recorded_requests: Arc<Mutex<Vec<CompileRequest>>>,
}

impl MockCompilerService {
    pub fn new(code_hash: &str) -> Self {
        Self {
            result: MockCompilerResult::Ok {
                code_hash: code_hash.to_owned(),
                used_source_paths: None,
                generated_sources: Vec::new(),
                source_map: None,
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_generated_sources(
        code_hash: &str,
        generated_sources: Vec<CompileGeneratedSource>,
    ) -> Self {
        Self {
            result: MockCompilerResult::Ok {
                code_hash: code_hash.to_owned(),
                used_source_paths: None,
                generated_sources,
                source_map: None,
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_source_map_data(code_hash: &str, source_map: SourceMapData) -> Self {
        Self {
            result: MockCompilerResult::Ok {
                code_hash: code_hash.to_owned(),
                used_source_paths: None,
                generated_sources: Vec::new(),
                source_map: Some(source_map),
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(error: &str) -> Self {
        Self {
            result: MockCompilerResult::CompileFailed {
                error: error.to_owned(),
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn timing_out(timeout_ms: u128) -> Self {
        Self {
            result: MockCompilerResult::Timeout { timeout_ms },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn blocking(code_hash: &str) -> (Self, Arc<Notify>, Arc<Notify>) {
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        (
            Self {
                result: MockCompilerResult::Blocked {
                    code_hash: code_hash.to_owned(),
                    started: Arc::clone(&started),
                    release: Arc::clone(&release),
                },
                recorded_requests: Arc::new(Mutex::new(Vec::new())),
            },
            started,
            release,
        )
    }

    pub fn with_used_source_paths(code_hash: &str, used_source_paths: Vec<String>) -> Self {
        Self {
            result: MockCompilerResult::Ok {
                code_hash: code_hash.to_owned(),
                used_source_paths: Some(used_source_paths),
                generated_sources: Vec::new(),
                source_map: None,
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn by_compiler(compilers: &[(&str, &str, &str)]) -> Self {
        Self {
            result: MockCompilerResult::ByCompiler(
                compilers
                    .iter()
                    .map(|(language, version, code_hash)| {
                        (
                            ((*language).to_owned(), (*version).to_owned()),
                            (*code_hash).to_owned(),
                        )
                    })
                    .collect(),
            ),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn recorded_requests(&self) -> Arc<Mutex<Vec<CompileRequest>>> {
        Arc::clone(&self.recorded_requests)
    }
}

enum MockCompilerResult {
    Ok {
        code_hash: String,
        used_source_paths: Option<Vec<String>>,
        generated_sources: Vec<CompileGeneratedSource>,
        source_map: Option<SourceMapData>,
    },
    CompileFailed {
        error: String,
    },
    Timeout {
        timeout_ms: u128,
    },
    Blocked {
        code_hash: String,
        started: Arc<Notify>,
        release: Arc<Notify>,
    },
    ByCompiler(BTreeMap<(String, String), String>),
}

#[async_trait]
impl CompilerService for MockCompilerService {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError> {
        let compiler = (request.language.clone(), request.compiler_version.clone());
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded compiler requests mutex should not be poisoned");
            recorded_requests.push(request);
        }

        match &self.result {
            MockCompilerResult::Ok {
                code_hash,
                used_source_paths,
                generated_sources,
                source_map,
            } => Ok(CompileOutput {
                code_hash: code_hash.clone(),
                used_source_paths: used_source_paths.clone(),
                generated_sources: generated_sources.clone(),
                source_map: source_map.clone(),
            }),
            MockCompilerResult::CompileFailed { error } => {
                Err(CompilerError::CompileFailed(error.clone()))
            }
            MockCompilerResult::Timeout { timeout_ms } => Err(CompilerError::Timeout {
                timeout_ms: *timeout_ms,
            }),
            MockCompilerResult::Blocked {
                code_hash,
                started,
                release,
            } => {
                started.notify_one();
                release.notified().await;
                Ok(CompileOutput {
                    code_hash: code_hash.clone(),
                    used_source_paths: None,
                    generated_sources: Vec::new(),
                    source_map: None,
                })
            }
            MockCompilerResult::ByCompiler(code_hashes) => {
                let code_hash = code_hashes.get(&compiler).ok_or_else(|| {
                    CompilerError::CompileFailed(format!(
                        "no mock result for compiler {} {}",
                        compiler.0, compiler.1
                    ))
                })?;
                Ok(CompileOutput {
                    code_hash: code_hash.clone(),
                    used_source_paths: None,
                    generated_sources: Vec::new(),
                    source_map: None,
                })
            }
        }
    }
}
