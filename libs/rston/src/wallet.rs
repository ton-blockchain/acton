mod wallet_code;
mod wallet_constants;
mod wallet_tlb;
mod wallet_version;

pub use crate::mnemonic::*;
use ed25519_dalek::{Signer, SigningKey};
pub use wallet_code::*;
pub use wallet_constants::*;
pub use wallet_tlb::*;
pub use wallet_version::*;

use crate::cell::{Cell, CellBuilder};
use crate::error::WalletError;
use crate::models::{ExtInMsgInfo, MessageLayout, MsgInfo, OwnedMessage, StateInit, StdAddr};
use crate::num::Tokens;

/// A wallet contract derived from a version and key pair.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[non_exhaustive]
pub struct TonWallet {
    pub version: WalletVersion,
    pub key_pair: KeyPair,
    pub address: StdAddr,
    pub wallet_id: i32,
}

impl TonWallet {
    /// Creates a wallet with the default workchain and wallet ID.
    pub fn new(version: WalletVersion, key_pair: KeyPair) -> Result<Self, WalletError> {
        let wallet_id = match version {
            WalletVersion::V5R1 => WALLET_V5R1_ID_DEFAULT,
            _ => WALLET_ID_DEFAULT,
        };
        Self::new_with_params(version, key_pair, 0, wallet_id)
    }

    /// Creates a wallet from mnemonic credentials.
    pub fn new_with_creds(
        version: WalletVersion,
        seed: &str,
        pass: Option<String>,
    ) -> Result<Self, WalletError> {
        Self::new(version, Mnemonic::from_str(seed, pass)?.to_key_pair()?)
    }

    /// Creates a wallet with explicit workchain and wallet ID.
    pub fn new_with_params(
        version: WalletVersion,
        key_pair: KeyPair,
        workchain: i8,
        wallet_id: i32,
    ) -> Result<Self, WalletError> {
        let code = WalletVersion::get_code(version)?.clone();
        let data = WalletVersion::get_default_data(version, &key_pair, wallet_id)?;
        let state_init = StateInit {
            code: Some(code),
            data: Some(data),
            ..Default::default()
        };
        let address = StdAddr::new(workchain, *CellBuilder::build_from(state_init)?.repr_hash());

        Ok(TonWallet {
            key_pair,
            version,
            address,
            wallet_id,
        })
    }

    /// Creates and signs an external inbound message.
    pub fn create_ext_in_msg(
        &self,
        int_msgs: Vec<Cell>,
        seqno: u32,
        expire_at: u32,
        add_state_init: bool,
    ) -> Result<Cell, WalletError> {
        let body = self.create_ext_in_body(expire_at, seqno, int_msgs)?;
        let signed = self.sign_ext_in_body(&body)?;
        let external = self.create_ext_in_msg_from_body(signed, add_state_init)?;
        Ok(external)
    }

    /// Creates an unsigned external-message body.
    pub fn create_ext_in_body(
        &self,
        expire_at: u32,
        seqno: u32,
        int_msgs: Vec<Cell>,
    ) -> Result<Cell, WalletError> {
        WalletVersion::build_ext_in_body(self.version, expire_at, seqno, self.wallet_id, int_msgs)
    }

    /// Signs an external-message body.
    pub fn sign_ext_in_body(&self, ext_in_body: &Cell) -> Result<Cell, WalletError> {
        let msg_hash = ext_in_body.repr_hash();

        let signing_key =
            SigningKey::from_keypair_bytes(&self.key_pair.secret_key).map_err(|err| {
                WalletError::Custom(format!("Failed to parse Ed25519 keypair: {err}"))
            })?;

        if signing_key.verifying_key().to_bytes() != self.key_pair.public_key {
            return Err(WalletError::Custom(
                "Failed to parse Ed25519 keypair: mismatched public key".to_string(),
            ));
        }

        let sign = signing_key.sign(msg_hash.as_ref()).to_bytes().to_vec();
        WalletVersion::sign_msg(self.version, ext_in_body, &sign)
    }

