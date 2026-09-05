//! Shutdown acknowledgement precedes Docker cleanup and release of service ownership.

use super::Client;
use crate::{Error, Network, ServiceDescriptor, Status, storage};
use reqwest::Method;
use serde_json::Value;
use std::time::Duration;

// An accepted snapshot may need its one-hour safe boundary before the two-minute
// Docker stop deadline. Keep the same lifecycle guarantee for external CLI clients.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(65 * 60);

impl Client {
    /// Requests shutdown and waits for durable cleanup results and released ownership.
    /// HTTP becoming unavailable is expected during shutdown and is not completion.
    /// A replacement service is never stopped or mistaken for the original owner.
    pub async fn shutdown(&self) -> Result<(), Error> {
        let requested = self
            .request::<Value>(Method::POST, "/v1/shutdown", None)
            .await;
        let deadline = tokio::time::Instant::now() + SHUTDOWN_TIMEOUT;
        let descriptor_path = storage::service_descriptor_path(&self.root);
        let log_path = self.root.join("service.log");

        loop {
            let locked = storage::service_is_locked(&self.root)?;
            let descriptor = match tokio::fs::read(&descriptor_path).await {
                Ok(bytes) => Some(
                    serde_json::from_slice::<ServiceDescriptor>(&bytes)
                        .map_err(|e| Error::storage(&descriptor_path, e))?,
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(Error::storage(&descriptor_path, error)),
            };
            if descriptor
                .as_ref()
                .is_some_and(|current| current.token != self.descriptor.token)
            {
                return Err(Error::Internal {
                    code: "service_replaced",
                    message: format!(
                        "Another localnet service started before shutdown could be confirmed; full log: {}",
                        log_path.display()
                    ),
                });
            }

            if !locked {
                let network: Network = storage::read_json(&self.root.join("network.json")).await?;
                if descriptor.is_none()
                    && matches!(network.status, Status::Stopped | Status::Deleted)
                {
                    return Ok(());
                }
                return Err(Error::Internal {
                    code: "shutdown_failed",
                    message: format!(
                        "Localnet shutdown did not complete: {}; full log: {}",
                        network
                            .error
                            .as_deref()
                            .unwrap_or("the service exited without confirming graceful shutdown"),
                        log_path.display()
                    ),
                });
            }

            // An unacknowledged request must not wait an hour on an unrelated
            // busy service. Completed shutdown remains idempotent above.
            if let Err(error) = requested {
                return Err(error);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Internal {
                    code: "shutdown_timeout",
                    message: format!(
                        "Localnet shutdown did not finish within 65 minutes; full log: {}",
                        log_path.display()
                    ),
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
