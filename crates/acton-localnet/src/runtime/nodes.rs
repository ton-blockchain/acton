//! Topology mutations and TON elected-set checks.

use super::{Runtime, operations::Context};
use crate::{Error, Node, Status, docker::DockerNetwork};
use serde::Deserialize;
use serde_json::Value;

impl Context {
    async fn require_running(&self) -> Result<(), Error> {
        if self.entry.record.read().await.status != Status::Running {
            return Err(Error::Conflict {
                code: "network_not_running",
                message: "The network must be running for this operation".to_owned(),
            });
        }

        Ok(())
    }

    pub(super) async fn add_node(
        &mut self,
        driver: &DockerNetwork,
        name: String,
        validator: bool,
    ) -> Result<Value, Error> {
        self.require_running().await?;
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 80
            || name.eq_ignore_ascii_case("genesis")
            || name.contains('$')
        {
            return Err(Error::invalid(
                "Node names must contain 1 to 80 characters, cannot contain $, and cannot be genesis",
            ));
        }

        let old_nodes = self.entry.record.read().await.nodes.clone();
        if old_nodes.iter().any(|n| n.name.eq_ignore_ascii_case(name)) {
            return Err(Error::invalid("A node with this name already exists"));
        }

        let number = old_nodes
            .iter()
            .filter_map(|n| n.id.strip_prefix("node-")?.parse::<u16>().ok())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Error::invalid("Node limit reached"))?;
        let port_base = number
            .checked_mul(10)
            .and_then(|n| n.checked_add(20000))
            .filter(|p| *p <= 65525)
            .ok_or_else(|| Error::invalid("Node port range exhausted"))?;
        let node = Node {
            id: format!("node-{number}"),
            name: name.to_owned(),
            validator,
            port_base,
        };

        self.phase("joiningNode").await?;
        self.entry.record.write().await.nodes.push(node.clone());
        Runtime::save(&self.entry).await?;
        if let Err(error) = self
            .observe(driver, driver.add_node(&old_nodes, &node))
            .await
        {
            self.entry.record.write().await.nodes = old_nodes;
            Runtime::save(&self.entry).await?;
            return Err(error);
        }
        serde_json::to_value(node).map_err(|e| Error::invalid(e.to_string()))
    }

    pub(super) async fn remove_node(
        &mut self,
        driver: &DockerNetwork,
        id: &str,
        force: bool,
    ) -> Result<(), Error> {
        let old_nodes = self.entry.record.read().await.nodes.clone();
        let node = old_nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::invalid("Node is not managed by this network"))?;
        if node.validator && !force {
            self.ensure_validator_can_be_removed(node).await?;
        }

        self.phase("removingNode").await?;
        self.entry.record.write().await.nodes.retain(|n| n.id != id);
        Runtime::save(&self.entry).await?;
        if let Err(error) = driver.remove_node(&old_nodes, node).await {
            self.entry.record.write().await.nodes = old_nodes;
            Runtime::save(&self.entry).await?;
            return Err(error);
        }

        Ok(())
    }

    pub(super) async fn validation(
        &mut self,
        driver: &DockerNetwork,
        id: &str,
        enabled: bool,
    ) -> Result<(), Error> {
        self.require_running().await?;
        let node = self
            .entry
            .record
            .read()
            .await
            .nodes
            .iter()
            .find(|n| n.id == id)
            .cloned()
            .ok_or_else(|| Error::invalid("Node is not managed by this network"))?;
        self.phase(if enabled {
            "enteringElections"
        } else {
            "leavingElections"
        })
        .await?;
        if enabled {
            driver.enter_validation(&node).await?;
            let mut record = self.entry.record.write().await;
            if let Some(node) = record.nodes.iter_mut().find(|n| n.id == id) {
                node.validator = true;
            }
        } else {
            driver.leave_validation(&node).await?;
        }

        Ok(())
    }

    /// Collector uncertainty is unsafe: ordinary deletion requires confirmed
    /// absence from both elected sets and disabled future participation.
    async fn ensure_validator_can_be_removed(&self, node: &Node) -> Result<(), Error> {
        let endpoint = self
            .entry
            .record
            .read()
            .await
            .endpoints
            .observability
            .clone();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| Error::invalid(e.to_string()))?;
        let result = async {
            client
                .get(format!("{endpoint}/api/v1/network"))
                .send()
                .await?
                .error_for_status()?
                .json::<ObservedNetwork>()
                .await
        }
        .await;
        let network = result.map_err(|e| Error::Conflict {
            code: "validator_state_unavailable",
            message: format!("Cannot confirm validator membership: {e}"),
        })?;
        let safe = network.nodes.iter().any(|n| {
            n.name.eq_ignore_ascii_case(&node.name)
                && !n.participate_in_elections
                && !n.active_validator
                && n.current_validator == Some(false)
                && n.next_validator != Some(true)
        });
        if !safe {
            return Err(Error::Conflict { code: "validator_still_active", message: "Leave validation and wait until the node is outside the current and next elected sets before removing it".to_owned() });
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct ObservedNetwork {
    nodes: Vec<ObservedNode>,
}

#[derive(Deserialize)]
struct ObservedNode {
    name: String,
    active_validator: bool,
    participate_in_elections: bool,
    current_validator: Option<bool>,
    next_validator: Option<bool>,
}
