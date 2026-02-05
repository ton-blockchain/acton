use crate::storage::TraceNode;
use serde_json::value::Value;

pub fn map_traces(tn: &TraceNode) -> Value {
    serde_json::json!({
        "traces": [
            map_trace(tn)
        ]
    })
}

fn map_trace(tn: &TraceNode) -> Value {
    serde_json::json!({
        "trace_id": tn.transaction.meta.tx_hash.to_hex(),
        "external_hash": tn.external_hash.as_ref().map(|h| h.to_hex()).unwrap_or_else(|| tn.transaction.meta.tx_hash.to_hex()),
        "mc_seqno_start": 0,
        "mc_seqno_end": 0,
        "start_lt": tn.transaction.meta.lt.to_string(),
        "start_utime": tn.transaction.meta.now,
        "end_lt": tn.max_lt().to_string(),
        "end_utime": tn.max_utime(),
        "is_incomplete": false,
        "trace": map_trace_node(tn),
    })
}

fn map_trace_node(tn: &TraceNode) -> Value {
    serde_json::json!({
        "tx_hash": tn.transaction.meta.tx_hash.to_hex(),
        "in_msg_hash": tn.transaction.meta.in_msg_hash.as_ref().map(|h| h.to_hex()).unwrap_or_default(),
        "children": tn.children.iter().map(map_trace_node).collect::<Vec<_>>(),
    })
}
