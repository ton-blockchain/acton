use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::wallet_tlb::wallet_ext_msg_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Wallet v3 data.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV3Data {
    pub seqno: u32,
    pub wallet_id: i32,
    pub public_key: HashBytes,
}

impl WalletV3Data {
    /// Creates wallet data with sequence number zero.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
            wallet_id,
            public_key,
        }
    }
}

/// Unsigned wallet v3 external-message body.
///
/// Schema: <https://docs.ton.org/participate/wallets/contracts#wallet-v3>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV3ExtMsgBody {
    pub subwallet_id: i32,
    pub valid_until: u32,
    pub msg_seqno: u32,
    pub msgs_modes: Vec<u8>,
    pub msgs: Vec<Cell>,
}

impl<'a> Load<'a> for WalletV3ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let subwallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let (msgs_modes, msgs) = read_up_to_4_msgs(parser)?;
        Ok(Self {
            subwallet_id,
            msg_seqno,
            valid_until,
            msgs_modes,
            msgs,
        })
    }
}

impl Store for WalletV3ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        self.subwallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs, &self.msgs_modes)?;
        Ok(())
    }
}

impl WalletV3ExtMsgBody {
    /// Reads a signature followed by the message body.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((Self::load_from(parser)?, signature))
    }
}

#[cfg(test)]
mod tests {
    use crate::boc::{Boc, BocRepr};
    use crate::cell::{CellBuilder, HashBytes};
    use crate::wallet::{WALLET_ID_DEFAULT, WalletV3Data, WalletV3ExtMsgBody};
    use std::str::FromStr;

    #[test]
    fn test_wallet_v3_data() -> anyhow::Result<()> {
        // https://tonviewer.com/UQAMY2B4xfQO6m3YpmzfX5Za-Ning4kWKFjPdubbPPV3Ffel
        let src_boc_hex = "b5ee9c7241010101002a0000500000000129a9a317cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4b6de3f10";
        let wallet_data = BocRepr::decode_hex::<WalletV3Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 1);
        assert_eq!(wallet_data.wallet_id, WALLET_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        let serial_boc_hex = BocRepr::encode_hex(&wallet_data)?;
        let restored = BocRepr::decode_hex::<WalletV3Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_v3_ext_msg_body() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/b4bd316c74b4c99586e07c167979ce4a6e18db31704abd7e85b1cacb065ce66c
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c7201010201008500019a86be376ea96e2f1252377976716a3d252906151feabc8e4b51506405035e45a7b4ff81f783cfe3f86483c822bcbb4f9481804990868bac69caf7af56e30fe70b29a9a317ffffffff000000000301006642007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c187a120000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(512, 0)?;
        let body_no_sign = CellBuilder::build_from(parser.load_remaining())?;

        let body = WalletV3ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;
        assert_eq!(body.subwallet_id, WALLET_ID_DEFAULT);
        assert_eq!(body.msg_seqno, 0);
        assert_eq!(body.valid_until, 4294967295);
        assert_eq!(body.msgs_modes, vec![3]);
        assert_eq!(body.msgs.len(), 1);

        let serial_cell = CellBuilder::build_from(&body)?;
        assert_eq!(body_no_sign, serial_cell);
        Ok(())
    }
}
