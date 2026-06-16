pub mod common;
mod content;
pub mod jettons;
pub mod multisigs;
pub mod nfts;
pub mod types;

use base64::Engine;
use tycho_types::cell::HashBytes;

pub enum WalletType {
    Unknown,
    WalletV1R1,
    WalletV1R2,
    WalletV1R3,
    WalletV2R1,
    WalletV2R2,
    WalletV3R1,
    WalletV3R2,
    WalletV4R1,
    WalletV4R2,
    WalletV5Beta,
    WalletV5R1,
    WalletHighloadV1R1,
    WalletHighloadV1R2,
    WalletHighloadV2,
    WalletHighloadV2R1,
    WalletHighloadV2R2,
    WalletHighloadV3R1,
    WalletPreprocessedV2,
    WalletVesting,
}

impl WalletType {
    #[must_use]
    pub const fn interface_name(&self) -> Option<&'static str> {
        match self {
            Self::Unknown => None,
            Self::WalletV1R1 => Some("wallet_v1r1"),
            Self::WalletV1R2 => Some("wallet_v1r2"),
            Self::WalletV1R3 => Some("wallet_v1r3"),
            Self::WalletV2R1 => Some("wallet_v2r1"),
            Self::WalletV2R2 => Some("wallet_v2r2"),
            Self::WalletV3R1 => Some("wallet_v3r1"),
            Self::WalletV3R2 => Some("wallet_v3r2"),
            Self::WalletV4R1 => Some("wallet_v4r1"),
            Self::WalletV4R2 => Some("wallet_v4r2"),
            Self::WalletV5Beta => Some("wallet_v5_beta"),
            Self::WalletV5R1 => Some("wallet_v5r1"),
            Self::WalletHighloadV1R1 => Some("wallet_highload_v1r1"),
            Self::WalletHighloadV1R2 => Some("wallet_highload_v1r2"),
            Self::WalletHighloadV2 => Some("wallet_highload_v2"),
            Self::WalletHighloadV2R1 => Some("wallet_highload_v2r1"),
            Self::WalletHighloadV2R2 => Some("wallet_highload_v2r2"),
            Self::WalletHighloadV3R1 => Some("wallet_highload_v3r1"),
            Self::WalletPreprocessedV2 => Some("wallet_preprocessed_v2"),
            Self::WalletVesting => Some("wallet_vesting"),
        }
    }
}

#[must_use]
pub fn categorize_wallet(hash: HashBytes) -> WalletType {
    let hash_str = base64::engine::general_purpose::STANDARD.encode(hash);

    match hash_str.as_str() {
        "oM/CxIruFqJx8s/AtzgtgXVs7LEBfQd/qqs7tgL2how=" => WalletType::WalletV1R1,
        "1JAvzJ+tdGmPqONTIgpo2g3PcuMryy657gQhfBfTBiw=" => WalletType::WalletV1R2,
        "WHzHie/xyE9G7DeX5F/ICaFP9a4k8eDHpqmcydyQYf8=" => WalletType::WalletV1R3,
        "XJpeaMEI4YchoHxC+ZVr+zmtd+xtYktgxXbsiO7mUyk=" => WalletType::WalletV2R1,
        "/pUw0yQ4Uwg+8u8LTCkIwKv2+hwx6iQ6rKpb+MfXU/E=" => WalletType::WalletV2R2,
        "thBBpYp5gLlG6PueGY48kE0keZ/6NldOpCUcQaVm9YE=" => WalletType::WalletV3R1,
        "hNr6RJ+Ypph3ibojI1gHK8D3bcRSQAKl0JGLmnXS1Zk=" => WalletType::WalletV3R2,
        "ZN1UgFUixb6KnbWc6gEFzPDQh4bKeb64y3nogKjXMi0=" => WalletType::WalletV4R1,
        "/rX/aCDi/w2Ug+fg1iyBfYRniftK5YDIeIZtlZ2r1cA=" => WalletType::WalletV4R2,
        "5M87L0xtamHqDytUR9JmeFsmrzY32y3u5rzRqoJvNBI="
        | "89fKU0k97trCizgZhqhJQDy6w9LFhHea8IEGWvCsS5M=" => WalletType::WalletV5Beta,
        "IINLe3KxEhR+Gy+0V7hOdNGjDwT3N9T2KmaOlVLSty8=" => WalletType::WalletV5R1,
        "2M27t58sXKpnesRQdwvgNRviHhJQSG3oXMUqoz3RZIQ=" => WalletType::WalletHighloadV1R1,
        "Dc7tISadZgE+lbGfu1xVpvAa2tQIN7qo5SHN46AqpGw=" => WalletType::WalletHighloadV1R2,
        "lJTRzI7fEvBWcaGpugmSEJbrUIEeGSTsZcPGKfu4CBI=" => WalletType::WalletHighloadV2,
        "jOtFs81LXMYOquHBO5wJI5Jnf+U2sumy2AG2Lv+TH+E=" => WalletType::WalletHighloadV2R1,
        "ID3U81ittJmTEpqpJcrDmRa2ig5PeNJujywraer6Vnk=" => WalletType::WalletHighloadV2R2,
        "EayteVWEQJDyg78ji8FEmHH3g+fMCXlAjT9IWUg+hSU=" => WalletType::WalletHighloadV3R1,
        "Reu86bXSNYhstr/hw62TtwjeBYJEiSNlye4N/kOct7U=" => WalletType::WalletPreprocessedV2,
        "tItTGr7DtxRjgpH3137W3J9qJynvyiBHcTc3TUrotZA=" => WalletType::WalletVesting,
        _ => WalletType::Unknown,
    }
}
