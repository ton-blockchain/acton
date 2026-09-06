//! The network owner serializes configuration changes against stop and restore.

use super::operations::Context;
use crate::{Error, Status, UpdateNetworkConfig};
use std::time::Duration;

impl Context {
    pub(super) async fn update_network_config(
        &mut self,
        request: UpdateNetworkConfig,
    ) -> Result<serde_json::Value, Error> {
        let network = self.entry.record.read().await.clone();
        if network.status != Status::Running {
            return Err(Error::Conflict {
                code: "network_not_running",
                message: "Start the network before updating its configuration".to_owned(),
            });
        }

        self.phase("confirmingConfig").await?;
        let response = reqwest::Client::new().post(format!("{}/v1/network/config", network.endpoints.admin))
            .timeout(Duration::from_secs(70)).json(&request).send().await
            .map_err(|error| Error::Internal { code: "config_update_failed", message: format!("Localton config update failed: {error}; reload the configuration before retrying") })?;
        let status = response.status();
        let result: serde_json::Value = response.json().await.map_err(|error| Error::Internal {
            code: "config_update_failed",
            message: format!("Invalid Localton config response: {error}"),
        })?;
        if !status.is_success() {
            return Err(Error::Api {
                status: status.as_u16(),
                code: "config_update_failed".to_owned(),
                message: result["error"]
                    .as_str()
                    .unwrap_or("Localton could not apply the configuration")
                    .to_owned(),
            });
        }

        // Localton confirms activation before returning success. Clear the stale
        // environment diagnosis here; the failed operation keeps its own error.
        self.entry.record.write().await.error = None;

        Ok(result)
    }
}
