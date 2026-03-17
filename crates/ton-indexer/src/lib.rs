pub mod jettons;
pub mod nfts;

use base64::Engine;
use tycho_types::cell::HashBytes;

pub enum ContractType {
    Unknown,
    WalletV4R1,
    WalletV4R2,
    WalletV5Beta,
    WalletV5R1,
}

pub fn categorize_contract(hash: HashBytes) -> ContractType {
    let hash_str = base64::engine::general_purpose::STANDARD.encode(hash);

    match hash_str.as_str() {
        "ZN1UgFUixb6KnbWc6gEFzPDQh4bKeb64y3nogKjXMi0=" => ContractType::WalletV4R1,
        "/rX/aCDi/w2Ug+fg1iyBfYRniftK5YDIeIZtlZ2r1cA=" => ContractType::WalletV4R2,
        "89fKU0k97trCizgZhqhJQDy6w9LFhHea8IEGWvCsS5M=" => ContractType::WalletV5Beta,
        "IINLe3KxEhR+Gy+0V7hOdNGjDwT3N9T2KmaOlVLSty8=" => ContractType::WalletV5R1,
        _ => ContractType::Unknown,
    }
}
