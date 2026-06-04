use expect_test::Expect;
use ton_indexer::actions::{Extraction, Trace, TraceNode, extract_actions};

macro_rules! trace {
    ($source:literal) => {
        crate::common::parse_trace($source)
    };
}

pub(crate) use trace;

pub(crate) fn check_extraction(trace: Trace, expected: Expect) {
    let extraction = extract_actions(&trace);
    expected.assert_eq(&format_extraction(&extraction));
}

fn format_extraction(extraction: &Extraction) -> String {
    let actions = extraction
        .actions
        .iter()
        .map(|action| {
            format!(
                "{:?} nodes={:?} base_actions={:?}",
                action.kind, action.nodes, action.base_actions
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let base_actions = extraction
        .base_actions
        .iter()
        .map(|action| {
            format!(
                "#{} {:?} nodes={:?} root={} user_facing={}",
                action.id, action.kind, action.nodes, action.root_node, action.user_facing
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("actions:\n{actions}\n\nbase_actions:\n{base_actions}\n")
}

pub(crate) fn parse_trace(source: &str) -> Trace {
    let lines = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let common_indent = lines
        .iter()
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or_default();
    let entries = lines
        .into_iter()
        .map(|line| &line[common_indent..])
        .map(parse_trace_line)
        .collect::<Vec<_>>();

    let mut position = 0;
    let root = build_trace_node(&entries, &mut position, 0);
    assert_eq!(position, entries.len(), "trace contains more than one root");

    Trace { root }
}

fn parse_trace_line(line: &str) -> (usize, TraceNode) {
    let Some(edge_start) = line.find(['├', '└']) else {
        return (0, parse_trace_node(line));
    };

    let depth = edge_start / 4 + 1;
    let content = line[edge_start..]
        .trim_start_matches(['├', '└', '─'])
        .trim();

    (depth, parse_trace_node(content))
}

fn parse_trace_node(content: &str) -> TraceNode {
    let (opcode, id) = content
        .rsplit_once('#')
        .unwrap_or_else(|| panic!("trace line must end with #id: {content}"));
    let id = id
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("trace line has invalid #id: {content}"));
    let opcode = opcode.trim();

    TraceNode {
        id,
        opcode_name: (!opcode.is_empty()).then(|| opcode.to_string()),
        children: Vec::new(),
    }
}

fn build_trace_node(
    entries: &[(usize, TraceNode)],
    position: &mut usize,
    expected_depth: usize,
) -> TraceNode {
    let Some((depth, node)) = entries.get(*position) else {
        panic!("expected node at depth {expected_depth}");
    };
    assert_eq!(*depth, expected_depth, "unexpected trace indentation");

    *position += 1;
    let mut node = node.clone();

    while entries
        .get(*position)
        .is_some_and(|(depth, _)| *depth > expected_depth)
    {
        node.children
            .push(build_trace_node(entries, position, expected_depth + 1));
    }

    node
}
