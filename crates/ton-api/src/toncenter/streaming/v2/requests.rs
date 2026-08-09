use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Finality {
    Pending,
    Confirmed,
    #[default]
    Finalized,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Transactions,
    Actions,
    Trace,
    AccountStateChange,
    JettonsChange,
    TraceInvalidated,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Subscription {
    #[serde(default)]
    pub types: Vec<EventType>,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub trace_external_hash_norms: Vec<String>,
    pub min_finality: Option<Finality>,
    #[serde(default)]
    pub action_types: Vec<String>,
    #[serde(default)]
    pub supported_action_types: Vec<String>,
    pub include_address_book: Option<bool>,
    pub include_metadata: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UnsubscribeRequest {
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub trace_external_hash_norms: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "lowercase")]
pub enum WebSocketRequest {
    Subscribe {
        id: Option<String>,
        #[serde(flatten)]
        subscription: Subscription,
    },
    Unsubscribe {
        id: Option<String>,
        #[serde(flatten)]
        request: UnsubscribeRequest,
    },
    Ping {
        id: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_subscribe_request_is_tagged_by_operation() {
        let request: WebSocketRequest = serde_json::from_value(serde_json::json!({
            "operation": "subscribe",
            "id": "1",
            "types": ["transactions"],
            "addresses": ["EQB3"],
            "min_finality": "confirmed"
        }))
        .expect("documented WebSocket request must deserialize");

        let WebSocketRequest::Subscribe { id, subscription } = request else {
            panic!("expected subscribe request");
        };
        assert_eq!(id.as_deref(), Some("1"));
        assert_eq!(subscription.types, vec![EventType::Transactions]);
        assert_eq!(subscription.min_finality, Some(Finality::Confirmed));
    }
}
