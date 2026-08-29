use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use tokio::process::Command;

use crate::{
    binaries::TonBinaries,
    runtime::{CommandOutput, run_checked},
    storage::{Layout, Manifest, NodeLayout},
    storage::{NodeSettings, Settings},
};

#[derive(Debug, Clone)]
pub struct Toolchain {
    pub layout: Layout,
    pub binaries: TonBinaries,
}

impl Toolchain {
    pub async fn resolve(state_dir: &Path, override_dir: Option<PathBuf>) -> Result<Self> {
        let root = absolute_path(state_dir)?;
        let layout = Layout::new(root);
        layout.create_dirs()?;
        let explicit_override = override_dir.is_some();
        let binaries = TonBinaries::resolve(&layout, override_dir).await?;
        if explicit_override && layout.manifest.is_file() {
            let mut manifest = Manifest::load(&layout.manifest)?;
            if manifest.ton_bin_dir.as_ref() != Some(&binaries.root) {
                manifest.ton_bin_dir = Some(binaries.root.clone());
                manifest.save_atomic(&layout.manifest)?;
            }
        }
        Ok(Self { layout, binaries })
    }

    pub fn settings(&self) -> Result<Settings> {
        Settings::load_or_create(&self.layout.settings)
    }

    pub async fn lite_client(&self, command_text: &str) -> Result<String> {
        let mut command = Command::new(self.binaries.command("lite-client"));
        command
            .args(["-t", "15", "-C"])
            .arg(&self.layout.global_config)
            .args(["-c", command_text]);
        let output = run_checked(
            &format!("lite-client {command_text}"),
            command,
            Duration::from_secs(30),
        )
        .await?;
        Ok(join_output(output))
    }

    pub async fn validator_console(
        &self,
        node: &NodeSettings,
        command_text: &str,
    ) -> Result<String> {
        let node_layout = self.layout.node(node);
        self.validator_console_for_layout(node, &node_layout, command_text)
            .await
    }

    pub async fn validator_console_for_layout(
        &self,
        node: &NodeSettings,
        node_layout: &NodeLayout,
        command_text: &str,
    ) -> Result<String> {
        ensure!(
            node_layout.client_private_key().is_file(),
            "node {} has no console client certificate",
            node.name
        );
        ensure!(
            node_layout.server_public_key().is_file(),
            "node {} has no console server public key",
            node.name
        );
        let mut command = Command::new(self.binaries.command("validator-engine-console"));
        command
            .args(["-t", "10", "-k"])
            .arg(node_layout.client_private_key())
            .arg("-p")
            .arg(node_layout.server_public_key())
            .args([
                "-v",
                "0",
                "-a",
                &format!("{}:{}", node.public_ip, node.console_port),
                "-rc",
                command_text,
            ]);
        let output = run_checked(
            &format!("validator-engine-console {} {command_text}", node.name),
            command,
            Duration::from_secs(20),
        )
        .await?;
        Ok(join_output(output))
    }

    pub async fn fift<I, S>(&self, current_dir: &Path, args: I) -> Result<CommandOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(self.binaries.command("fift"));
        command
            .args(args)
            .current_dir(current_dir)
            .env("FIFTPATH", self.fift_path()?);
        run_checked("fift", command, Duration::from_secs(60)).await
    }

    pub fn fift_path(&self) -> Result<OsString> {
        std::env::join_paths([
            self.binaries.lib_dir(),
            self.binaries.smartcont_dir(),
            self.layout.smartcont.clone(),
        ])
        .context("failed to build FIFTPATH")
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
        let temp = tempfile::tempdir().unwrap();
        let layout = Layout::new(temp.path().join("state"));
        layout.create_dirs().unwrap();
        let binaries = TonBinaries {
            root: temp.path().join("ton"),
        };
        std::fs::create_dir_all(binaries.smartcont_dir()).unwrap();
        let toolchain = Toolchain { layout, binaries };

        let release_script = toolchain.binaries.smartcont_dir().join("wallet.fif");
        std::fs::write(&release_script, "release").unwrap();
        assert_eq!(toolchain.smartcont_script("wallet.fif"), release_script);

        let state_script = toolchain.layout.smartcont.join("wallet.fif");
        std::fs::write(&state_script, "state").unwrap();
        assert_eq!(toolchain.smartcont_script("wallet.fif"), state_script);
    }
}
