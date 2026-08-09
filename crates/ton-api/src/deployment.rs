use anyhow::{Context, bail};
use std::collections::HashSet;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, HashBytes};
use tycho_types::models::{
    IntAddr, Message, MsgInfo, RelaxedMessage, RelaxedMsgInfo, StateInit, StdAddr,
};
use tycho_types::num::SplitDepth;

const MAX_BOC_BASE64_BYTES: usize = 2 * 1024 * 1024;
const MAX_CELLS: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_CANDIDATES: usize = 64;

/// A contract deployment proven by a message `StateInit`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeploymentCandidate {
    /// Canonical raw standard address (`workchain:account_id`).
    pub address: String,
    /// Representation hash of the deployed code as 64 lowercase hex characters.
    pub code_hash: String,
}

/// Extracts contract deployments from a `sendBoc` base64 payload.
///
/// The extractor performs a bounded walk over the `BoC` cell DAG, but only accepts cells that
/// decode through the canonical TON message types. A candidate is emitted only when the message
/// has a code-bearing `StateInit` whose representation hash matches the destination account id,
/// taking its fixed-prefix length into account. This lets wallet-wrapped deployments be found
/// without decoding any wallet-specific payload layout.
pub fn extract_deployment_candidates(boc_base64: &str) -> anyhow::Result<Vec<DeploymentCandidate>> {
    if boc_base64.len() > MAX_BOC_BASE64_BYTES {
        bail!("sendBoc payload exceeds {MAX_BOC_BASE64_BYTES} encoded bytes");
    }

    let root = Boc::decode_base64(boc_base64).context("failed to decode sendBoc payload")?;
    extract_from_root(root)
}

fn extract_from_root(root: Cell) -> anyhow::Result<Vec<DeploymentCandidate>> {
    let mut stack = vec![(root, 0usize)];
    let mut visited = HashSet::<HashBytes>::new();
    let mut candidates = Vec::new();
    let mut unique_candidates = HashSet::<DeploymentCandidate>::new();

    while let Some((cell, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            bail!("sendBoc cell DAG exceeds maximum depth {MAX_DEPTH}");
        }
        if !visited.insert(*cell.repr_hash()) {
            continue;
        }
        if visited.len() > MAX_CELLS {
            bail!("sendBoc cell DAG exceeds maximum size {MAX_CELLS}");
        }

        if let Some(candidate) = candidate_from_message(&cell)
            && unique_candidates.insert(candidate.clone())
        {
            candidates.push(candidate);
        }
        if let Some(candidate) = candidate_from_relaxed_message(&cell)
            && unique_candidates.insert(candidate.clone())
        {
            candidates.push(candidate);
        }
        if candidates.len() > MAX_CANDIDATES {
            bail!("sendBoc payload contains more than {MAX_CANDIDATES} deployments");
        }

        for index in (0..cell.reference_count()).rev() {
            if let Some(child) = cell.reference_cloned(index) {
                stack.push((child, depth + 1));
            }
        }
    }

    Ok(candidates)
}

fn candidate_from_message(cell: &Cell) -> Option<DeploymentCandidate> {
    let message = cell.parse::<Message<'_>>().ok()?;
    let destination = match &message.info {
        MsgInfo::Int(info) => std_address(&info.dst)?,
        MsgInfo::ExtIn(info) => std_address(&info.dst)?,
        MsgInfo::ExtOut(_) => return None,
    };
    candidate_from_state_init(destination, message.init.as_ref()?)
}

fn candidate_from_relaxed_message(cell: &Cell) -> Option<DeploymentCandidate> {
    let message = cell.parse::<RelaxedMessage<'_>>().ok()?;
    let RelaxedMsgInfo::Int(info) = &message.info else {
        return None;
    };
    candidate_from_state_init(std_address(&info.dst)?, message.init.as_ref()?)
}

const fn std_address(address: &IntAddr) -> Option<&StdAddr> {
    match address {
        IntAddr::Std(address) if address.anycast.is_none() => Some(address),
        IntAddr::Std(_) | IntAddr::Var(_) => None,
    }
}

fn candidate_from_state_init(
    destination: &StdAddr,
    state_init: &StateInit,
) -> Option<DeploymentCandidate> {
    let code = state_init.code.as_ref()?;
    let state_init_cell = CellBuilder::build_from(state_init.clone()).ok()?;
    if !state_init_matches_destination(
        state_init_cell.repr_hash(),
        &destination.address,
        state_init.split_depth,
    )? {
        return None;
    }

    Some(DeploymentCandidate {
        address: destination.to_string(),
        code_hash: code.repr_hash().to_string().to_ascii_lowercase(),
    })
}

