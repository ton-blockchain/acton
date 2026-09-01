//! Typed access to the official `create-state` program.
//!
//! Genesis bootstrap owns the generated Fift script and the order in which
//! chain states are created. This module owns the pinned program's execution
//! contract and proves that its promised BoC and hash sidecars exist before a
//! workflow can publish them.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::process::Command;
use tracing::{Instrument, Span, field::Empty, info, info_span, warn};

use crate::{binaries::TonBinaries, runtime::run_checked};

use super::types::{OperationContext, TonBlockHash, ZeroStateId};

/// Selects the filename contract used by the official genesis scripts.
///
/// The masterchain and basechain scripts write different fixed artifact names.
/// Keeping the distinction typed prevents a successful basechain invocation
/// from being mistaken for the masterchain zerostate later in bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroStateKind {
    /// The masterchain state written as `zerostate.*`.
    Masterchain,
    /// The initial workchain 0 state written as `basestate0.*`.
    Basechain,
}

impl ZeroStateKind {
    fn artifact_stem(self) -> &'static str {
        match self {
            Self::Masterchain => "zerostate",
            Self::Basechain => "basestate0",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Masterchain => "masterchain",
            Self::Basechain => "basechain",
        }
    }
}

/// Describes one state-generation operation without exposing CLI syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStateRequest {
    /// Determines which fixed artifact set must exist after the script exits.
    pub kind: ZeroStateKind,
    /// Generated Fift script that defines the initial state.
    pub script: PathBuf,
    /// Working directory where `create-state` writes its artifact set.
    pub output_dir: PathBuf,
    /// Script and library directories appended to the adapter-owned `FIFTPATH`.
    pub include_paths: Vec<PathBuf>,
}

/// A complete zerostate identity ready for global config and static storage.
///
/// TON addresses an initial state with both its cell representation hash and
/// the SHA-256 hash of the serialized BoC. Returning both values together with
/// the BoC path makes it impossible for callers to accept only a partial output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroStateArtifacts {
    /// Generated Bag of Cells containing the serialized initial state.
    boc: PathBuf,
    /// Root and serialized-file hashes that must always describe this exact BoC.
    id: ZeroStateId,
}

impl ZeroStateArtifacts {
    /// Loads the fixed output set for one official genesis script.
    ///
    /// The combined masterchain script creates a basechain state as a side
    /// effect, so the workflow must sometimes load an artifact that was not the
    /// direct return value of its `create-state` invocation. Filename knowledge
    /// remains here at the binary boundary.
    pub fn load(kind: ZeroStateKind, output_dir: &Path) -> Result<Self> {
        let stem = kind.artifact_stem();
        let boc = output_dir.join(format!("{stem}.boc"));
        let boc_metadata = fs::metadata(&boc)
            .with_context(|| format!("create-state did not produce {}", boc.display()))?;
        ensure!(boc_metadata.is_file(), "{} is not a file", boc.display());
        ensure!(
            boc_metadata.len() > 0,
            "create-state produced an empty BoC: {}",
            boc.display()
        );

        Ok(Self {
            boc,
            id: ZeroStateId::new(
                read_hash(&output_dir.join(format!("{stem}.rhash")))?,
                read_hash(&output_dir.join(format!("{stem}.fhash")))?,
            ),
        })
    }

    /// Returns the BoC path without exposing its release-defined filename.
    pub fn boc_path(&self) -> &Path {
        &self.boc
    }

    /// Returns the inseparable root/file hash pair used by global config.
    pub const fn id(&self) -> ZeroStateId {
        self.id
    }

    /// Replaces a workflow-modified BoC and updates both hash sidecars together.
    ///
    /// Localton adjusts the masterchain global balance after separate basechain
    /// generation. That changes the BoC and both hashes; returning a new value
    /// prevents the caller from retaining the stale identity returned by the
    /// original `create-state` invocation.
    pub fn replace_boc(&self, bytes: &[u8], root_hash: TonBlockHash) -> Result<Self> {
        let file_hash = TonBlockHash::from_bytes(Sha256::digest(bytes).into());
        fs::write(&self.boc, bytes)
            .with_context(|| format!("failed to write {}", self.boc.display()))?;
        fs::write(self.boc.with_extension("rhash"), root_hash.as_bytes())?;
        fs::write(self.boc.with_extension("fhash"), file_hash.as_bytes())?;
        Ok(Self {
            boc: self.boc.clone(),
            id: ZeroStateId::new(root_hash, file_hash),
        })
    }

    /// Installs the BoC where validator-engine looks up its initial static state.
    ///
    /// Static storage is keyed by the uppercase file hash rather than the source
    /// filename. Keeping the copy rule on the artifact prevents a caller from
    /// pairing a BoC with some other hash or spelling the filename differently.
    pub fn install_for_validator_engine(&self, validator_db: &Path) -> Result<()> {
        let static_dir = validator_db.join("static");
        fs::create_dir_all(&static_dir)?;
        let destination = static_dir.join(self.id.file_hash().to_static_state_filename());
        fs::copy(&self.boc, &destination).with_context(|| {
            format!(
                "failed to install {} as validator static state {}",
                self.boc.display(),
                destination.display()
            )
        })?;
        Ok(())
    }
}

