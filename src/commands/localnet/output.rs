//! Terminal rendering consumes the same operation records as an HTTP client.

use acton_localnet::{Network, Operation, OperationStatus, client::Client};
use reqwest::Method;
use serde_json::Value;

use super::progress::{self, Activity, label};

pub(super) fn print(value: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);

    Ok(())
}

pub(super) fn network(network: &Network, json: bool) -> anyhow::Result<()> {
    if json {
        return print(network);
    }

    let status = format!("{:?}", network.status);
    println!("\nFull localnet \"{}\"", network.name);
    println!("  Status:    {}", status.to_lowercase());
    println!("  Network:   {}", network.id);
    if let Some(state) = &network.state {
        println!("  State:     {} (inside Docker)", state.directory);
        println!("  Volume:    {}", state.volume);
    }

    for (name, url) in [
        ("API v2", &network.endpoints.api_v2),
        ("API v3", &network.endpoints.api_v3),
        ("Admin", &network.endpoints.admin),
        ("Config", &network.endpoints.config),
        ("Dashboard", &network.endpoints.observability),
    ] {
        println!("  {:<10} {url}", format!("{name}:"));
    }

    if let Some(error) = &network.error {
        eprintln!("{} {error}", label("Error", true));
    }

    Ok(())
}

pub(super) async fn wait(
    client: &Client,
    mut operation: Operation,
    json: bool,
) -> anyhow::Result<Operation> {
    let mut activity = Activity::new(json);
    let mut completed_steps = 0;

    loop {
        if completed_steps < operation.completed_steps.len() {
            for step in &operation.completed_steps[completed_steps..] {
                activity.finish_step(step, &operation.kind);
            }
            completed_steps = operation.completed_steps.len();
            activity = Activity::new(json);
        }

        match operation.status {
            OperationStatus::Completed => {
                drop(activity);
                if !json {
                    eprintln!(
                        "{} {} in {:.1}s",
                        label("Finished", false),
                        progress::action(&operation.kind),
                        operation.duration_ms as f64 / 1000.0
                    );
                }

                return Ok(operation);
            }
            OperationStatus::Failed => {
                drop(activity);
                if json {
                    print(&operation)?;
                }
                anyhow::bail!(
                    "{}",
                    operation
                        .error
                        .as_deref()
                        .unwrap_or("Localnet operation failed")
                );
            }
            OperationStatus::Running => activity.operation(&operation),
        }

        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        operation = client
            .request(
                Method::GET,
                &format!("/v1/operations/{}", operation.id),
                None,
            )
            .await?;
    }
}

pub(super) async fn mutate(
    client: &Client,
    method: Method,
    path: &str,
    body: Option<Value>,
    json: bool,
) -> anyhow::Result<()> {
    let operation = client.request(method, path, body).await?;
    let operation = wait(client, operation, json).await?;
    if json {
        print(&operation)?;
    } else if let Some(id) = operation
        .result
        .as_ref()
        .and_then(|result| result.get("id"))
        .and_then(Value::as_str)
    {
        println!("\n  Result: {id}");
    }

    Ok(())
}

/// The success message follows the service exit, not its HTTP 202 acknowledgement.
pub(super) async fn shutdown(
    json: bool,
    requested: bool,
    work: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    if !json {
        if requested {
            // The terminal echoes ^C without a newline. Start the status column
            // on a fresh line instead of letting those two characters shift it.
            eprintln!();
        }

        let message = if requested {
            "Acton localnet gracefully (shutdown requested)"
        } else {
            "Acton localnet gracefully after failed startup"
        };
        eprintln!("{} {message}", label("Stopping", true));
    }

    let mut activity = Activity::new(json);
    activity.update(
        "Stopping",
        "Docker services gracefully; preserving network data",
        None,
    );
    let result = work.await;
    drop(activity);
    result?;

    if !json {
        eprintln!("{} Acton localnet gracefully", label("Stopped", false));
    }

    Ok(())
}
