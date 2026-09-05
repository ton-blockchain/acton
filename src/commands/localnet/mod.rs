//! Command-line adapter for the standalone localnet control service.

mod args;
mod delete;
mod inspect;
mod output;
mod progress;
mod selection;
mod service;
mod start;

pub use args::LocalnetArgs;

use acton_localnet::{catalog, client::Client};
use anyhow::Context;
use reqwest::Method;
use serde_json::{Value, json};

use args::{LocalnetCommand, NodeCommand, SnapshotCommand};

/// Runs localnet independently from Studio. Only the service owns Docker state;
/// every command uses the same HTTP contract as other management clients.
pub fn localnet_cmd(args: LocalnetArgs) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(args))
}

async fn run(args: LocalnetArgs) -> anyhow::Result<()> {
    let root = args
        .state_dir
        .unwrap_or_else(|| acton_config::config::project_root().join(".acton-localnet"));

    tokio::fs::create_dir_all(&root).await?;
    let root = dunce::canonicalize(root)?;

    match args.command {
        LocalnetCommand::Start { options, detach } => {
            return start::start(&root, options, detach, args.json).await;
        }
        LocalnetCommand::Create { options } => {
            let request = start::create_request(&options).await?;
            let location = catalog::create(&root, request).await?;
            return output::network(&location.network, args.json);
        }
        LocalnetCommand::List => {
            let networks = catalog::list(&root).await?;
            return output::print(&networks.into_iter().map(|n| n.network).collect::<Vec<_>>());
        }
        _ => {}
    }

    let name = match &args.command {
        LocalnetCommand::Serve { network, .. }
        | LocalnetCommand::Status { network }
        | LocalnetCommand::Stop { network }
        | LocalnetCommand::Delete { network, .. }
        | LocalnetCommand::Logs { network, .. }
        | LocalnetCommand::Node { network, .. }
        | LocalnetCommand::Snapshot { network, .. }
        | LocalnetCommand::Operation { network, .. }
        | LocalnetCommand::Shutdown { network } => network.as_deref(),
        _ => unreachable!("handled before network selection"),
    };
    let Some(location) = selection::resolve(&root, name, args.json).await? else {
        return Ok(());
    };

    if let LocalnetCommand::Serve { port, .. } = args.command {
        let location = location.prepare(&root).await?;
        return service::serve(&location.path, port, args.json).await;
    }
    if let LocalnetCommand::Delete { yes, .. } = args.command
        && !delete::confirm(&location.network, yes, args.json)?
    {
        return Ok(());
    }

    match &args.command {
        LocalnetCommand::Status { .. } => return inspect::status(&location.path, args.json).await,
        LocalnetCommand::Logs { tail, .. } => {
            return inspect::logs(&location.path, *tail, args.json).await;
        }
        LocalnetCommand::Operation { id, wait, .. } => {
            return inspect::operation(&location.path, id, *wait, args.json).await;
        }
        _ => {}
    }

    // Stop/delete may need to clean up a network whose service is already gone.
    // Any temporary owner is scoped to this directory and is always reaped.
    let (client, owned) = if matches!(
        args.command,
        LocalnetCommand::Stop { .. }
            | LocalnetCommand::Delete { .. }
            | LocalnetCommand::Shutdown { .. }
    ) {
        service::connect_or_start(&root, location).await?
    } else {
        (Client::connect(&location.path).await.with_context(|| format!(
            "Network {:?} has no running service; run `acton localnet start {:?}` or `acton localnet serve {:?}`",
            location.network.name, location.network.name, location.network.name
        ))?, None)
    };
    let close_service = matches!(
        args.command,
        LocalnetCommand::Stop { .. }
            | LocalnetCommand::Delete { .. }
            | LocalnetCommand::Shutdown { .. }
    );
    let result = execute(&client, args.command, args.json).await;
    if let Some(mut child) = owned {
        let cleanup =
            output::shutdown(args.json, true, service::stop_owned(&client, &mut child)).await;
        result?;
        return cleanup;
    }
    result?;
    if close_service {
        output::shutdown(args.json, true, async {
            client.shutdown().await.map_err(Into::into)
        })
        .await?;
    }
    Ok(())
}

async fn execute(client: &Client, command: LocalnetCommand, json: bool) -> anyhow::Result<()> {
    match command {
        LocalnetCommand::Stop { .. } => {
            output::mutate(client, Method::POST, "/v1/network/stop", None, json).await
        }
        LocalnetCommand::Delete { .. } => {
            output::mutate(client, Method::DELETE, "/v1/network", None, json).await
        }
        LocalnetCommand::Node { command, .. } => {
            let base = "/v1/network/nodes".to_owned();
            let (method, path, body) = match command {
                NodeCommand::Add { name, validator } => (
                    Method::POST,
                    base,
                    Some(json!({"name":name, "validator":validator})),
                ),
                NodeCommand::Remove { id, force, .. } => {
                    validate_identifier(&id)?;
                    (Method::DELETE, format!("{base}/{id}?force={force}"), None)
                }
                NodeCommand::EnterValidation { id } => {
                    validate_identifier(&id)?;
                    (Method::POST, format!("{base}/{id}/enter-validation"), None)
                }
                NodeCommand::LeaveValidation { id } => {
                    validate_identifier(&id)?;
                    (Method::POST, format!("{base}/{id}/leave-validation"), None)
                }
            };

            output::mutate(client, method, &path, body, json).await
        }
        LocalnetCommand::Snapshot { command, .. } => {
            let base = "/v1/network/snapshots".to_owned();
            let (method, path, body) = match command {
                SnapshotCommand::List => {
                    let snapshots: Value = client.request(Method::GET, &base, None).await?;
                    return output::print(&snapshots);
                }
                SnapshotCommand::Create { name } => {
                    (Method::POST, base, Some(json!({"name":name})))
                }
                SnapshotCommand::Restore { id, .. } => {
                    validate_identifier(&id)?;
                    (Method::POST, format!("{base}/{id}/restore"), None)
                }
                SnapshotCommand::Delete { id, .. } => {
                    validate_identifier(&id)?;
                    (Method::DELETE, format!("{base}/{id}"), None)
                }
            };

            output::mutate(client, method, &path, body, json).await
        }
        LocalnetCommand::Shutdown { .. } => Ok(()),
        LocalnetCommand::Serve { .. }
        | LocalnetCommand::Start { .. }
        | LocalnetCommand::Create { .. }
        | LocalnetCommand::List
        | LocalnetCommand::Status { .. }
        | LocalnetCommand::Logs { .. }
        | LocalnetCommand::Operation { .. } => {
            unreachable!("handled before client discovery")
        }
    }
}

fn validate_identifier(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 80
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
        "Invalid resource identifier"
    );
    Ok(())
}
