use super::Finality;
use crate::toncenter::v3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Subscribed,
    Unsubscribed,
    Pong,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: Status,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub error: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notification {
    Transactions {
        finality: Finality,
        trace_external_hash_norm: String,
        transactions: Vec<v3::Transaction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address_book: Option<v3::AddressBook>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<v3::Metadata>,
    },
    Actions {
        finality: Finality,
        trace_external_hash_norm: String,
        actions: Vec<v3::Action>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address_book: Option<v3::AddressBook>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<v3::Metadata>,
    },
    Trace {
        finality: Finality,
        trace_external_hash_norm: String,
        trace: Box<v3::TraceNode>,
        transactions: HashMap<String, v3::Transaction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<v3::Action>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address_book: Option<v3::AddressBook>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<v3::Metadata>,
    },
    AccountStateChange {
        finality: Finality,
        account: String,
        state: v3::AccountState,
    },
    JettonsChange {
        finality: Finality,
        jetton: v3::JettonWallet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address_book: Option<v3::AddressBook>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<v3::Metadata>,
    },
    TraceInvalidated {
        trace_external_hash_norm: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_response_matches_streaming_protocol() {
        let value = serde_json::to_value(StatusResponse {
            id: Some("ping-42".to_owned()),
            status: Status::Pong,
        })
        .expect("status response must serialize");

        assert_eq!(
            value,
            serde_json::json!({"id": "ping-42", "status": "pong"})
        );
    }

    #[test]
    fn trace_invalidated_notification_matches_reference() {
        let notification: Notification = serde_json::from_value(serde_json::json!({
            "type": "trace_invalidated",
            "trace_external_hash_norm": "normalized-hash"
        }))
        .expect("notification must deserialize");

        assert!(matches!(
            notification,
            Notification::TraceInvalidated { .. }
        ));
    }
}