/// Creates and validates one TON zerostate artifact set.
///
/// The interface deliberately starts at a generated script rather than trying
/// to model arbitrary Fift source. Script generation remains a Localton
/// workflow concern, while release-specific execution and output validation
/// remain replaceable behind this trait.
#[async_trait]
pub trait CreateState: Send + Sync {
    /// Executes the state script and returns artifacts only after all three
    /// binary-owned outputs have passed structural validation.
    async fn create(
        &self,
        context: &OperationContext,
        request: CreateStateRequest,
    ) -> Result<ZeroStateArtifacts>;
}

/// Production `CreateState` implementation backed by the pinned TON release.
#[derive(Debug, Clone)]
pub struct OfficialCreateState {
    binaries: TonBinaries,
}

impl OfficialCreateState {
    /// Binds the adapter to one already validated, immutable TON distribution.
    pub fn new(binaries: TonBinaries) -> Self {
        Self { binaries }
    }

    // `create-state` resolves its standard Fift words from the release `lib`
    // directory, while network-specific scripts and copied smart contracts are
    // supplied by the workflow. Their order is intentional: it reproduces the
    // current genesis bootstrap's FIFTPATH without making the workflow join an
    // OS-specific path-list string.
    fn command(&self, request: &CreateStateRequest) -> Result<Command> {
        let fift_path = std::env::join_paths(
            std::iter::once(self.binaries.lib_dir()).chain(request.include_paths.iter().cloned()),
        )
        .context("failed to build create-state FIFTPATH")?;
        let mut command = Command::new(self.binaries.command("create-state"));
        command
            .arg(&request.script)
            .current_dir(&request.output_dir)
            .env("FIFTPATH", fift_path);
        Ok(command)
    }
}

#[async_trait]
impl CreateState for OfficialCreateState {
    async fn create(
        &self,
        context: &OperationContext,
        request: CreateStateRequest,
    ) -> Result<ZeroStateArtifacts> {
        let started = Instant::now();
        let span = operation_span(context);
        let kind = request.kind.as_str();

        let result = async {
            validate_request(&request)?;
            info!(
                milestone = "artifact_generation_started",
                %kind,
                output_dir = %request.output_dir.display(),
                "creating TON zerostate artifacts"
            );
            let output = run_checked(
                "create-state create zerostate",
                self.command(&request)?,
                context.timeout,
            )
            .await?;
            if !output.stderr.trim().is_empty() {
                // Some official releases print progress on stderr even when the
                // operation succeeds. Record that diagnostics exist, but never
                // promote tool output to tracing fields.
                warn!(
                    milestone = "stderr_diagnostics",
                    "create-state produced stderr diagnostics"
                );
            }
            ZeroStateArtifacts::load(request.kind, &request.output_dir)
        }
        .instrument(span.clone())
        .await;

        if let Ok(artifacts) = &result {
            span.in_scope(|| {
                info!(
                    milestone = "artifacts_validated",
                    artifact_boc = %artifacts.boc_path().display(),
                    artifact_root_hash = %artifacts.id().root_hash(),
                    artifact_file_hash = %artifacts.id().file_hash(),
                    "TON zerostate artifacts are ready"
                );
            });
        }
        finish_operation(&span, started, &result);
        result
    }
}

// Fail before spawning so a missing script or include directory is reported as
// a Localton input error rather than an opaque Fift interpreter failure.
fn validate_request(request: &CreateStateRequest) -> Result<()> {
    ensure!(
        request.script.is_file(),
        "create-state script does not exist: {}",
        request.script.display()
    );
    ensure!(
        request.output_dir.is_dir(),
        "create-state output directory does not exist: {}",
        request.output_dir.display()
    );
    for include_path in &request.include_paths {
        ensure!(
            include_path.is_dir(),
            "create-state include directory does not exist: {}",
            include_path.display()
        );
    }
    Ok(())
}

// Sidecars are raw hash bytes even though many downstream APIs render them as
// hex or base64. Parse once at the tool boundary to reject text-encoded hashes.
fn read_hash(path: &Path) -> Result<TonBlockHash> {
    let bytes = fs::read(path)
        .with_context(|| format!("create-state did not produce {}", path.display()))?;
    let actual = bytes.len();
    let bytes = bytes.try_into().map_err(|_| {
        anyhow::anyhow!(
            "create-state hash {} must contain exactly 32 bytes, found {actual}",
            path.display()
        )
    })?;
    Ok(TonBlockHash::from_bytes(bytes))
}

// A stable span schema lets diagnostics aggregate all official tool calls by
// tool, operation, and node without inferring semantics from subprocess labels.
fn operation_span(context: &OperationContext) -> Span {
    info_span!(
        "ton_tool_operation",
        ton.tool = "create-state",
        operation = "create",
        node = context.node_name.as_deref().unwrap_or("network"),
        outcome = Empty,
        duration_ms = Empty,
    )
}

