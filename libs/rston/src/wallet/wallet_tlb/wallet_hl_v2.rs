use crate::cell::{Cell, HashBytes, Load, Store};

/// Highload v2r2 wallet data.
#[derive(Clone, Debug, Load, Store)]
pub struct WalletHLV2R2Data {
    pub wallet_id: i32,
    pub last_cleaned_time: u64,
    pub public_key: HashBytes,
    pub queries: Option<Cell>,
}

impl WalletHLV2R2Data {
    /// Creates initial data with zero cleanup time and no processed queries.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            wallet_id,
            last_cleaned_time: 0,
            public_key,
            queries: None,
        }
    }
}
