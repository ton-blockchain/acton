use crate::localnet::LocalnetAccountState;
use crate::types::Hash256;
use anyhow::Context;
use rston::boc::Boc;
use ton::ton_core::cell::{TonCell, TonHash};
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::WalletVersion;

const V2_WALLET_V5_BETA_CODE_HASH: &str = "89fKU0k97trCizgZhqhJQDy6w9LFhHea8IEGWvCsS5M=";

#[derive(Debug, Clone, Copy)]
pub(crate) struct StandardWalletState {
    pub version: WalletVersion,
    pub seqno: u32,
    pub wallet_id: Option<i32>,
    pub is_signature_allowed: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum V2WalletVersion {
    V1R1,
    V1R2,
    V1R3,
    V2R1,
    V2R2,
    V3R1,
    V3R2,
    V4R1,
    V4R2,
    V5Beta,
    V5R1,
}

impl From<V2WalletVersion> for toncenter::v2::responses::WalletInformationWalletType {
    fn from(version: V2WalletVersion) -> Self {
        match version {
            V2WalletVersion::V1R1 => Self::WalletV1R1,
            V2WalletVersion::V1R2 => Self::WalletV1R2,
            V2WalletVersion::V1R3 => Self::WalletV1R3,
            V2WalletVersion::V2R1 => Self::WalletV2R1,
            V2WalletVersion::V2R2 => Self::WalletV2R2,
            V2WalletVersion::V3R1 => Self::WalletV3R1,
            V2WalletVersion::V3R2 => Self::WalletV3R2,
            V2WalletVersion::V4R1 => Self::WalletV4R1,
            V2WalletVersion::V4R2 => Self::WalletV4R2,
            V2WalletVersion::V5Beta => Self::WalletV5Beta,
            V2WalletVersion::V5R1 => Self::WalletV5R1,
        }
    }
}

impl TryFrom<WalletVersion> for V2WalletVersion {
    type Error = anyhow::Error;

