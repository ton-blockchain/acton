//! Starts configured validator-engine nodes through the shared process registry.

use crate::{
    runtime::{ProcessInfo, ProcessRegistry},
    storage::Layout,
    ton::toolchain::Toolchain,
};

#[derive(Clone)]
pub struct NodeController {
    layout: Layout,
    tools: Toolchain,
    processes: ProcessRegistry,
}

impl NodeController {
    pub(crate) fn new(layout: Layout, tools: Toolchain, processes: ProcessRegistry) -> Self {
        Self {
            layout,
            tools,
            processes,
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(crate) fn toolchain(&self) -> Toolchain {
        self.tools.clone()
    }

    pub async fn process_info(&self) -> Vec<ProcessInfo> {
        self.processes.info().await
    }
}
