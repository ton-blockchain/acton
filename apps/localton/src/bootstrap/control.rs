//! Runtime control operations exposed by the launcher's admin HTTP service.
//!
//! Once the core network is running, the admin service can start or stop a
//! configured validator and enable the next available validator slot. These
//! operations reuse the same initialization and process registry as the startup
//! pipeline, so runtime state always describes the managed processes.

use std::time::Duration;

use anyhow::{Context, Result, ensure};

use crate::{
    binaries::TonBinaries,
    runtime::{ManagedProcess, ProcessInfo, ProcessRegistry},
    storage::Layout,
    storage::Settings,
    storage::{NodeRuntime, RuntimeState},
};

use super::{nodes, validator};

#[derive(Clone)]
pub struct LauncherControl {
    layout: Layout,
    binaries: TonBinaries,
    timeout: Duration,
    processes: ProcessRegistry,
}

impl LauncherControl {
    pub(super) fn new(
        layout: Layout,
        binaries: TonBinaries,
        timeout: Duration,
        processes: ProcessRegistry,
    ) -> Self {
        Self {
            layout,
            binaries,
            timeout,
            processes,
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Starts one enabled validator-engine node and publishes it as running.
    ///
    /// Initialization is idempotent and may create the node's database and keys
    /// on the first call. The process enters the shared registry only after its
    /// authenticated control console responds, so status cannot claim that a
    /// node is running while validator-engine is still unusable.
    pub async fn start_node(&self, name: &str) -> Result<NodeRuntime> {
        let settings = Settings::load_or_create(&self.layout.settings)?;
        let node = settings.node(name)?.clone();
        ensure!(node.enabled, "node `{name}` is disabled");
        if self.processes.contains(name).await {
            return RuntimeState::load(&self.layout.runtime)?
                .nodes
                .get(name)
                .cloned()
                .context("running node is absent from runtime state");
        }
        let mut node_runtime =
            nodes::ensure_initialized(&self.layout, &self.binaries, &node, self.timeout).await?;
        let node_layout = self.layout.node(&node);
        let mut process = ManagedProcess::spawn(
            node.name.clone(),
            validator::command(&self.layout, &self.binaries, &node, true),
            &node_layout.logs.join("validator.stdout.log"),
            &node_layout.logs.join("validator.stderr.log"),
        )?;
        if let Err(error) = validator::wait_for_console(
            &self.layout,
            &self.binaries,
            &node,
            &mut process,
            self.timeout,
        )
        .await
        {
            process.stop().await?;
            return Err(error).context(format!("node `{name}` console did not become ready"));
        }
        node_runtime.running = true;
        node_runtime.pid = process.id();
        node_runtime.status = "running".to_owned();
        self.processes.insert(process).await?;
        RuntimeState::update_atomic(&self.layout.runtime, |runtime| {
            runtime
                .nodes
                .insert(node.name.clone(), node_runtime.clone());
            Ok(())
        })?;
        Ok(node_runtime)
    }

    /// Stops a managed node if present and records the result atomically.
    ///
    /// The operation is safe to repeat: an absent process becomes `not running`
    /// instead of turning an administrative retry into an error.
    pub async fn stop_node(&self, name: &str) -> Result<NodeRuntime> {
        Settings::load_or_create(&self.layout.settings)?.node(name)?;
        let stopped = self.processes.stop(name).await?;
        let updated = RuntimeState::update_atomic(&self.layout.runtime, |runtime| {
            let node = runtime.nodes.entry(name.to_owned()).or_default();
            node.running = false;
            node.pid = None;
            node.status = if stopped {
                "stopped".to_owned()
            } else {
                "not running".to_owned()
            };
            Ok(())
        })?;
        Ok(updated.nodes.get(name).cloned().unwrap_or_default())
    }

    /// Enables and starts the next preconfigured non-genesis validator slot.
    ///
    /// Ports and directories already exist in settings. This operation activates
    /// one slot, marks it as a validator, optionally enables election
    /// participation, persists the topology, and delegates normal node startup.
    pub async fn add_validator(&self, participate: bool) -> Result<String> {
        let mut settings = Settings::load_or_create(&self.layout.settings)?;
        let node = settings
            .nodes
            .iter_mut()
            .find(|node| node.name != "genesis" && !node.enabled)
            .context("all configured local nodes are already enabled")?;
        node.enabled = true;
        node.validator = true;
        node.participate_in_elections = participate;
        let name = node.name.clone();
        settings.validate()?;
        settings.save_atomic(&self.layout.settings)?;
        self.start_node(&name).await?;
        Ok(name)
    }

    pub async fn process_info(&self) -> Vec<ProcessInfo> {
        self.processes.info().await
    }
}
