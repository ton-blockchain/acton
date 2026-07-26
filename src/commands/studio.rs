use std::net::{IpAddr, SocketAddr};

use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, manifest_path as configured_manifest_path, project_root as configured_project_root,
};
use acton_studio::{
    LocalProcessEnvironmentRuntime, StudioServer, StudioServerConfig, StudioWorkspace,
};
use anyhow::Context;

pub async fn studio_start_cmd(host: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let address = SocketAddr::new(host, port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("Failed to bind Studio server to {address}"))?;
    let address = listener
        .local_addr()
        .context("Failed to inspect Studio server address")?;
    let url = format!("http://{address}");

    let mut config = StudioServerConfig::new(crate::build_info::SHORT_VERSION);
    if let Some(workspace) = configured_workspace()? {
        config = config.with_workspace(workspace);
    }
    let acton_executable =
        std::env::current_exe().context("Failed to locate the Acton executable")?;
    let environment_runtime =
        LocalProcessEnvironmentRuntime::new(acton_executable, configured_project_root());
    let server = StudioServer::new(config).with_environment_runtime(environment_runtime);

    println!("    {} Acton Studio at {}", "Starting".green().bold(), url);

    if open_browser
        && host.is_loopback()
        && let Err(error) = opener::open(&url)
    {
        eprintln!("Warning: Failed to open Acton Studio at {url}: {error}");
    }

    server.serve(listener, shutdown_signal()).await?;
    Ok(())
}

fn configured_workspace() -> anyhow::Result<Option<StudioWorkspace>> {
    if !configured_manifest_path().is_file() {
        return Ok(None);
    }

    let config = ActonConfig::load_manifest()?;
    let wallet_names = ActonConfig::load_wallets()?.wallets.into_keys().collect();
    Ok(Some(
        StudioWorkspace::new(config.package.name, configured_project_root())
            .with_wallet_names(wallet_names),
    ))
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
