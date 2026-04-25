// Child debug contexts splice nested live executors into the current DAP session,
// e.g. when runtime helpers step into `net.send*` or `net.runGetMethod`.

use crate::DebugExecutorHandle;
use std::sync::Arc;
use tolk_compiler::TolkSourceMap;
use tolk_compiler::abi::ContractABI;

#[derive(Clone)]
pub struct ChildDebugContextSpec {
    /// DAP thread id for the nested context. Today nested runtimes are serialized,
    /// so one synthetic child thread id is sufficient.
    pub thread_id: i64,
    /// User-visible label shown in entry stops / fallback frame names.
    pub name: String,
    /// Live executor already prepared for stepping.
    pub executor: DebugExecutorHandle,
    /// Debug info for the code being entered. Without it we cannot create a replayer.
    pub source_map: Option<Arc<TolkSourceMap>>,
    /// Optional ABI used to render runtime storage / messages in "Registers".
    pub compiler_abi: Option<Arc<ContractABI>>,
    /// True when parent Step Into should land on the first user-visible child location.
    pub stop_on_entry: bool,
}
