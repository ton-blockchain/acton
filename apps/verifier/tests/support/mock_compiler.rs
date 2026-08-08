use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
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
        generated_sources: Vec<CompileGeneratedSource>,
        source_map: Option<SourceMapData>,
    },
    CompileFailed {
        error: String,
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
                generated_sources,
                source_map,
            } => Ok(CompileOutput {
                code_hash: code_hash.clone(),
                generated_sources: generated_sources.clone(),
                source_map: source_map.clone(),
            }),
            MockCompilerResult::CompileFailed { error } => {
                Err(CompilerError::CompileFailed(error.clone()))
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
                    generated_sources: Vec::new(),
                    source_map: None,
                })
            }
        }
    }
}
