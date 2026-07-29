use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, manifest_path as configured_manifest_path, project_root as configured_project_root,
};
use acton_studio::{
    ContractRegistryStore, LocalProcessEnvironmentRuntime, LocalProcessTestRunRuntime,
    PUBLIC_TON_ENVIRONMENT_IDS, STUDIO_API_VERSION, StudioDaemonDescriptor, StudioServer,
    StudioServerConfig, StudioWorkspace, persist_studio_daemon_descriptor,
    remove_studio_daemon_descriptor,
};
use anyhow::Context;

use crate::studio_wallets::ProjectWalletRuntime;

pub async fn studio_start_cmd(host: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let configured_project = configured_project()?;
    if !host.is_loopback()
        && configured_project
            .as_ref()
            .is_some_and(|(_, wallet_runtime)| !wallet_runtime.is_empty())
    {
        anyhow::bail!(
            "Project wallet signing requires a loopback Studio host until remote authentication is configured"
        );
    }

    let address = SocketAddr::new(host, port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("Failed to bind Studio server to {address}"))?;
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
    let ip = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    format!("http://{}", SocketAddr::new(ip, address.port()))
}

struct StudioDaemonGuard {
    project_root: PathBuf,
    pid: u32,
}

impl StudioDaemonGuard {
    fn register(project_root: &Path, url: String) -> anyhow::Result<Self> {
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
