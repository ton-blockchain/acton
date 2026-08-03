use std::{collections::BTreeMap, path::PathBuf, process::ExitStatus};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::{self, Duration},
};

use crate::{config::Config, source_storage::SourceMapData};

#[async_trait]
pub trait CompilerService: Send + Sync + 'static {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError>;
}

pub struct NodeCompilerService {
    node_bin: String,
    worker_path: PathBuf,
    timeout: Duration,
}

impl NodeCompilerService {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            node_bin: config.compiler_node_bin().to_owned(),
            worker_path: config.compiler_worker_path().to_path_buf(),
            timeout: config.compiler_timeout(),
        }
    }
}

#[async_trait]
impl CompilerService for NodeCompilerService {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError> {
        let input = serde_json::to_vec(&request).map_err(CompilerError::SerializeInput)?;
        let worker_path = dunce::canonicalize(&self.worker_path).map_err(|source| {
            CompilerError::ResolveWorkerPath {
                path: self.worker_path.clone(),
                source,
            }
        })?;
        let worker_directory = worker_path
            .parent()
            .ok_or_else(|| CompilerError::MissingWorkerDirectory(worker_path.clone()))?;
        let mut child = isolated_command(&self.node_bin)
            .arg("--permission")
            .arg("--disallow-code-generation-from-strings")
            .arg("--disable-proto=throw")
            .arg("--no-experimental-sqlite")
            .arg(format!("--allow-fs-read={}", worker_directory.display()))
            .arg(&worker_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(CompilerError::Spawn)?;

        let mut stdin = child.stdin.take().ok_or(CompilerError::MissingStdin)?;
        stdin
            .write_all(&input)
            .await
            .map_err(CompilerError::WriteStdin)?;
        drop(stdin);

        let output = time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| CompilerError::Timeout {
                timeout_ms: self.timeout.as_millis(),
            })?
            .map_err(CompilerError::Wait)?;

        if !output.status.success() {
            return Err(CompilerError::WorkerFailed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let output = serde_json::from_slice::<WorkerOutput>(&output.stdout)
            .map_err(CompilerError::DeserializeOutput)?;

        match output {
            WorkerOutput::Ok {
                code_hash,
                generated_sources,
                source_map,
            } => Ok(CompileOutput {
                code_hash,
                generated_sources,
                source_map,
            }),
            WorkerOutput::CompileError { error } => Err(CompilerError::CompileFailed(error)),
        }
    }
}

fn isolated_command(program: &str) -> Command {
    let program = PathBuf::from(program);
    let executable = std::env::var_os("PATH")
        .filter(|_| program.components().count() == 1)
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|directory| directory.join(&program))
                .find(|candidate| candidate.is_file())
        })
        .unwrap_or(program);
    let mut command = Command::new(executable);
    command.env_clear();
    command
}

#[derive(Debug, Serialize)]
pub struct CompileRequest {
    pub language: String,
    pub compiler_version: String,
    pub entrypoint: String,
    pub import_mappings: BTreeMap<String, String>,
    pub compile_params: Value,
    pub sources: Vec<CompileSource>,
}

#[derive(Debug, Serialize)]
pub struct CompileSource {
    pub path: String,
    pub content: String,
    pub is_entrypoint: bool,
    pub include_in_command: Option<bool>,
    pub is_stdlib: Option<bool>,
    pub has_include_directives: Option<bool>,
}

