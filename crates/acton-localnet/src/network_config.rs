//! Public on-chain config mutations, distinct from Docker and genesis settings.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Replaces one parameter after checking the cell the editor originally loaded.
///
/// `expected_hash: None` adds a parameter only if it is still absent. The service
/// owns signing and confirmation; applications never receive the master key.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateNetworkConfig {
    pub index: i32,
    /// Standard base64, single-root parameter `BoC`
    pub boc: String,
    /// Lowercase hexadecimal representation hash, or null for a new parameter
    pub expected_hash: Option<String>,
}

impl crate::client::Client {
    /// Starts a durable operation which succeeds only after masterchain confirmation.
    pub async fn update_network_config(
        &self,
        request: &UpdateNetworkConfig,
    ) -> Result<crate::Operation, crate::Error> {
        self.request(
            reqwest::Method::POST,
            "/v1/network/config",
            Some(
                serde_json::to_value(request)
                    .map_err(|error| crate::Error::invalid(error.to_string()))?,
            ),
        )
        .await
    }

    /// Reconnects to an accepted operation without replaying its mutation.
    pub async fn operation(&self, id: &str) -> Result<crate::Operation, crate::Error> {
        crate::storage::validate_id(id)?;
        self.request(reqwest::Method::GET, &format!("/v1/operations/{id}"), None)
            .await
    }
}
