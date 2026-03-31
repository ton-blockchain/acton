use crate::debugger::any_executor::AnyExecutor;
use crate::replayer::StepMode;
use std::sync::Arc;
use tolkc::TolkSourceMap;

#[derive(Clone)]
pub struct ChildDebugContextSpec {
    pub thread_id: i64,
    pub name: String,
    pub executor: AnyExecutor,
    pub tolk_source_map: Option<Arc<TolkSourceMap>>,
    pub stop_on_entry: bool,
}

pub trait DebugSession {
    fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<()>;

    fn need_to_stop_child_thread_on_start(&self) -> bool;

    fn begin_child_context(&mut self, spec: ChildDebugContextSpec) -> anyhow::Result<bool>;

    fn finish_child_context(&mut self, thread_id: i64) -> anyhow::Result<()>;

    fn step(&mut self, mode: StepMode) -> bool;

    fn active_context_is_terminated(&self) -> bool;

    fn performing_step(&self) -> Option<StepMode>;

    fn advance_parent_after_child_return(&mut self) -> anyhow::Result<()>;
}
