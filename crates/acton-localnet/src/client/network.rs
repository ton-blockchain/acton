//! Typed network operations keep wire paths and completion semantics out of applications.

use super::Client;
use crate::{Error, Network, NetworkHealth, Operation, OperationStatus, Snapshot};
use reqwest::Method;
use serde_json::json;

impl Client {
    /// Reads current state and progress without waiting for an active mutation.
    pub async fn network(&self) -> Result<Network, Error> {
        self.request(Method::GET, "/v1/network", None).await
    }

    /// Samples endpoint and Compose health from the process that owns this network.
    pub async fn health(&self) -> Result<NetworkHealth, Error> {
        self.request(Method::GET, "/v1/network/health", None).await
    }

    /// Starts this service's network; completion is reported by the returned operation.
    pub async fn start(&self) -> Result<Operation, Error> {
        self.request(Method::POST, "/v1/network/start", None).await
    }

    /// Preserves network volumes and leaves its control service available.
    pub async fn stop(&self) -> Result<Operation, Error> {
        self.request(Method::POST, "/v1/network/stop", None).await
    }

    /// Deletes this deployment's Docker resources; callers must explicitly authorize deletion.
    pub async fn delete(&self) -> Result<Operation, Error> {
        self.request(Method::DELETE, "/v1/network", None).await
    }

    /// Lets the service allocate and persist node identity before joining the network.
    pub async fn add_node(&self, name: &str, validator: bool) -> Result<Operation, Error> {
        self.request(
            Method::POST,
            "/v1/network/nodes",
            Some(json!({"name": name, "validator": validator})),
        )
        .await
    }

    /// The service checks elected-set membership unless the user explicitly forces removal.
    pub async fn remove_node(&self, id: &str, force: bool) -> Result<Operation, Error> {
        crate::storage::validate_id(id)?;
        self.request(
            Method::DELETE,
            &format!("/v1/network/nodes/{id}?force={force}"),
            None,
        )
        .await
    }

    /// Changes future election participation; current validator sets remain immutable.
    pub async fn validation(&self, id: &str, enabled: bool) -> Result<Operation, Error> {
        crate::storage::validate_id(id)?;
        let action = if enabled {
            "enter-validation"
        } else {
            "leave-validation"
        };
        self.request(
            Method::POST,
            &format!("/v1/network/nodes/{id}/{action}"),
            None,
        )
        .await
    }

    /// Reads archive metadata while the service serializes access against restore operations.
    pub async fn snapshots(&self) -> Result<Vec<Snapshot>, Error> {
        self.request(Method::GET, "/v1/network/snapshots", None)
            .await
    }

    /// Creates an archive and restores the network's previous running intent.
    pub async fn create_snapshot(&self, name: Option<&str>) -> Result<Operation, Error> {
        self.request(
            Method::POST,
            "/v1/network/snapshots",
            Some(json!({"name": name})),
        )
        .await
    }

    /// Restores blockchain state and rebuilds the derived indexer before reporting completion.
    pub async fn restore_snapshot(&self, id: &str) -> Result<Operation, Error> {
        crate::storage::validate_id(id)?;
        self.request(
            Method::POST,
            &format!("/v1/network/snapshots/{id}/restore"),
            None,
        )
        .await
    }

    /// Removes one archive without changing network state.
    pub async fn delete_snapshot(&self, id: &str) -> Result<Operation, Error> {
        crate::storage::validate_id(id)?;
        self.request(Method::DELETE, &format!("/v1/network/snapshots/{id}"), None)
            .await
    }

    /// Polls durable completion without replaying mutations after a transport failure.
    pub async fn wait(&self, mut operation: Operation) -> Result<Operation, Error> {
        while operation.status == OperationStatus::Running {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            operation = self
                .request(
                    Method::GET,
                    &format!("/v1/operations/{}", operation.id),
                    None,
                )
                .await?;
        }
        if operation.status == OperationStatus::Failed {
            return Err(Error::Api {
                status: operation.error_status.unwrap_or(500),
                code: operation
                    .error_code
                    .unwrap_or_else(|| "operation_failed".to_owned()),
                message: operation.error.unwrap_or_else(|| {
                    format!(
                        "Localnet operation failed; full log: {}",
                        operation.log_path
                    )
                }),
            });
        }
        Ok(operation)
    }
}

impl Client {
    /// Submits an edit once; reuse its ID after an ambiguous response.
    pub async fn start_admin(
        &self,
        request: &crate::AdminRequest,
    ) -> Result<crate::AdminOperation, Error> {
        self.request(
            Method::POST,
            "/v1/network/admin",
            Some(serde_json::to_value(request).map_err(|e| Error::invalid(e.to_string()))?),
        )
        .await
    }

    /// Reads the active or latest durable administrative operation.
    pub async fn admin_operation(&self) -> Result<Option<crate::AdminOperation>, Error> {
        self.request(Method::GET, "/v1/network/admin", None).await
    }
}