    fn try_from(version: WalletVersion) -> Result<Self, Self::Error> {
        Ok(match version {
            WalletVersion::V1R1 => Self::V1R1,
            WalletVersion::V1R2 => Self::V1R2,
            WalletVersion::V1R3 => Self::V1R3,
            WalletVersion::V2R1 => Self::V2R1,
            WalletVersion::V2R2 => Self::V2R2,
            WalletVersion::V3R1 => Self::V3R1,
            WalletVersion::V3R2 => Self::V3R2,
            WalletVersion::V4R1 => Self::V4R1,
            WalletVersion::V4R2 => Self::V4R2,
            WalletVersion::V5R1 => Self::V5R1,
            _ => anyhow::bail!("Unsupported V2 wallet type: {version:?}"),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct V2WalletState {
    pub version: V2WalletVersion,
    pub seqno: u32,
    pub wallet_id: Option<i32>,
    pub is_signature_allowed: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
enum V2WalletCode {
    Standard {
        version: WalletVersion,
        v2_version: V2WalletVersion,
    },
    V5Beta,
}

pub(crate) fn read_standard_wallet_state(
    account: &LocalnetAccountState,
) -> anyhow::Result<StandardWalletState> {
    let code_hash = account_code_hash(account)?;
    let code_hash = TonHash::from_vec(code_hash.0.to_vec())?;
    let version =
        WalletVersion::get_version_by_code(code_hash).context("Account is not a known wallet")?;
    read_standard_wallet_data(account, version)
}

pub(crate) fn account_code_hash(account: &LocalnetAccountState) -> anyhow::Result<Hash256> {
    account
        .code
        .as_ref()
        .context("Account state has no code")?
        .hash()
        .context("Failed to decode account code")
}

fn read_standard_wallet_data(
    account: &LocalnetAccountState,
    version: WalletVersion,
) -> anyhow::Result<StandardWalletState> {
    let data = account.data.as_ref().context("Account state has no data")?;
    let data = TonCell::from_boc(data.0.clone()).context("Failed to decode wallet data")?;
    let mut parser = data.parser();
    let parsed: anyhow::Result<_> = (|| {
        Ok(match version {
            WalletVersion::V1R1
            | WalletVersion::V1R2
            | WalletVersion::V1R3
            | WalletVersion::V2R1
            | WalletVersion::V2R2 => (parser.read_num::<u32>(32)?, None, None),
            WalletVersion::V3R1
            | WalletVersion::V3R2
            | WalletVersion::V4R1
            | WalletVersion::V4R2 => (
                parser.read_num::<u32>(32)?,
                Some(parser.read_num::<i32>(32)?),
                None,
            ),
            WalletVersion::V5R1 => {
                let is_signature_allowed = parser.read_bit()?;
                let seqno = parser.read_num::<u32>(32)?;
                let wallet_id = parser.read_num::<i32>(32)?;
                (seqno, Some(wallet_id), Some(is_signature_allowed))
            }
            _ => anyhow::bail!("Unsupported wallet type: {version:?}"),
        })
    })();
    let (seqno, wallet_id, is_signature_allowed) =
        parsed.with_context(|| format!("Failed to parse {version:?} wallet data"))?;

    Ok(StandardWalletState {
        version,
        seqno,
        wallet_id,
        is_signature_allowed,
    })
}

pub(crate) fn read_v2_wallet_state(
    account: &LocalnetAccountState,
) -> anyhow::Result<Option<V2WalletState>> {
    let Some(code) = classify_v2_wallet_code(account)? else {
        return Ok(None);
    };

    match code {
        V2WalletCode::Standard {
            version,
            v2_version,
        } => read_v2_standard_wallet_data(account, version, v2_version).map(Some),
        V2WalletCode::V5Beta => read_v2_beta_wallet_data(account).map(Some),
    }
}

pub(crate) fn read_v2_extended_wallet_state(
    account: &LocalnetAccountState,
) -> anyhow::Result<Option<V2WalletState>> {
    let Some(V2WalletCode::Standard {
        version,
        v2_version,
    }) = classify_v2_wallet_code(account)?
    else {
        return Ok(None);
    };
    if !matches!(
        v2_version,
        V2WalletVersion::V3R1 | V2WalletVersion::V3R2 | V2WalletVersion::V4R2
    ) {
        return Ok(None);
    }

    read_v2_standard_wallet_data(account, version, v2_version).map(Some)
}

fn classify_v2_wallet_code(account: &LocalnetAccountState) -> anyhow::Result<Option<V2WalletCode>> {
    if account.code.is_none() {
        return Ok(None);
    }

    classify_v2_wallet_code_hash(account_code_hash(account)?)
}

fn classify_v2_wallet_code_hash(code_hash: Hash256) -> anyhow::Result<Option<V2WalletCode>> {
    let ton_hash = TonHash::from_vec(code_hash.0.to_vec())?;
    if let Ok(version) = WalletVersion::get_version_by_code(ton_hash) {
        return Ok(V2WalletVersion::try_from(version).ok().map(|v2_version| {
            V2WalletCode::Standard {
                version,
                v2_version,
            }
        }));
    }

    Ok((code_hash.to_base64() == V2_WALLET_V5_BETA_CODE_HASH).then_some(V2WalletCode::V5Beta))
}

fn read_v2_standard_wallet_data(
    account: &LocalnetAccountState,
    version: WalletVersion,
    v2_version: V2WalletVersion,
) -> anyhow::Result<V2WalletState> {
    let state = read_standard_wallet_data(account, version)?;
    Ok(V2WalletState {
        version: v2_version,
        seqno: state.seqno,
        wallet_id: state.wallet_id,
        is_signature_allowed: state.is_signature_allowed,
    })
}

fn read_v2_beta_wallet_data(account: &LocalnetAccountState) -> anyhow::Result<V2WalletState> {
    let data = account.data.as_ref().context("Account state has no data")?;
    let cell = Boc::decode(data).context("Failed to decode wallet V5 beta data")?;
    let mut data = cell
        .as_slice()
        .context("Failed to load wallet V5 beta data")?;
    let is_signature_allowed = data.load_bit()?;
    let seqno = data.load_u32()?;
    let wallet_id = i32::from_be_bytes(data.load_u32()?.to_be_bytes());
    Ok(V2WalletState {
        version: V2WalletVersion::V5Beta,
        seqno,
        wallet_id: Some(wallet_id),
        is_signature_allowed: Some(is_signature_allowed),
    })
}
