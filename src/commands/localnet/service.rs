//! Foreground ownership and startup discovery for the control service.

use acton_localnet::{catalog::NetworkDirectory, client::Client};
use anyhow::Context;
use std::path::Path;
use tokio::process::Child;

use super::{output, progress::label};

pub(super) async fn serve(root: &Path, port: u16, json: bool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .context("Failed to bind the localnet control API")?;
    if !json {
        eprintln!(
            "{} the localnet control API on http://{}",
            label("Starting", false),
            listener.local_addr()?
        );
    }

    let (send, receive) = tokio::sync::oneshot::channel();
    let serving = acton_localnet::http::serve(root, listener, async {
        let _ = receive.await;
    });
    tokio::pin!(serving);

    tokio::select! {
        result = &mut serving => {
            result?;
            if !json {
                eprintln!("{} Acton localnet gracefully", label("Stopped", false));
            }
        }
        _ = shutdown_signal() => {
            let _ = send.send(());
            output::shutdown(json, true, async { serving.await.map_err(Into::into) }).await?;
        }
    }

    Ok(())
}

/// Starts only the service belonging to this state directory. The child is kept
/// by the foreground command so Ctrl-C can request graceful HTTP shutdown.
pub(super) async fn connect_or_start(
    catalog_root: &Path,
    location: NetworkDirectory,
) -> anyhow::Result<(Client, Option<Child>)> {
    Ok(acton_localnet::process::Launcher {
        executable: std::env::current_exe()?,
        project_root: acton_config::config::project_root().to_owned(),
        catalog_root: catalog_root.to_owned(),
    }
    .connect_or_start(location)
    .await?)
}

pub(super) async fn stop_owned(client: &Client, child: &mut Child) -> anyhow::Result<()> {
    let result = client.shutdown().await;
    if result.is_err() && child.try_wait()?.is_none() {
        terminate_owned(child).await?;
    }

    let status = child.wait().await?;
    result?;
    anyhow::ensure!(
        status.success(),
        "Localnet service could not stop cleanly ({status}); inspect service.log"
    );
    Ok(())
}

/// The PID comes directly from our Child handle. SIGTERM runs the same graceful
/// service shutdown as HTTP when discovery or the listener has failed.
async fn terminate_owned(child: &mut Child) -> anyhow::Result<()> {
    Ok(acton_localnet::process::terminate(child).await?)
}

pub(super) fn shutdown_signal() -> impl Future<Output = ()> {
    // Register SIGTERM before launch/discovery so an application can close a
    // newly spawned foreground owner without bypassing graceful shutdown.
    #[cfg(unix)]
    let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();

    async move {
        #[cfg(unix)]
        if let Some(mut terminate) = terminate {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
        let _ = tokio::signal::ctrl_c().await;
    }
}