    /// Wraps a signed body in an external inbound message.
    pub fn create_ext_in_msg_from_body(
        &self,
        signed_body: Cell,
        add_state_init: bool,
    ) -> Result<Cell, WalletError> {
        let msg_info = MsgInfo::ExtIn(ExtInMsgInfo {
            src: None,
            dst: self.address.clone().into(),
            import_fee: Tokens::ZERO,
        });

        let mut msg = OwnedMessage {
            info: msg_info,
            init: None,
            body: signed_body.into(),
            layout: Some(MessageLayout {
                init_to_cell: false,
                body_to_cell: true,
            }),
        };
        if add_state_init {
            let code = WalletVersion::get_code(self.version)?.clone();
            let data =
                WalletVersion::get_default_data(self.version, &self.key_pair, self.wallet_id)?;
            let state_init = StateInit {
                code: Some(code),
                data: Some(data),
                ..Default::default()
            };
            msg.init = Some(state_init);
        }
        Ok(CellBuilder::build_from(msg)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MNEMONIC_STR: &str = "fancy carpet hello mandate penalty trial consider property top vicious exit rebuild tragic profit urban major total month holiday sudden rib gather media vicious";
    const MNEMONIC_STR_V5: &str = "section garden tomato dinner season dice renew length useful spin trade intact use universe what post spike keen mandate behind concert egg doll rug";

    fn make_keypair(mnemonic_str: &str) -> KeyPair {
        let mnemonic = Mnemonic::from_str(mnemonic_str, None).unwrap();
        mnemonic.to_key_pair().unwrap()
    }

    #[test]
    fn test_ton_wallet_create() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);

        let wallet_v3 = TonWallet::new(WalletVersion::V3R1, key_pair.clone())?;
        let expected_v3 = StdAddr::from_str_ext(
            "EQBiMfDMivebQb052Z6yR3jHrmwNhw1kQ5bcAUOBYsK_VPuK",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v3.address, expected_v3);

        let wallet_v3r2 = TonWallet::new(WalletVersion::V3R2, key_pair.clone())?;
        let expected_v3r2 = StdAddr::from_str_ext(
            "EQA-RswW9QONn88ziVm4UKnwXDEot5km7GEEXsfie_0TFOCO",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v3r2.address, expected_v3r2);

        let wallet_v4r2 = TonWallet::new(WalletVersion::V4R2, key_pair.clone())?;
        let expected_v4r2 = StdAddr::from_str_ext(
            "EQCDM_QGggZ3qMa_f3lRPk4_qLDnLTqdi6OkMAV2NB9r5TG3",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v4r2.address, expected_v4r2);

        let key_pair_v5 = make_keypair(MNEMONIC_STR_V5);
        let wallet_v5 = TonWallet::new(WalletVersion::V5R1, key_pair_v5.clone())?;
        let expected_v5 = StdAddr::from_str_ext(
            "UQDv2YSmlrlLH3hLNOVxC8FcQf4F9eGNs4vb2zKma4txo6i3",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v5.address, expected_v5);

        let wallet_v5_testnet = TonWallet::new_with_params(
            WalletVersion::V5R1,
            key_pair_v5.clone(),
            0,
            WALLET_V5R1_ID_DEFAULT_TESTNET,
        )?;
        let expected_v5 = StdAddr::from_str_ext(
            "0QA_6fh0aRAkD7n1MNfAUx8TvyCUw2iTQfzVM-0isMze2anN",
            crate::models::StdAddrFormat::any(),
        )?
        .0;
        assert_eq!(wallet_v5_testnet.address, expected_v5);
        Ok(())
    }

    #[test]
    fn test_ton_wallet_debug() -> anyhow::Result<()> {
        let mut public_key = [0; ed25519_dalek::PUBLIC_KEY_LENGTH];
        public_key[..3].copy_from_slice(&[1, 2, 3]);

        let mut secret_key = [0; ed25519_dalek::KEYPAIR_LENGTH];
        secret_key[..3].copy_from_slice(&[4, 5, 6]);

        let key_pair = KeyPair {
            public_key,
            secret_key,
        };

        let wallet = TonWallet {
            key_pair,
            version: WalletVersion::V4R2,
            address: StdAddr::from_str_ext(
                "EQBiMfDMivebQb052Z6yR3jHrmwNhw1kQ5bcAUOBYsK_VPuK",
                crate::models::StdAddrFormat::any(),
            )?
            .0,
            wallet_id: 42,
        };

        let debug_output = format!("{wallet:?}");
        let expected_output = format!(
            "TonWallet {{ version: V4R2, key_pair: KeyPair {{ public_key: {:?}, secret_key: \"***REDACTED***\" }}, address: {:?}, wallet_id: 42 }}",
            public_key, wallet.address
        );
        assert_eq!(debug_output, expected_output);
        Ok(())
    }

