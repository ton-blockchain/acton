use std::{
    ffi::OsString,
    fmt,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use tokio::process::Command;

use crate::{
    binaries::TonBinaries,
    runtime::{CommandOutput, run_checked},
    storage::{Layout, Manifest},
    storage::{NodeLayout, NodeSettings, Settings},
    ton::tools::{
        create_state::{CreateState, OfficialCreateState},
        dht_server::{DhtServer, OfficialDhtServer},
        fift::{Fift, FiftOutput, FiftScriptRequest, OfficialFift},
        lite_client::{LiteClient, NativeLiteClient},
        random_id::{OfficialRandomIdGenerator, RandomIdGenerator},
        validator_console::{OfficialValidatorConsole, ValidatorConsole, ValidatorConsoleEndpoint},
        validator_engine::{OfficialValidatorEngine, ValidatorEngine},
    },
};

/// Cloneable dependency bundle for every official TON program used by Localton
///
/// Workflows depend on semantic traits from this value and never construct argv
/// themselves. Every production adapter is built from the same validated release,
/// which prevents a node lifecycle from accidentally mixing incompatible tools
/// while still allowing tests to replace one boundary at a time
#[derive(Clone)]
pub struct Toolchain {
    pub layout: Layout,
    pub binaries: TonBinaries,
    lite_config: PathBuf,
    pub(crate) create_state: Arc<dyn CreateState>,
    pub(crate) dht_server: Arc<dyn DhtServer>,
    pub(crate) fift_tool: Arc<dyn Fift>,
    pub(crate) lite_client_tool: Arc<dyn LiteClient>,
    pub(crate) random_id: Arc<dyn RandomIdGenerator>,
    pub(crate) validator_engine: Arc<dyn ValidatorEngine>,
    pub(crate) validator_console_tool: Arc<dyn ValidatorConsole>,
}

impl fmt::Debug for Toolchain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Toolchain")
            .field("layout", &self.layout)
            .field("distribution_root", &self.binaries.root)
            .finish_non_exhaustive()
    }
}

impl Toolchain {
    /// Builds all production adapters from one already validated TON distribution
    ///
    /// The constructor is synchronous because binary installation and validation
    /// happen before this boundary. Adapters do not spawn processes until a
    /// semantic method is called
    #[must_use]
    pub fn official(layout: Layout, binaries: TonBinaries) -> Self {
        let lite_config = layout.global_config.clone();
        Self {
            layout,
            lite_config,
            create_state: Arc::new(OfficialCreateState::new(binaries.clone())),
            dht_server: Arc::new(OfficialDhtServer::new(binaries.clone())),
            fift_tool: Arc::new(OfficialFift::new(binaries.clone())),
            lite_client_tool: Arc::new(NativeLiteClient::new()),
            random_id: Arc::new(OfficialRandomIdGenerator::new(binaries.clone())),
            validator_engine: Arc::new(OfficialValidatorEngine::new(
                binaries.command("validator-engine"),
            )),
            validator_console_tool: Arc::new(OfficialValidatorConsole::new(
                binaries.command("validator-engine-console"),
            )),
            binaries,
        }
    }

    /// Resolves the pinned distribution and returns its semantic dependency bundle
    ///
    /// CLI operations use this entry point when no workflow-owned bundle already
    /// exists. Explicit release overrides are persisted exactly as before
    pub async fn resolve(state_dir: &Path, override_dir: Option<PathBuf>) -> Result<Self> {
        let root = absolute_path(state_dir)?;
        let layout = Layout::new(root);
        layout.create_dirs()?;
        let explicit_override = override_dir.is_some();
        let binaries = TonBinaries::resolve(&layout, override_dir).await?;
        if explicit_override && layout.manifest.is_file() {
            let mut manifest = Manifest::load(&layout.manifest)?;
            if manifest.ton_bin_dir != binaries.root {
                manifest.ton_bin_dir = binaries.root.clone();
                manifest.save_atomic(&layout.manifest)?;
            }
        }
        let toolchain = Self::official(layout, binaries);
        if toolchain.layout.settings.is_file() {
            let node_layout = toolchain.layout.node.clone();
            return Ok(toolchain.with_node_config(&node_layout));
        }

        Ok(toolchain)
    }