pub struct CompileOutput {
    pub code_hash: String,
    pub generated_sources: Vec<CompileGeneratedSource>,
    pub source_map: Option<SourceMapData>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompileGeneratedSource {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WorkerOutput {
    Ok {
        code_hash: String,
        #[serde(default)]
        generated_sources: Vec<CompileGeneratedSource>,
        source_map: Option<SourceMapData>,
    },
    CompileError {
        error: String,
    },
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("failed to serialize compiler input: {0}")]
    SerializeInput(serde_json::Error),
    #[error("failed to resolve compiler worker path {path}: {source}")]
    ResolveWorkerPath {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("compiler worker path has no parent directory: {0}")]
    MissingWorkerDirectory(PathBuf),
    #[error("failed to spawn compiler worker: {0}")]
    Spawn(std::io::Error),
    #[error("compiler worker stdin was not available")]
    MissingStdin,
    #[error("failed to write compiler worker stdin: {0}")]
    WriteStdin(std::io::Error),
    #[error("compiler worker timed out after {timeout_ms} ms")]
    Timeout { timeout_ms: u128 },
    #[error("failed to wait for compiler worker: {0}")]
    Wait(std::io::Error),
    #[error("compiler worker failed with status {status}: {stderr}")]
    WorkerFailed { status: ExitStatus, stderr: String },
    #[error("failed to parse compiler worker output: {0}")]
    DeserializeOutput(serde_json::Error),
    #[error("compile error: {0}")]
    CompileFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn isolated_command_resolves_executable_and_clears_environment() {
        let mut command = isolated_command("node");
        let executable = PathBuf::from(command.as_std().get_program());
        assert!(
            executable.is_file(),
            "resolved Node executable does not exist: {}",
            executable.display()
        );

        let output = command
            .arg("--eval")
            .arg("process.stdout.write(String(process.env.PATH === undefined))")
            .output()
            .await
            .expect("isolated Node command should run");

        assert!(
            output.status.success(),
            "isolated Node command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"true");
    }

    #[tokio::test]
    async fn node_permissions_allow_selected_reads_and_deny_other_filesystem_access() {
        let allowed_directory = tempfile::tempdir().expect("temporary directory should be created");
        let denied_directory = tempfile::tempdir().expect("temporary directory should be created");
        let allowed_file = allowed_directory.path().join("allowed.txt");
        let denied_file = denied_directory.path().join("denied.txt");
        let output_file = allowed_directory.path().join("output.txt");
        std::fs::write(&allowed_file, "allowed").expect("allowed fixture should be written");
        std::fs::write(&denied_file, "denied").expect("denied fixture should be written");

        let output = isolated_command("node")
            .arg("--permission")
            .arg(format!(
                "--allow-fs-read={}",
                allowed_directory.path().display()
            ))
            .arg("--eval")
            .arg(
                r#"
                    const fs = require("node:fs");
                    const [allowedPath, deniedPath, outputPath] = process.argv.slice(1);
                    const allowed = fs.readFileSync(allowedPath, "utf8");
                    let denied;
                    try {
                      fs.readFileSync(deniedPath, "utf8");
                    } catch (error) {
                      denied = error.code;
                    }
                    let write;
                    try {
                      fs.writeFileSync(outputPath, "output");
                    } catch (error) {
                      write = error.code;
                    }
                    process.stdout.write(`${allowed}:${denied}:${write}`);
                "#,
            )
            .arg(&allowed_file)
            .arg(&denied_file)
            .arg(&output_file)
            .output()
            .await
            .expect("isolated Node command should run");

        assert!(
            output.status.success(),
            "isolated Node command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"allowed:ERR_ACCESS_DENIED:ERR_ACCESS_DENIED"
        );
        assert!(!output_file.exists());
    }

    #[tokio::test]
    async fn node_disallows_code_generation_from_strings() {
        let output = isolated_command("node")
            .arg("--disallow-code-generation-from-strings")
            .arg("--eval")
            .arg(
                r#"
                    try {
                      eval("1");
                    } catch (error) {
                      process.stdout.write(error.name);
                    }
                "#,
            )
            .output()
            .await
            .expect("isolated Node command should run");

        assert!(
            output.status.success(),
            "isolated Node command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"EvalError");
    }

    #[tokio::test]
    async fn node_disables_proto_and_experimental_sqlite() {
        let output = isolated_command("node")
            .arg("--disable-proto=throw")
            .arg("--no-experimental-sqlite")
            .arg("--eval")
            .arg(
                r#"
                    let protoError;
                    try {
                      ({}).__proto__;
                    } catch (error) {
                      protoError = error.code;
                    }
                    let sqliteError;
                    try {
                      require("node:sqlite");
                    } catch (error) {
                      sqliteError = error.code;
                    }
                    process.stdout.write(`${protoError}:${sqliteError}`);
                "#,
            )
            .output()
            .await
            .expect("isolated Node command should run");

        assert!(
            output.status.success(),
            "isolated Node command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"ERR_PROTO_ACCESS:ERR_UNKNOWN_BUILTIN_MODULE"
        );
    }
}
