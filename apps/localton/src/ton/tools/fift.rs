//! Typed script execution through the official Fift interpreter.
//!
//! Fift is intentionally modeled as a script interpreter rather than as a
//! collection of wallet-specific operations. Wallet and validator workflows
//! choose scripts and interpret their artifacts; this adapter owns only the
//! release CLI, include-path environment, bounded execution, and raw result.

use std::{ffi::OsString, path::PathBuf, time::Instant};

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::{Instrument, Span, field::Empty, info, info_span, warn};

use crate::{binaries::TonBinaries, runtime::run_checked};

use super::types::OperationContext;

/// One explicit Fift script execution without exposing positional CLI details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiftScriptRequest {
    /// Script interpreted as the first Fift program argument.
    pub script: PathBuf,
    /// Script-owned parameters kept opaque to the adapter and its tracing.
    pub arguments: Vec<OsString>,
    /// Working directory used for scripts that read or create relative files.
    pub current_dir: PathBuf,
    /// Network-specific include directories appended to release resources.
    pub include_paths: Vec<PathBuf>,
}

/// Captured Fift output returned to the workflow that understands the script.
///
/// Output can contain serialized messages or signing diagnostics. It is
/// therefore returned as data and deliberately never copied into tracing fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiftOutput {
    /// Complete UTF-8-lossy standard output from the interpreter.
    pub stdout: String,
    /// Complete UTF-8-lossy standard error from the interpreter.
    pub stderr: String,
}

/// Runs Fift scripts behind a replaceable, object-safe interface.
///
/// The semantic boundary is one script because Fift scripts define many
/// unrelated TON operations. Higher-level code must still provide explicit
/// script paths and typed post-processing instead of regaining a raw shell API.
#[async_trait]
pub trait Fift: Send + Sync {
    /// Interprets a script under the operation deadline and returns captured
    /// output without logging arguments or output that may contain secrets.
    async fn run_script(
        &self,
        context: &OperationContext,
        request: FiftScriptRequest,
    ) -> Result<FiftOutput>;
}

/// Production `Fift` implementation backed by the pinned TON release.
#[derive(Debug, Clone)]
pub struct OfficialFift {
    binaries: TonBinaries,
}

impl OfficialFift {
    /// Binds the interpreter adapter to one validated TON distribution so
    /// workflows do not depend directly on release lookup or executable paths.
    pub fn new(binaries: TonBinaries) -> Self {
        Self { binaries }
    }

    // Release libraries precede workflow paths to preserve the existing
    // Toolchain contract. A workflow can still select a generated/overridden
    // script directly through `request.script`; include paths are for words that
    // script loads transitively, not for locating the entry script itself.
    fn command(&self, request: &FiftScriptRequest) -> Result<Command> {
        let fift_path = std::env::join_paths(
            [self.binaries.lib_dir(), self.binaries.smartcont_dir()]
                .into_iter()
                .chain(request.include_paths.iter().cloned()),
        )
        .context("failed to build Fift FIFTPATH")?;
        let mut command = Command::new(self.binaries.command("fift"));
        command
            // `-s` selects script mode in the official interpreter. It must stay
            // adapter-owned because omitting it changes how positional arguments
            // and process exit status are handled by Fift.
            .arg("-s")
            .arg(&request.script)
            .args(&request.arguments)
            .current_dir(&request.current_dir)
            .env("FIFTPATH", fift_path);
        Ok(command)
    }
}

#[async_trait]
impl Fift for OfficialFift {
    async fn run_script(
        &self,
        context: &OperationContext,
        request: FiftScriptRequest,
    ) -> Result<FiftOutput> {
        let started = Instant::now();
        let span = operation_span(context, &request.script);

        let result = async {
            validate_request(&request)?;
            info!(
                milestone = "script_started",
                script = %request.script.display(),
                "running Fift script"
            );
            run_checked("fift run script", self.command(&request)?, context.timeout)
                .await
                .map(|output| FiftOutput {
                    stdout: output.stdout,
                    stderr: output.stderr,
                })
        }
        .instrument(span.clone())
        .await;

        if result.is_ok() {
            span.in_scope(|| {
                info!(
                    milestone = "script_completed",
                    script = %request.script.display(),
                    "Fift script completed"
                );
            });
        }
        finish_operation(&span, started, &result);
        result
    }
}

// Input validation keeps path-setup failures distinct from interpreter errors.
// Arguments remain opaque because some scripts accept signed messages, private
// material, or payloads whose safety the generic Fift adapter cannot determine.
fn validate_request(request: &FiftScriptRequest) -> Result<()> {
    ensure!(
        request.script.is_file(),
        "Fift script does not exist: {}",
        request.script.display()
    );
    ensure!(
        request.current_dir.is_dir(),
        "Fift working directory does not exist: {}",
        request.current_dir.display()
    );
    for include_path in &request.include_paths {
        ensure!(
            include_path.is_dir(),
            "Fift include directory does not exist: {}",
            include_path.display()
        );
    }
    Ok(())
}

// All Fift calls share one semantic span schema. The script path is safe
// operational context, while arguments and captured output remain deliberately
// absent because wallet scripts can carry signing payloads through either.
fn operation_span(context: &OperationContext, script: &std::path::Path) -> Span {
    info_span!(
        "ton_tool_operation",
        ton.tool = "fift",
        operation = "run_script",
        node = context.node_name.as_deref().unwrap_or("network"),
        script = %script.display(),
        outcome = Empty,
        duration_ms = Empty,
    )
}

// Finish after output capture so subprocess failures and typed adapter failures
// have the same duration/outcome fields for monitoring and alerting.
fn finish_operation<T>(span: &Span, started: Instant, result: &Result<T>) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    span.in_scope(|| match result {
        Ok(_) => info!(duration_ms, outcome, "TON tool operation completed"),
        // `run_checked` returns rich diagnostics to the caller. Do not mirror
        // that error into tracing because stdout/stderr may contain signed data.
        Err(_) => warn!(duration_ms, outcome, "TON tool operation failed"),
    });
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use expect_test::expect;

    use super::*;

    #[test]
    fn command_keeps_script_separate_from_opaque_arguments() {
        let adapter = OfficialFift::new(TonBinaries {
            root: PathBuf::from("/ton/bin"),
        });
        let request = FiftScriptRequest {
            script: PathBuf::from("/network/smartcont/validator-elect-req.fif"),
            arguments: vec![OsString::from("payload.boc"), OsString::from("42")],
            current_dir: PathBuf::from("/network/work"),
            include_paths: vec![PathBuf::from("/network/smartcont")],
        };

        let command = adapter.command(&request).unwrap();

        expect![[r#"
            program=/ton/bin/fift
            current_dir=/network/work
            args=-s|/network/smartcont/validator-elect-req.fif|payload.boc|42
            FIFTPATH=/ton/bin/lib:/ton/bin/smartcont:/network/smartcont"#]]
        .assert_eq(&command_snapshot(&command));
    }

    fn command_snapshot(command: &Command) -> String {
        let command = command.as_std();
        let args = command
            .get_args()
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join("|");
        let fift_path = command
            .get_envs()
            .find_map(|(name, value)| {
                if name == "FIFTPATH" {
                    value.map(OsStr::to_string_lossy)
                } else {
                    None
                }
            })
            .unwrap();
        format!(
            "program={}\ncurrent_dir={}\nargs={}\nFIFTPATH={}",
            command.get_program().to_string_lossy(),
            command.get_current_dir().unwrap().display(),
            args,
            fift_path
        )
    }
}
