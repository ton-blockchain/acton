use tolk_dataflow::cfg::{ControlFlowGraph, FlowNodeKind, NodeId};
use tolk_dataflow::solver::{DataflowAnalysis, DataflowResult, Direction, solve};
use tolk_resolver::Span;

/// Forward authorization state:
/// `sender_checked` is true when execution is proven to pass an admin sender check
/// (`assert(in.senderAddress == *.adminAddress)` in the current approximation).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdminAuthorizationState {
    pub sender_checked: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdminAuthorizationAnalysis;

impl DataflowAnalysis for AdminAuthorizationAnalysis {
    type State = AdminAuthorizationState;

    fn direction(&self) -> Direction {
        Direction::Forward
    }

    fn bottom(&self, _cfg: &ControlFlowGraph) -> Self::State {
        // "Top" for must-analysis, so merge can apply conjunction.
        AdminAuthorizationState {
            sender_checked: true,
        }
    }

    fn boundary(&self, _cfg: &ControlFlowGraph) -> Self::State {
        AdminAuthorizationState {
            sender_checked: false,
        }
    }

    fn merge(&self, into: &mut Self::State, other: &Self::State) -> bool {
        let merged = into.sender_checked && other.sender_checked;
        let changed = merged != into.sender_checked;
        into.sender_checked = merged;
        changed
    }

    fn transfer(&self, cfg: &ControlFlowGraph, node: NodeId, state: &Self::State) -> Self::State {
        let flow_node = cfg.node(node);
        let mut next = *state;

        if flow_node.kind == FlowNodeKind::Assert && flow_node.taint.has_admin_sender_check {
            next.sender_checked = true;
        }

        next
    }
}

/// Single sink report for storage writes potentially reachable without admin guard.
#[derive(Debug, Clone)]
pub struct UncheckedStorageWrite {
    pub node: NodeId,
    pub span: Option<Span>,
}

/// Runs admin-authorization propagation.
#[must_use]
pub fn run_admin_authorization(cfg: &ControlFlowGraph) -> DataflowResult<AdminAuthorizationState> {
    solve(cfg, &AdminAuthorizationAnalysis)
}

/// Finds storage write sinks that are reachable without guaranteed admin sender check.
#[must_use]
pub fn find_unchecked_storage_writes(
    cfg: &ControlFlowGraph,
    dataflow: &DataflowResult<AdminAuthorizationState>,
) -> Vec<UncheckedStorageWrite> {
    let mut issues = Vec::new();

    for node in cfg.nodes() {
        if !node.taint.has_storage_write_sink {
            continue;
        }

        if dataflow.in_at(node.id).sender_checked {
            continue;
        }

        issues.push(UncheckedStorageWrite {
            node: node.id,
            span: node.span,
        });
    }

    issues
}

#[derive(Debug, Clone)]
pub struct AdminAuthorizationReport {
    pub dataflow: DataflowResult<AdminAuthorizationState>,
    pub issues: Vec<UncheckedStorageWrite>,
}

#[must_use]
pub fn run(cfg: &ControlFlowGraph) -> AdminAuthorizationReport {
    let dataflow = run_admin_authorization(cfg);
    let issues = find_unchecked_storage_writes(cfg, &dataflow);
    AdminAuthorizationReport { dataflow, issues }
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tolk_dataflow::ControlFlowGraph;
    use tolk_dataflow::build_cfg_for_top_level_with_source;
    use tolk_resolver::{FileDb, ProjectIndex, resolve};
    use tolk_syntax::TopLevel;

    fn unique_temp_file_path() -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let pid = std::process::id();
        std::env::temp_dir().join(format!("tolk_dataflow_admin_guard_{pid}_{ts}.tolk"))
    }

    fn build_cfg_with_resolution(source: &str) -> ControlFlowGraph {
        let path = unique_temp_file_path();
        std::fs::write(&path, source).expect("write source");

        let file_db = FileDb::new(PathBuf::from("/__dummy_stdlib__"), None);
        let mut project = ProjectIndex::builder(&file_db, path.clone())
            .build()
            .expect("build project index");
        resolve(&file_db, &mut project);

        let canonical = file_db.canonicalize(&path).expect("canonical path");
        let file_id = project.get_file_by_path(&canonical).expect("file id");
        let file = file_db.get_by_id(file_id).expect("file info");
        let top_level = file
            .source()
            .top_levels()
            .find(|top_level| matches!(top_level, TopLevel::Func(_)))
            .expect("function is expected");

        let resolve_index = project
            .get_resolved_uses(file_id)
            .expect("resolve index for file");

        let cfg =
            build_cfg_for_top_level_with_source(&top_level, resolve_index.as_ref(), Some(source))
                .expect("cfg is expected");

        let _ = std::fs::remove_file(path);
        cfg
    }

    #[test]
    fn reports_storage_write_without_admin_check() {
        let source = r"
            struct Storage {
                adminAddress: address
            }

            fun onInternalMessage(in: InMessage) {
                val storage = lazy Storage.fromCell(contract.getData());
                storage.save();
            }
        ";

        let cfg = build_cfg_with_resolution(source);
        let report = run(&cfg);

        assert_eq!(report.issues.len(), 1);
    }

    #[test]
    fn reports_contract_set_data_without_admin_check() {
        let source = r"
            fun onInternalMessage(in: InMessage) {
                val _sender = in.senderAddress;
                contract.setData(contract.getData());
            }
        ";

        let cfg = build_cfg_with_resolution(source);
        let report = run(&cfg);

        assert_eq!(report.issues.len(), 1);
    }

    #[test]
    fn does_not_report_storage_write_after_admin_assert() {
        let source = r"
            struct Storage {
                adminAddress: address
            }

            fun onInternalMessage(in: InMessage) {
                val storage = lazy Storage.fromCell(contract.getData());
                assert (in.senderAddress == storage.adminAddress) throw 5;
                storage.save();
            }
        ";

        let cfg = build_cfg_with_resolution(source);
        let report = run(&cfg);

        assert!(report.issues.is_empty());
    }

    #[test]
    fn does_not_treat_plain_comparison_as_admin_check() {
        let source = r"
            struct Storage {
                adminAddress: address
            }

            fun onInternalMessage(in: InMessage) {
                val storage = lazy Storage.fromCell(contract.getData());
                val isAdmin = in.senderAddress == storage.adminAddress;
                storage.save();
            }
        ";

        let cfg = build_cfg_with_resolution(source);
        let report = run(&cfg);

        assert_eq!(report.issues.len(), 1);
    }
}
