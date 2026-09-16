//! Private overlay topology expressed using durable localnet node identities.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Error, Node};

/// Complete private overlay configuration. An empty list removes all overlays.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayConfig {
    pub overlays: Vec<PrivateOverlay>,
}

/// Every participant sends blocks and external messages to the other members.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrivateOverlay {
    pub name: String,
    /// Stable node IDs, with `genesis` reserved for the original network node.
    pub nodes: Vec<String>,
}

impl OverlayConfig {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Checks topology before any runtime changes. Display names and ADNL keys
    /// are not node selectors; stopped nodes still retain their membership.
    pub fn validate(&self, nodes: &[Node]) -> Result<(), Error> {
        let known_nodes: HashSet<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
        let mut names = HashSet::new();
        for overlay in &self.overlays {
            if overlay.name.is_empty()
                || overlay.name.len() > 80
                || !overlay
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            {
                return Err(Error::invalid(
                    "Overlay names must contain 1 to 80 ASCII letters, digits, underscores, or hyphens",
                ));
            }
            if !names.insert(overlay.name.as_str()) {
                return Err(Error::invalid(format!(
                    "Duplicate overlay name {:?}",
                    overlay.name
                )));
            }
            if overlay.nodes.len() < 2 {
                return Err(Error::invalid(format!(
                    "Overlay {:?} must contain at least two distinct nodes",
                    overlay.name
                )));
            }
            let mut members = HashSet::new();
            for id in &overlay.nodes {
                if id != "genesis" && !known_nodes.contains(id.as_str()) {
                    return Err(Error::invalid(format!(
                        "Unknown node {id:?} in overlay {:?}; use a node ID or genesis",
                        overlay.name
                    )));
                }
                if !members.insert(id.as_str()) {
                    return Err(Error::invalid(format!(
                        "Duplicate node {id:?} in overlay {:?}",
                        overlay.name
                    )));
                }
            }
        }
        Ok(())
    }
}

impl crate::client::Client {
    /// Reads the saved topology from the localnet owner.
    pub async fn overlays(&self) -> Result<OverlayConfig, Error> {
        self.request(reqwest::Method::GET, "/v1/network/overlays", None)
            .await
    }

    /// Replaces the complete topology and returns its durable operation.
    pub async fn configure_overlays(
        &self,
        config: &OverlayConfig,
    ) -> Result<crate::Operation, Error> {
        self.request(
            reqwest::Method::PUT,
            "/v1/network/overlays",
            Some(serde_json::to_value(config).map_err(|error| Error::invalid(error.to_string()))?),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes() -> Vec<Node> {
        vec![
            Node {
                id: "node-1".to_owned(),
                name: "Alice".to_owned(),
                validator: true,
                port_base: 8100,
                stopped: false,
            },
            Node {
                id: "node-2".to_owned(),
                name: "Bob".to_owned(),
                validator: false,
                port_base: 8200,
                stopped: true,
            },
        ]
    }

    fn overlay(name: &str, nodes: &[&str]) -> PrivateOverlay {
        PrivateOverlay {
            name: name.to_owned(),
            nodes: nodes.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn accepts_overlapping_memberships_and_stopped_nodes() {
        let config = OverlayConfig {
            overlays: vec![
                overlay("Relay_1", &["genesis", "node-1"]),
                overlay(&"x".repeat(80), &["node-1", "node-2"]),
            ],
        };
        config.validate(&nodes()).unwrap();
        OverlayConfig::default().validate(&[]).unwrap();
    }

    #[test]
    fn rejects_invalid_names_and_duplicate_overlays() {
        for name in ["", "relay/name", "relay name", "реле", &"x".repeat(81)] {
            let config = OverlayConfig {
                overlays: vec![overlay(name, &["genesis", "node-1"])],
            };
            assert!(config.validate(&nodes()).is_err(), "name: {name:?}");
        }
        let config = OverlayConfig {
            overlays: vec![
                overlay("relay", &["genesis", "node-1"]),
                overlay("relay", &["genesis", "node-2"]),
            ],
        };
        assert!(config.validate(&nodes()).is_err());
    }

    #[test]
    fn requires_two_distinct_existing_ids() {
        for members in [
            vec![],
            vec!["genesis"],
            vec!["genesis", "genesis"],
            vec!["node-1", "node-1"],
            vec!["genesis", "missing"],
            vec!["genesis", "Alice"],
            vec!["genesis", ""],
        ] {
            let config = OverlayConfig {
                overlays: vec![overlay("relay", &members)],
            };
            assert!(config.validate(&nodes()).is_err(), "members: {members:?}");
        }
    }

    #[test]
    fn rejects_misspelled_config_without_clearing_overlays() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"overlay": []}),
            serde_json::json!({"overlays": [], "typo": true}),
            serde_json::json!({"overlays": [{"name": "relay", "node": ["genesis", "node-1"]}]}),
        ] {
            assert!(serde_json::from_value::<OverlayConfig>(value).is_err());
        }
        let config: OverlayConfig =
            serde_json::from_value(serde_json::json!({"overlays": []})).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn old_network_records_default_to_no_overlays() {
        let mut network: crate::Network = serde_json::from_value(serde_json::json!({
            "id": "network-1",
            "name": "test",
            "config": {"portBase": 8000, "importedAccountBocs": []},
            "endpoints": {"apiV2": "", "apiV3": "", "admin": "", "config": "", "observability": ""},
            "nodes": [],
            "status": "stopped"
        }))
        .unwrap();
        assert!(network.overlay_config.is_empty());
        assert!(
            serde_json::to_value(&network)
                .unwrap()
                .get("overlayConfig")
                .is_none()
        );

        network
            .overlay_config
            .overlays
            .push(overlay("relay", &["genesis", "node-1"]));
        let saved = serde_json::to_value(&network).unwrap();
        let restored: crate::Network = serde_json::from_value(saved).unwrap();
        assert_eq!(network.overlay_config, restored.overlay_config);
    }
}