    #[test]
    fn test_ton_wallet_create_external_msg_v3() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);
        let wallet = TonWallet::new(WalletVersion::V3R1, key_pair)?;

        let int_msg = CellBuilder::new().build()?;

        let ext_body_cell = wallet.create_ext_in_body(13, 7, vec![int_msg.clone()])?;
        let body = ext_body_cell.parse::<WalletV3ExtMsgBody>()?;
        let expected = WalletV3ExtMsgBody {
            subwallet_id: WALLET_ID_DEFAULT,
            msg_seqno: 7,
            valid_until: 13,
            msgs_modes: vec![3],
            msgs: vec![int_msg.clone()],
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_ton_wallet_create_external_msg_v4() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR);
        let wallet = TonWallet::new(WalletVersion::V4R1, key_pair)?;

        let int_msg = CellBuilder::new().build()?;

        let ext_body_cell = wallet.create_ext_in_body(13, 7, vec![int_msg.clone()])?;
        let body = ext_body_cell.parse::<WalletV4ExtMsgBody>()?;
        let expected = WalletV4ExtMsgBody {
            subwallet_id: WALLET_ID_DEFAULT,
            msg_seqno: 7,
            opcode: 0,
            valid_until: 13,
            msgs_modes: vec![3],
            msgs: vec![int_msg],
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_ton_wallet_create_external_msg_v5() -> anyhow::Result<()> {
        let key_pair = make_keypair(MNEMONIC_STR_V5);
        let wallet = TonWallet::new(WalletVersion::V5R1, key_pair)?;

        let msgs_cnt = 10usize;
        let mut int_msgs = vec![];
        for i in 0..msgs_cnt as u32 {
            let mut builder = CellBuilder::new();
            builder.store_u32(i)?;
            int_msgs.push(builder.build()?);
        }
        CellBuilder::new().build()?;

        let ext_body_cell = wallet.create_ext_in_body(13, 7, int_msgs.clone())?;
        let body = ext_body_cell.parse::<WalletV5ExtMsgBody>()?;
        let expected = WalletV5ExtMsgBody {
            wallet_id: WALLET_V5R1_ID_DEFAULT,
            msg_seqno: 7,
            valid_until: 13,
            msgs_modes: vec![3; msgs_cnt],
            msgs: int_msgs,
        };
        assert_eq!(body, expected);
        Ok(())
    }

    #[test]
    fn test_ton_wallet_create_external_msg_signed() -> anyhow::Result<()> {
        let key_pair_v3 = make_keypair(MNEMONIC_STR);
        let wallet_v3 = TonWallet::new(WalletVersion::V3R1, key_pair_v3)?;

        let key_pair_v5 = make_keypair(MNEMONIC_STR_V5);
        let wallet_v5 = TonWallet::new(WalletVersion::V5R1, key_pair_v5)?;

        let mut builder = CellBuilder::new();
        builder.store_u32(100)?;
        let msg = builder.build()?;

        for wallet in [wallet_v3, wallet_v5] {
            let body = wallet.create_ext_in_body(1, 3, vec![msg.clone()])?;
            let signed_msg = wallet.sign_ext_in_body(&body)?;

            let mut parser = signed_msg.as_slice()?;
            match wallet.version {
                WalletVersion::V5R1 => {
                    // sign in last 512 bits
                    let data_size_bits = signed_msg.bit_len() - 512;
                    let mut builder = CellBuilder::new();
                    builder.store_slice(parser.load_prefix(data_size_bits, 0)?)?;
                    while let Ok(cell_ref) = parser.load_reference_cloned() {
                        builder.store_reference(cell_ref.clone())?;
                    }
                    assert_eq!(body, builder.build()?)
                }
                _ => {
                    // sign in first 512 bits
                    parser.skip_first(512, 0)?;
                    assert_eq!(body, CellBuilder::build_from(parser.load_remaining())?);
                }
            }
        }
        Ok(())
    }
}
