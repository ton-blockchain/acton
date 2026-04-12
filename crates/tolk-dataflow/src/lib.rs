//! Dataflow framework for Tolk analyses.
//!
//! This crate provides:
//! - CFG builder for function/method/get-method bodies;
//! - generic fixed-point solver (forward/backward);

pub mod builder;
pub mod cfg;
pub mod solver;

pub use builder::{
    build_cfg_for_function, build_cfg_for_function_with_source, build_cfg_for_top_level,
    build_cfg_for_top_level_with_source,
};
pub use cfg::{
    ControlFlowGraph, DotOptions, EdgeKind, FlowEdge, FlowNode, FlowNodeKind, FlowNodeTaintFacts,
    NodeId,
};
pub use solver::{
    DataflowAnalysis, DataflowResult, Direction, SolverConfig, solve, solve_with_config,
};

#[cfg(test)]
mod tests {
    use crate::builder::build_cfg_for_top_level;
    use crate::cfg::EdgeKind;
    use tolk_resolver::FileResolveIndex;
    use tolk_syntax::{TopLevel, parse};

    #[test]
    fn builds_cfg_for_function_body() {
        let source = r"
            fun main(x: int) {
                var y = x;
                if (y > 0_) {
                    y = y - 1;
                } else {
                    y = y + 1;
                }

                while (y > 0) {
                    y = y - 0__1;
                }
            }
        ";

        let file = parse(source).expect("failed to parse");
        let top_level = file
            .top_levels()
            .find(|top_level| matches!(top_level, TopLevel::Func(_)))
            .expect("function is expected");

        let file_index = FileResolveIndex {
            file_id: 0,
            locals: vec![],
            uses: vec![],
        };
        let cfg = build_cfg_for_top_level(&top_level, &file_index).expect("cfg is expected");

        assert!(cfg.node_count() > 2);
        assert!(cfg.edge_count() > 2);

        let has_true_branch = cfg
            .edges()
            .iter()
            .any(|edge| edge.kind == EdgeKind::TrueBranch);
        let has_false_branch = cfg
            .edges()
            .iter()
            .any(|edge| edge.kind == EdgeKind::FalseBranch);
        let has_loop_back = cfg
            .edges()
            .iter()
            .any(|edge| edge.kind == EdgeKind::LoopBack);

        assert!(has_true_branch);
        assert!(has_false_branch);
        assert!(has_loop_back);
    }

    #[test]
    fn exports_cfg_to_dot() {
        let source = r"
            fun main(x: int) {
                var y = x;
                if (y > 0) {
                    y = y - 1;
                } else {
                    y = y + 1;
                }
                return y;
            }
        ";

        let file = parse(source).expect("failed to parse");
        let top_level = file
            .top_levels()
            .find(|top_level| matches!(top_level, TopLevel::Func(_)))
            .expect("function is expected");

        let file_index = FileResolveIndex {
            file_id: 0,
            locals: vec![],
            uses: vec![],
        };
        let cfg = build_cfg_for_top_level(&top_level, &file_index).expect("cfg is expected");
        let dot = cfg.to_dot();

        assert!(dot.contains("digraph tolk_cfg"));
        assert!(dot.contains("TrueBranch"));
        assert!(dot.contains("FalseBranch"));
        assert!(dot.contains("Return"));
    }

    #[test]
    fn exports_complex_cfg_to_dot() {
        let source = r"
            fun main(seed: int) {
                var acc = seed;
                var i = 0;

                repeat (3) {
                    if (acc > 1___0) {
                        acc -= 1;
                    } else {
                        acc += 2;
                    }
                }

                while (i < 4) {
                    i += 1;

                    try {
                        assert (acc > -100, 101);
                        match (acc) {
                            0 => { throw 7; }
                            1 => { acc = acc + i; }
                            else => {
                                if (i > 2) {
                                    acc = acc - i;
                                } else {
                                    acc = acc + 1;
                                }
                            }
                        }
                    } catch (e, arg) {
                        acc = e;
                    }
                }

                do {
                    acc = acc + 1;
                } while (acc < 20);

                if (acc < 0) {
                    throw 99;
                }

                return acc;
            }
        ";

        let file = parse(source).expect("failed to parse");
        let top_level = file
            .top_levels()
            .find(|top_level| matches!(top_level, TopLevel::Func(_)))
            .expect("function is expected");

        let file_index = FileResolveIndex {
            file_id: 0,
            locals: vec![],
            uses: vec![],
        };
        let cfg = build_cfg_for_top_level(&top_level, &file_index).expect("cfg is expected");
        let dot = cfg.to_dot();

        assert!(cfg.node_count() > 20, "node_count={}", cfg.node_count());
        assert!(cfg.edge_count() > 25, "edge_count={}", cfg.edge_count());

        assert!(dot.contains("TrueBranch"));
        assert!(dot.contains("FalseBranch"));
        assert!(dot.contains("LoopBack"));
        assert!(dot.contains("Exceptional"));
        assert!(dot.contains("Throw"));
        assert!(dot.contains("Return"));
        assert!(dot.contains("Condition"));
        assert!(dot.contains("Join"));
    }
}