    pub fn settings(&self) -> Result<Settings> {
        Settings::load_or_create(&self.layout.settings)
    }

    /// Selects the global config used by liteserver client operations.
    ///
    /// Validator-engine keeps the config supplied during its own initialization;
    /// changing this path only affects subsequent client requests.
    #[must_use]
    pub(crate) fn with_node_config(mut self, node_layout: &NodeLayout) -> Self {
        self.lite_config = node_layout.global_config.clone();
        self
    }

    /// Returns the global config used by liteserver client operations.
    pub(crate) fn lite_config(&self) -> &Path {
        &self.lite_config
    }

    pub async fn lite_client(&self, command_text: &str) -> Result<String> {
        self.lite_client_commands(&[command_text]).await
    }

    pub async fn lite_client_commands(&self, commands: &[&str]) -> Result<String> {
        ensure!(!commands.is_empty(), "lite-client command list is empty");
        let mut command = Command::new(self.binaries.command("lite-client"));
        command.args(["-t", "15", "-C"]).arg(&self.lite_config);
        for command_text in commands {
            command.args(["-c", command_text]);
        }
        command.args(["-c", "quit"]);
        let output = run_checked(
            &format!("lite-client {}", commands.join(", ")),
            command,
            Duration::from_secs(30),
        )
        .await?;
        Ok(join_output(output))
    }

    /// Returns the authenticated host-local control endpoint for a managed node
    ///
    /// Administrative traffic stays on loopback even when the node advertises a
    /// LAN address. The console adapter still authenticates both sides with the
    /// node-specific client private key and server public key
    pub(crate) fn validator_console_endpoint(
        &self,
        layout: &NodeLayout,
        node: &NodeSettings,
    ) -> ValidatorConsoleEndpoint {
        ValidatorConsoleEndpoint {
            address: (Ipv4Addr::LOCALHOST, node.console_port).into(),
            client_private_key: layout.client_private_key(),
            server_public_key: layout.server_public_key(),
        }
    }

    /// Runs one selected Fift script without exposing interpreter flags to callers
    ///
    /// Script arguments remain opaque because individual workflows own their
    /// meaning. The adapter owns `-s`, `FIFTPATH`, subprocess lifecycle, and safe
    /// tracing so wallet and election code cannot drift from the pinned release
    pub async fn run_fift_script(
        &self,
        current_dir: &Path,
        script: PathBuf,
        arguments: Vec<OsString>,
        timeout: Duration,
    ) -> Result<FiftOutput> {
        self.fift_tool
            .run_script(
                &crate::ton::tools::types::OperationContext::new(timeout),
                FiftScriptRequest {
                    script,
                    arguments,
                    current_dir: current_dir.to_owned(),
                    include_paths: vec![self.layout.smartcont.clone()],
                },
            )
            .await
    }

    pub fn smartcont_script(&self, name: &str) -> PathBuf {
        let state_script = self.layout.smartcont.join(name);
        if state_script.is_file() {
            state_script
        } else {
            self.binaries.smartcont_dir().join(name)
        }
    }
}

pub fn join_output(output: CommandOutput) -> String {
    match (output.stdout.trim(), output.stderr.trim()) {
        ("", "") => String::new(),
        (stdout, "") => stdout.to_owned(),
        ("", stderr) => stderr.to_owned(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .context("failed to determine current directory")?
        .join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smartcont_script_prefers_state_override_and_falls_back_to_release() {
        let temp = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(temp.path().join("state"));
        layout.create_bootstrap_dirs().unwrap();
        let binaries = TonBinaries {
            root: temp.path().join("ton"),
        };
        std::fs::create_dir_all(binaries.smartcont_dir()).unwrap();
        let toolchain = Toolchain::official(layout, binaries);

        let release_script = toolchain.binaries.smartcont_dir().join("wallet.fif");
        std::fs::write(&release_script, "release").unwrap();
        assert_eq!(toolchain.smartcont_script("wallet.fif"), release_script);

        let state_script = toolchain.layout.smartcont.join("wallet.fif");
        std::fs::write(&state_script, "state").unwrap();
        assert_eq!(toolchain.smartcont_script("wallet.fif"), state_script);
    }
}