fn state_init_matches_destination(
    state_init_hash: &HashBytes,
    destination: &HashBytes,
    fixed_prefix_length: Option<SplitDepth>,
) -> Option<bool> {
    let fixed_prefix_length = fixed_prefix_length
        .map(SplitDepth::into_bit_len)
        .unwrap_or_default();
    let state_init_hash = CellBuilder::build_from(*state_init_hash).ok()?;
    let destination = CellBuilder::build_from(*destination).ok()?;
    let mut state_init_hash = state_init_hash.as_slice().ok()?;
    let mut destination = destination.as_slice().ok()?;

    state_init_hash.skip_first(fixed_prefix_length, 0).ok()?;
    destination.skip_first(fixed_prefix_length, 0).ok()?;
    state_init_hash.contents_eq(&destination).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ton::ton_core::cell::TonCell;
    use ton::ton_core::traits::tlb::TLB;
    use ton::ton_core::types::TonAddress;
    use ton::ton_wallet::{Mnemonic, TonWallet, WalletVersion};
    use tycho_types::cell::CellSliceParts;
    use tycho_types::models::{
        CurrencyCollection, ExtInMsgInfo, MsgInfo, OwnedMessage, OwnedRelaxedMessage,
        RelaxedIntMsgInfo,
    };
    use tycho_types::num::Tokens;

    const TEST_MNEMONIC: &str = "fancy carpet hello mandate penalty trial consider property top vicious exit rebuild tragic profit urban major total month holiday sudden rib gather media vicious";

    fn deployment() -> (StateInit, StdAddr, String) {
        deployment_with_split_depth(None)
    }

    fn deployment_with_split_depth(
        split_depth: Option<SplitDepth>,
    ) -> (StateInit, StdAddr, String) {
        let code = CellBuilder::build_from(0xcafe_u16).expect("code");
        let data = CellBuilder::build_from(0xbeef_u16).expect("data");
        let state_init = StateInit {
            split_depth,
            code: Some(code.clone()),
            data: Some(data),
            ..Default::default()
        };
        let state_init_cell = CellBuilder::build_from(state_init.clone()).expect("state init cell");
        let address = StdAddr::new(0, *state_init_cell.repr_hash());
        let code_hash = code.repr_hash().to_string();
        (state_init, address, code_hash)
    }

    fn empty_body() -> CellSliceParts {
        Cell::default().into()
    }

    #[test]
    fn extracts_direct_external_in_deployment() {
        let (state_init, address, code_hash) = deployment();
        let message = CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                src: None,
                dst: IntAddr::Std(address.clone()),
                import_fee: Tokens::ZERO,
            }),
            init: Some(state_init),
            body: empty_body(),
            layout: None,
        })
        .expect("external message");

        let candidates = extract_deployment_candidates(&Boc::encode_base64(message))
            .expect("deployment extraction");
        assert_eq!(
            candidates,
            vec![DeploymentCandidate {
                address: address.to_string(),
                code_hash,
            }]
        );
        TonAddress::from_str(&candidates[0].address).expect("candidate address must be parseable");
    }

    #[test]
    fn accepts_destination_that_differs_only_in_fixed_prefix() {
        let (state_init, mut address, code_hash) =
            deployment_with_split_depth(Some(SplitDepth::new(5).expect("split depth")));
        address.address.as_mut_array()[0] ^= 0b1111_1000;

        assert_eq!(
            candidate_from_state_init(&address, &state_init),
            Some(DeploymentCandidate {
                address: address.to_string(),
                code_hash,
            })
        );
    }

    #[test]
    fn rejects_destination_that_differs_after_fixed_prefix() {
        let (state_init, mut address, _) =
            deployment_with_split_depth(Some(SplitDepth::new(5).expect("split depth")));
        address.address.as_mut_array()[0] ^= 0b0000_0100;

        assert!(candidate_from_state_init(&address, &state_init).is_none());
    }

    fn assert_wallet_wrapped_deployment(version: WalletVersion) {
        let (state_init, address, code_hash) = deployment();
        let internal = CellBuilder::build_from(OwnedRelaxedMessage {
            info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                bounce: false,
                src: Some(IntAddr::Std(StdAddr::new(0, HashBytes([7; 32])))),
                dst: IntAddr::Std(address.clone()),
                value: CurrencyCollection::ZERO,
                ..Default::default()
            }),
            init: Some(state_init),
            body: empty_body(),
            layout: None,
        })
        .expect("internal deployment message");
        let internal =
            TonCell::from_boc(Boc::encode(internal)).expect("convert internal message cell");

        let key_pair = Mnemonic::from_str(TEST_MNEMONIC, None)
            .expect("mnemonic")
            .to_key_pair()
            .expect("key pair");
        let wallet = TonWallet::new(version, key_pair).expect("wallet");
        let external = wallet
            .create_ext_in_msg(vec![internal], 1, 2_000_000_000, false)
            .expect("wallet external message")
            .to_boc_base64()
            .expect("wallet BoC");

        assert_eq!(
            extract_deployment_candidates(&external).expect("deployment extraction"),
            vec![DeploymentCandidate {
                address: address.to_string(),
                code_hash,
            }]
        );
    }

    #[test]
    fn extracts_deployment_from_wallet_v4r2_wrapped_message() {
        assert_wallet_wrapped_deployment(WalletVersion::V4R2);
    }

    #[test]
    fn extracts_deployment_from_wallet_v5r1_wrapped_message() {
        assert_wallet_wrapped_deployment(WalletVersion::V5R1);
    }

    #[test]
    fn rejects_state_init_that_does_not_match_destination() {
        let (state_init, _, _) = deployment();
        let message = CellBuilder::build_from(OwnedRelaxedMessage {
            info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                dst: IntAddr::Std(StdAddr::new(0, HashBytes([9; 32]))),
                value: CurrencyCollection::ZERO,
                ..Default::default()
            }),
            init: Some(state_init),
            body: empty_body(),
            layout: None,
        })
        .expect("internal message");

        assert!(
            extract_deployment_candidates(&Boc::encode_base64(message))
                .expect("deployment extraction")
                .is_empty()
        );
    }
}
