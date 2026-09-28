use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::wallet_tlb::wallet_ext_msg_utils::*;

/// Wallet v5r1 data.
///
/// Schema: <https://github.com/ton-blockchain/wallet-contract-v5/blob/main/types.tlb#L29>
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV5Data {
    pub sign_allowed: bool,
    pub seqno: u32,
    pub wallet_id: i32,
    pub public_key: HashBytes,
    pub extensions: Option<Cell>,
}

impl WalletV5Data {
    /// Creates wallet data with signing enabled, sequence number zero, and no extensions.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            sign_allowed: true,
            seqno: 0,
            wallet_id,
            public_key,
            extensions: None,
        }
    }
}

/// Unsigned wallet v5r1 external-message body.
///
/// Schema: <https://github.com/ton-blockchain/wallet-contract-v5/blob/main/types.tlb>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV5ExtMsgBody {
    pub wallet_id: i32,
    pub valid_until: u32,
    pub msg_seqno: u32,
    pub msgs_modes: Vec<u8>,
    pub msgs: Vec<Cell>,
}

impl<'a> Load<'a> for WalletV5ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        if parser.load_u32()? != 0x7369676e {
            return Err(Error::InvalidTag);
        }
        let wallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let inner_request = InnerRequest::load_from(parser)?;
        let (msgs, msgs_modes) = parse_inner_request(inner_request)?;
        Ok(Self {
            wallet_id,
            valid_until,
            msg_seqno,
            msgs_modes,
            msgs,
        })
    }
}

impl Store for WalletV5ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        dst.store_u32(0x7369676e)?;
        self.wallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        let inner_req = build_inner_request(&self.msgs, &self.msgs_modes)?;
        inner_req.store_into(dst, context)?;
        Ok(())
    }
}

impl WalletV5ExtMsgBody {
    /// Reads the message body followed by its signature.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let body = Self::load_from(parser)?;
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((body, signature))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boc::{Boc, BocRepr};
    use crate::models::{OwnedRelaxedMessage, RelaxedMsgInfo};
    use crate::wallet::{WALLET_V5R1_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT_TESTNET};
    use std::str::FromStr;

    #[test]
    fn test_wallet_data_v5() -> anyhow::Result<()> {
        // https://tonviewer.com/UQDwj2jGHWEbPpDf0I2qktDwqtv6tBCfBVNH9gJEnM-QmHDa
        let src_boc_hex = "b5ee9c7241010101002b00005180000000bfffff88e5f9bbe4db9b026385fb9a446ee75d0a7bb1dd77956387b468eb01950900a4fa20cbe13a2a";
        let wallet_data = BocRepr::decode_hex::<WalletV5Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 1);
        assert_eq!(wallet_data.wallet_id, WALLET_V5R1_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        assert_eq!(wallet_data.extensions, None);

        let cell = CellBuilder::build_from(&wallet_data)?;
        let mut bytes = Vec::new();
        crate::boc::ser::BocHeader::<ahash::RandomState>::with_root(cell.as_ref())
            .with_crc(true)
            .encode(&mut bytes);
        let serial_boc_hex = hex::encode(bytes);
        assert_eq!(src_boc_hex, serial_boc_hex);
        let restored = BocRepr::decode_hex::<WalletV5Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_data_v5_testnet() -> anyhow::Result<()> {
        let src_boc_hex = "b5ee9c7201010101002b000051800000013ffffffed2b31b23dbe5144a626b9d5d1d4208e36d97e4adb472d42c073bfff85b3107e4a0";
        let wallet_data = BocRepr::decode_hex::<WalletV5Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 2);
        assert_eq!(wallet_data.wallet_id, WALLET_V5R1_ID_DEFAULT_TESTNET);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v5() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/b4c5eddc52d0e23dafb2da6d022a5b6ae7eba52876fa75d32b2a95fa30c7e2f0
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c720101040100940001a17369676e7fffff11ffffffff00000000bc04889cb28b36a3a00810e363a413763ec34860bf0fce552c5d36e37289fafd442f1983d740f92378919d969dd530aec92d258a0779fb371d4659f10ca1b3826001020a0ec3c86d030302006642007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c187a1200000000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(body_signed_cell.bit_len() - 512, 0)?;
        let mut sign = vec![0; 64];
        parser.load_raw(&mut sign, 512)?;

        let body = WalletV5ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;

        assert_eq!(body.wallet_id, WALLET_V5R1_ID_DEFAULT);
        assert_eq!(body.valid_until, 4294967295);
        assert_eq!(body.msg_seqno, 0);
        assert_eq!(body.msgs_modes, vec![3]);
        assert_eq!(body.msgs.len(), 1);

        let mut signed_builder = CellBuilder::new();
        signed_builder.store_slice(CellBuilder::build_from(&body)?.as_slice()?)?;
        signed_builder.store_raw(&sign, 512)?;
        let signed_serial = signed_builder.build()?;
        assert_eq!(body_signed_cell, signed_serial);
        let parsed_back = signed_serial.parse::<WalletV5ExtMsgBody>()?;
        assert_eq!(body, parsed_back);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v5_order() -> anyhow::Result<()> {
        let body_signed = Boc::decode_hex(
            "b5ee9c7201020a0100014a0001a17369676e7fffff1168f168d80000010db3172ba8e30d093aa36ec6e9e1f1f874cf38fb4a7914c66a0afcbd562ec6b54537629c135ce86700ae64b87dfc85545b34d561a3f7b429512c8cc02edbedc0c2a001020a0ec3c86d030203020a0ec3c86d03040500644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911389800000000000000000000000000020a0ec3c86d03060700644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911389000000000000000000000000000020a0ec3c86d03080900644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911388800000000000000000000000000000000644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911388000000000000000000000000000",
        )?;
        let mut parser = body_signed.as_slice()?;
        parser.skip_first(body_signed.bit_len() - 512, 0)?;
        let mut sign = vec![0; 64];
        parser.load_raw(&mut sign, 512)?;

        let body = WalletV5ExtMsgBody::read_signed(&mut body_signed.as_slice()?)?.0;
        assert_eq!(body.msgs.len(), 4);

        let RelaxedMsgInfo::Int(info) = body.msgs[0].parse::<OwnedRelaxedMessage>()?.info else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10000);
        let RelaxedMsgInfo::Int(info) = body.msgs[1].parse::<OwnedRelaxedMessage>()?.info else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10001);
        let RelaxedMsgInfo::Int(info) = body.msgs[2].parse::<OwnedRelaxedMessage>()?.info else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10002);
        let RelaxedMsgInfo::Int(info) = body.msgs[3].parse::<OwnedRelaxedMessage>()?.info else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10003);

        let mut signed_builder = CellBuilder::new();
        signed_builder.store_slice(CellBuilder::build_from(&body)?.as_slice()?)?;
        signed_builder.store_raw(&sign, 512)?;
        let signed_serial = signed_builder.build()?;
        assert_eq!(body_signed, signed_serial);
        let parsed_back = signed_serial.parse::<WalletV5ExtMsgBody>()?;
        assert_eq!(body, parsed_back);
        Ok(())
    }
}
