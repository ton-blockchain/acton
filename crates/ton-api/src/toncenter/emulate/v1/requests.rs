use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateRequest {
    pub boc: Option<String>,
    pub ignore_chksig: Option<bool>,
    pub include_code_data: Option<bool>,
    pub include_address_book: Option<bool>,
    pub include_metadata: Option<bool>,
    pub with_actions: Option<bool>,
    pub mc_block_seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonConnectEmulateRequest {
    pub from: Option<String>,
    #[serde(default)]
    pub messages: Vec<TonConnectMessage>,
    pub valid_until: Option<u32>,
    pub include_code_data: Option<bool>,
    pub include_address_book: Option<bool>,
    pub include_metadata: Option<bool>,
    pub with_actions: Option<bool>,
    pub mc_block_seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonConnectMessage {
    pub address: Option<String>,
    pub amount: Option<String>,
    pub payload: Option<String>,
    #[serde(rename = "stateInit")]
    pub state_init: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ton_connect_request_uses_documented_state_init_name() {
        let request: TonConnectEmulateRequest = serde_json::from_value(serde_json::json!({
            "from": "EQB3",
            "messages": [{
                "address": "EQB4",
                "amount": "1000000000",
                "stateInit": "te6ccgEBAQEAAgAAAA=="
            }]
        }))
        .expect("TonConnect request must deserialize");

        assert_eq!(
            request.messages[0].state_init.as_deref(),
            Some("te6ccgEBAQEAAgAAAA==")
        );
    }
}
