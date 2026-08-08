use std::fs::{File, OpenOptions};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, manifest_path as configured_manifest_path, project_root as configured_project_root,
};
use acton_studio::{
    ContractRegistryStore, LocalProcessEnvironmentRuntime, LocalProcessTestRunRuntime,
    PUBLIC_TON_ENVIRONMENT_IDS, STUDIO_API_VERSION, StudioDaemonDescriptor, StudioServer,
    StudioServerConfig, StudioWorkspace, persist_studio_daemon_descriptor,
    remove_studio_daemon_descriptor, studio_daemon_descriptor_path,
};
use anyhow::Context;
use fs2::FileExt;

use crate::studio_wallets::ProjectWalletRuntime;

pub async fn studio_start_cmd(host: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()> {
    if !host.is_loopback() {
        anyhow::bail!(
            "Acton Studio requires a loopback host until remote authentication is configured"
        );
    }
    let configured_project = configured_project()?;

    let address = SocketAddr::new(host, port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| {
            anyhow::Error::new(source).context(format!(
                "Failed to start Acton Studio on {address}\nSet another port with --port\nOr stop the process currently listening on that port"
            ))
        })?;
    let address = listener
        .local_addr()
        .context("Failed to inspect Studio server address")?;
    let url = format!("http://{address}");

    let mut config = StudioServerConfig::new(crate::build_info::SHORT_VERSION);
    if let Some((workspace, _)) = &configured_project {
        let workspace = workspace.clone();
        config = config.with_workspace(workspace);
    }
    let acton_executable =
        std::env::current_exe().context("Failed to locate the Acton executable")?;
    let project_root = configured_project_root().to_path_buf();
    let contract_registry = ContractRegistryStore::for_project(&project_root);
    let environment_runtime = LocalProcessEnvironmentRuntime::open(
        acton_executable.clone(),
        &project_root,
        contract_registry.clone(),
        PUBLIC_TON_ENVIRONMENT_IDS
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
    )
    .await?;
    let reporter_url = local_reporter_url(address);
    let test_run_runtime =
        LocalProcessTestRunRuntime::new(acton_executable, &project_root, &reporter_url);
    let mut server = StudioServer::new(config)
        .with_environment_runtime(environment_runtime)
        .with_contract_registry(contract_registry)
        .with_test_run_runtime(test_run_runtime);
    if let Some((_, wallet_runtime)) = configured_project {
        server = server.with_wallet_runtime(wallet_runtime);
    }
    let daemon_guard = StudioDaemonGuard::register(&project_root, reporter_url)?;

    println!("    {} Acton Studio at {}", "Starting".green().bold(), url);

    if open_browser
        && host.is_loopback()
        && let Err(error) = opener::open(&url)
    {
        eprintln!("Warning: Failed to open Acton Studio at {url}: {error}");
    }

    let result = server.serve(listener, shutdown_signal()).await;
    drop(daemon_guard);
    result?;
    Ok(())
}

fn local_reporter_url(address: SocketAddr) -> String {
    format!("http://{address}")
}

#[derive(Debug)]
struct StudioDaemonGuard {
    project_root: PathBuf,
    pid: u32,
    _lock_file: File,
}

impl StudioDaemonGuard {
    fn register(project_root: &Path, url: String) -> anyhow::Result<Self> {
        let descriptor_path = studio_daemon_descriptor_path(project_root);
        let descriptor_directory = descriptor_path
            .parent()
            .context("Studio daemon path has no parent directory")?;
        std::fs::create_dir_all(descriptor_directory)
            .context("Failed to create the Studio daemon directory")?;
        let lock_path = descriptor_path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("Failed to open Studio lock at {}", lock_path.display()))?;
        FileExt::try_lock_exclusive(&lock_file).with_context(|| {
            format!(
                "Another Acton Studio instance is already running for {}",
                project_root.display()
            )
        })?;

        let pid = std::process::id();
        persist_studio_daemon_descriptor(
            project_root,
            &StudioDaemonDescriptor {
                protocol_version: STUDIO_API_VERSION,
                url,
                pid,
            },
        )
        .context("Failed to publish the Studio daemon descriptor")?;
        Ok(Self {
            project_root: project_root.to_path_buf(),
            pid,
            _lock_file: lock_file,
        })
    }
}

impl Drop for StudioDaemonGuard {
    fn drop(&mut self) {
        if let Err(error) = remove_studio_daemon_descriptor(&self.project_root, self.pid) {
            log::warn!("Failed to remove the Studio daemon descriptor: {error}");
        }
    }
}

fn configured_project() -> anyhow::Result<Option<(StudioWorkspace, ProjectWalletRuntime)>> {
    if !configured_manifest_path().is_file() {
        return Ok(None);
    }

    let config = ActonConfig::load()?;
    let wallet_names = config
        .wallets()
        .into_iter()
        .flatten()
        .map(|(name, _)| name.clone())
        .collect();
    let workspace = StudioWorkspace::new(config.package.name.clone(), configured_project_root())
        .with_wallet_names(wallet_names);
    let wallet_runtime = ProjectWalletRuntime::new(&config)?;
    Ok(Some((workspace, wallet_runtime)))
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = ctrl_c => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }

    let _ = ctrl_c.await;
}

#[cfg(test)]
mod tests {
    use super::StudioDaemonGuard;

    #[test]
    fn studio_daemon_registration_is_exclusive_per_project() {
        let project = tempfile::tempdir().expect("temporary Studio project must be created");
        let first = StudioDaemonGuard::register(project.path(), "http://127.0.0.1:3015".to_owned())
            .expect("first Studio instance must acquire the project lock");
        let error = StudioDaemonGuard::register(project.path(), "http://127.0.0.1:3016".to_owned())
            .expect_err("second Studio instance must not replace the daemon descriptor");
        assert!(
            error
                .to_string()
                .contains("Another Acton Studio instance is already running")
        );

        drop(first);
        StudioDaemonGuard::register(project.path(), "http://127.0.0.1:3016".to_owned())
            .expect("the project lock must be released with the Studio instance");
    }
}
