//! Foreground start owns a newly launched service until graceful termination.

use super::{args::CreateOptions, output, selection, service};
use acton_localnet::{CreateNetwork, Network, Operation, Status, catalog, client::Client};
use reqwest::Method;
use std::path::Path;

pub(super) async fn start(
    root: &Path,
    options: CreateOptions,
    detach: bool,
    json: bool,
) -> anyhow::Result<()> {
    let shutdown = service::shutdown_signal();
    tokio::pin!(shutdown);

    // Read user input before launching a service so an invalid file cannot leave
    // a background process behind.
    let request = create_request(&options).await?;
    let networks = catalog::list(root).await?;
    let existing = options.name.as_ref().is_none_or(|name| {
        networks
            .iter()
            .any(|n| n.network.name == *name || n.network.id == *name)
    }) && !networks.is_empty();
    let location = if existing {
        let Some(location) = selection::choose(networks, options.name.as_deref(), json)? else {
            return Ok(());
        };
        anyhow::ensure!(
            options.port_base.is_none()
                && options.block_time_ms.is_none()
                && options.election_time_seconds.is_none()
                && options.accounts_file.is_none(),
            "Genesis options apply only to new networks; create a new name to change them"
        );
        location
    } else {
        catalog::create(root, request).await?
    };
    let (client, mut owned) = service::connect_or_start(root, location).await?;
    let startup = start_network(&client, json);

    if owned.is_none() {
        return startup.await;
    }

    let (result, interrupted) = tokio::select! {
        result = startup => (result, false),
        _ = &mut shutdown => (Ok(()), true),
    };

    if result.is_ok() && detach && !interrupted {
        return Ok(());
    }

    if result.is_ok() && !interrupted {
        if !json {
            eprintln!("\nPress Ctrl-C to stop the network gracefully");
        }
        // A separate `stop`, `delete`, or `shutdown` command can close this
        // network's service. The foreground owner must then exit as well.
        let child = owned.as_mut().expect("foreground service owner");
        tokio::select! {
            _ = &mut shutdown => {}
            status = child.wait() => {
                anyhow::ensure!(status?.success(), "Localnet service exited unsuccessfully; inspect service.log");
                if !json {
                    eprintln!("{} Acton localnet gracefully", super::progress::label("Stopped", false));
                }
                return Ok(());
            }
        }
    }

    if let Some(child) = &mut owned {
        let stopped =
            output::shutdown(json, result.is_ok(), service::stop_owned(&client, child)).await;
        if result.is_ok() {
            return stopped;
        }
        if let Err(error) = stopped {
            eprintln!("Graceful cleanup also failed: {error}");
        }
    }

    result
}

async fn start_network(client: &Client, json: bool) -> anyhow::Result<()> {
    let network: Network = client.request(Method::GET, "/v1/network", None).await?;

    if network.status != Status::Running {
        // Another client may already be starting this deployment. Adopt its
        // operation instead of issuing a conflicting second start request.
        let operation: Operation = match network.operation.as_ref() {
            Some(operation)
                if operation.kind == "start"
                    && operation.status == acton_localnet::OperationStatus::Running =>
            {
                operation.clone()
            }
            _ => {
                client
                    .request(Method::POST, "/v1/network/start", None)
                    .await?
            }
        };

        output::wait(client, operation, json).await?;
    }

    let network: Network = client.request(Method::GET, "/v1/network", None).await?;
    output::network(&network, json)
}

pub(super) async fn create_request(options: &CreateOptions) -> anyhow::Result<CreateNetwork> {
    let imported_account_bocs = match &options.accounts_file {
        Some(path) => serde_json::from_slice(&tokio::fs::read(path).await?)?,
        None => Vec::new(),
    };

    Ok(CreateNetwork {
        ports: Default::default(),
        reserved_ports: Vec::new(),
        name: options
            .name
            .clone()
            .unwrap_or_else(|| "localnet".to_owned()),
        port_base: options.port_base,
        block_time_ms: options.block_time_ms,
        election_time_seconds: options.election_time_seconds,
        imported_account_bocs,
    })
}