// Record completion on the operation span once, including validation failures
// that happen after the child itself exited successfully.
fn finish_operation<T>(span: &Span, started: Instant, result: &Result<T>) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    span.in_scope(|| match result {
        Ok(_) => info!(duration_ms, outcome, "TON tool operation completed"),
        // The returned error retains subprocess diagnostics for the caller.
        // Avoid duplicating them into tracing because Fift-originated output can
        // contain payload material even when `create-state` is the front-end.
        Err(_) => warn!(duration_ms, outcome, "TON tool operation failed"),
    });
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, fs};

    use expect_test::expect;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn basechain_command_uses_typed_script_and_fift_path() {
        let adapter = OfficialCreateState::new(TonBinaries {
            root: PathBuf::from("/ton/bin"),
        });
        let request = CreateStateRequest {
            kind: ZeroStateKind::Basechain,
            script: PathBuf::from("/network/smartcont/gen-basestate.fif"),
            output_dir: PathBuf::from("/network/zerostate"),
            include_paths: vec![PathBuf::from("/network/smartcont")],
        };

        let command = adapter.command(&request).unwrap();

        expect![[r#"
            program=/ton/bin/create-state
            current_dir=/network/zerostate
            args=/network/smartcont/gen-basestate.fif
            FIFTPATH=/ton/bin/lib:/network/smartcont"#]]
        .assert_eq(&command_snapshot(&command));
    }

    #[test]
    fn artifact_loader_returns_binary_hashes_as_one_complete_identity() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("zerostate.boc"),
            [0xb5, 0xee, 0x9c, 0x72],
        )
        .unwrap();
        fs::write(directory.path().join("zerostate.rhash"), [0x11; 32]).unwrap();
        fs::write(directory.path().join("zerostate.fhash"), [0xab; 32]).unwrap();

        let artifacts =
            ZeroStateArtifacts::load(ZeroStateKind::Masterchain, directory.path()).unwrap();
        let validator_db = directory.path().join("validator-db");
        artifacts
            .install_for_validator_engine(&validator_db)
            .unwrap();
        let installed_name = fs::read_dir(validator_db.join("static"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .into_owned();
        let summary = format!(
            "boc={}\nroot={}\nfile={}\nstatic={installed_name}",
            artifacts.boc_path().file_name().unwrap().to_string_lossy(),
            artifacts.id().root_hash(),
            artifacts.id().file_hash()
        );

        expect![[r#"
            boc=zerostate.boc
            root=1111111111111111111111111111111111111111111111111111111111111111
            file=abababababababababababababababababababababababababababababababab
            static=ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB"#]]
        .assert_eq(&summary);
    }

    #[test]
    fn artifact_loader_rejects_non_binary_hash_sidecars() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("basestate0.boc"), [1]).unwrap();
        fs::write(directory.path().join("basestate0.rhash"), "00").unwrap();
        fs::write(directory.path().join("basestate0.fhash"), [0; 32]).unwrap();

        let error =
            ZeroStateArtifacts::load(ZeroStateKind::Basechain, directory.path()).unwrap_err();

        expect![[
            r#"create-state hash <tmp>/basestate0.rhash must contain exactly 32 bytes, found 2"#
        ]]
        .assert_eq(
            &error
                .to_string()
                .replace(&directory.path().display().to_string(), "<tmp>"),
        );
    }

    #[test]
    fn replacing_a_boc_returns_the_new_complete_identity() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("zerostate.boc"), [1]).unwrap();
        fs::write(directory.path().join("zerostate.rhash"), [0x11; 32]).unwrap();
        fs::write(directory.path().join("zerostate.fhash"), [0x22; 32]).unwrap();
        let original =
            ZeroStateArtifacts::load(ZeroStateKind::Masterchain, directory.path()).unwrap();

        let updated = original
            .replace_boc(&[9, 8, 7], TonBlockHash::from_bytes([0x44; 32]))
            .unwrap();
        let root_sidecar = fs::read(directory.path().join("zerostate.rhash")).unwrap();
        let file_sidecar = fs::read(directory.path().join("zerostate.fhash")).unwrap();
        let summary = format!(
            "boc={:?}\nroot={}\nfile_changed={}\nroot_sidecar_matches={}\nfile_sidecar_matches={}",
            fs::read(updated.boc_path()).unwrap(),
            updated.id().root_hash(),
            updated.id().file_hash() != original.id().file_hash(),
            root_sidecar == updated.id().root_hash().as_bytes(),
            file_sidecar == updated.id().file_hash().as_bytes(),
        );

        expect![[r#"
            boc=[9, 8, 7]
            root=4444444444444444444444444444444444444444444444444444444444444444
            file_changed=true
            root_sidecar_matches=true
            file_sidecar_matches=true"#]]
        .assert_eq(&summary);
    }

    fn command_snapshot(command: &Command) -> String {
        let command = command.as_std();
        let args = command
            .get_args()
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ");
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
