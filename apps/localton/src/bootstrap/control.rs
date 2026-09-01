//! Starts configured validator-engine nodes through the shared process registry.

use std::time::Duration;

use anyhow::{Context, Result, ensure};

use crate::{
    runtime::{ProcessInfo, ProcessRegistry},
    storage::Layout,
    storage::Settings,
    storage::{NodeRuntime, RuntimeState},
    ton::toolchain::Toolchain,
    ton::tools::{types::OperationContext, validator_engine::ValidatorDatabase},
};

use super::{nodes, validator};

/// Owns idempotent node startup against one state directory and process registry.
///
/// Clones share process ownership, while layout, toolchain, and readiness timeout
/// remain immutable for the lifetime of one Localton invocation.
#[derive(Clone)]
pub struct NodeController {
    layout: Layout,
    tools: Toolchain,
    timeout: Duration,
    processes: ProcessRegistry,
}

impl NodeController {
    /// Binds node operations to one immutable layout, toolchain, timeout, and registry.
    pub(crate) fn new(
        layout: Layout,
        tools: Toolchain,
        timeout: Duration,
        processes: ProcessRegistry,
    ) -> Self {
        Self {
            layout,
            tools,
            timeout,
            processes,
        }
    }

    /// Returns the state layout whose nodes this controller is allowed to manage.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Clones the immutable toolchain handles used by managed node workflows.
    pub(crate) fn toolchain(&self) -> Toolchain {
        self.tools.clone()
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

        // Repeated starts return the already published runtime entry instead of
        // creating a second validator-engine for the same durable database.
        if self.processes.contains(name).await {
            return RuntimeState::load(&self.layout.runtime)?
                .nodes
                .get(name)
                .cloned()
                .context("running node is absent from runtime state");
        }

        let mut node_runtime =
            nodes::ensure_initialized(&self.layout, &self.tools, &node, self.timeout).await?;
        let context = OperationContext::for_node(self.timeout, &node.name);
        let node_layout = self.layout.node(&node);

        let mut process = validator::start_persistent(
            &self.layout,
            self.tools.validator_engine.as_ref(),
            &node,
            ValidatorDatabase::open(node_layout.db)?,
        )
        .await?;

        // Registry ownership starts only after the authenticated console proves
        // that the engine is ready for management operations.
        if let Err(error) = validator::wait_for_console(
            &self.layout,
            self.tools.validator_console_tool.as_ref(),
            &node,
            &mut process,
            &context,
        )
        .await
        {
            process.stop().await?;
            return Err(error).context(format!("node `{name}` console did not become ready"));
        }

        node_runtime.running = true;
        node_runtime.pid = process.pid();
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

    /// Returns a stable snapshot of processes owned by this controller's registry.
    pub async fn process_info(&self) -> Vec<ProcessInfo> {
        self.processes.info().await
    }
}
