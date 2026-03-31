use crate::AnyExecutor;
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
