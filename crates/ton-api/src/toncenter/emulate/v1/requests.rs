use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulateRequest {
    #[serde(default)]
    pub boc: String,
    #[serde(default)]
    pub ignore_chksig: bool,
    #[serde(default)]
    pub include_code_data: bool,
    #[serde(default)]
    pub include_address_book: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub with_actions: bool,
    pub mc_block_seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonConnectEmulateRequest {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub messages: Vec<TonConnectMessage>,
    pub valid_until: Option<u64>,
    #[serde(default)]
    pub include_code_data: bool,
    #[serde(default)]
    pub include_address_book: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub with_actions: bool,
    pub mc_block_seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonConnectMessage {
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub amount: String,
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
