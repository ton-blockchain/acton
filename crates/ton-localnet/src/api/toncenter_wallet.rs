use crate::localnet::LocalnetAccountState;
use anyhow::Context;
use ton::ton_core::cell::{TonCell, TonHash};
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::{WalletV1V2Data, WalletV3Data, WalletV4Data, WalletV5Data, WalletVersion};

#[derive(Debug, Clone, Copy)]
pub(crate) struct StandardWalletState {
    pub version: WalletVersion,
    pub seqno: u32,
    pub wallet_id: Option<i32>,
}

pub(crate) fn read_standard_wallet_state(
    account: &LocalnetAccountState,
) -> anyhow::Result<StandardWalletState> {
    let code_hash = account
        .code_hash
        .as_ref()
        .context("Account state has no code")?;
    let code_hash = TonHash::from_vec(code_hash.0.to_vec())?;
    let version =
        WalletVersion::get_version_by_code(code_hash).context("Account is not a known wallet")?;
    let data = account.data.as_ref().context("Account state has no data")?;
    let data = TonCell::from_boc(data.0.clone()).context("Failed to decode wallet data")?;

    let (seqno, wallet_id) = match version {
        WalletVersion::V1R1
        | WalletVersion::V1R2
        | WalletVersion::V1R3
        | WalletVersion::V2R1
        | WalletVersion::V2R2 => {
            let data =
                WalletV1V2Data::from_cell(&data).context("Failed to parse wallet V1/V2 data")?;
            (data.seqno, None)
        }
        WalletVersion::V3R1 | WalletVersion::V3R2 => {
            let data = WalletV3Data::from_cell(&data).context("Failed to parse wallet V3 data")?;
            (data.seqno, Some(data.wallet_id))
        }
        WalletVersion::V4R1 | WalletVersion::V4R2 => {
            let data = WalletV4Data::from_cell(&data).context("Failed to parse wallet V4 data")?;
            (data.seqno, Some(data.wallet_id))
        }
        WalletVersion::V5R1 => {
            let data = WalletV5Data::from_cell(&data).context("Failed to parse wallet V5 data")?;
            (data.seqno, Some(data.wallet_id))
        }
        _ => anyhow::bail!("Unsupported wallet type: {version:?}"),
    };

    Ok(StandardWalletState {
        version,
        seqno,
        wallet_id,
    })
}
