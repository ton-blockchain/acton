use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::wallet_tlb::wallet_ext_msg_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Wallet v4 data.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV4Data {
    pub seqno: u32,
    pub wallet_id: i32,
    pub public_key: HashBytes,
    pub plugins: Option<Cell>,
}

impl WalletV4Data {
    /// Creates wallet data with sequence number zero and no plugins.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
            wallet_id,
            public_key,
            plugins: None,
        }
    }
}

/// Unsigned wallet v4 external-message body.
///
/// Schema: <https://docs.ton.org/participate/wallets/contracts#wallet-v4>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV4ExtMsgBody {
    pub subwallet_id: i32,
    pub valid_until: u32,
    pub msg_seqno: u32,
    pub opcode: u8,
    pub msgs_modes: Vec<u8>,
    pub msgs: Vec<Cell>,
}

impl<'a> Load<'a> for WalletV4ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let subwallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let opcode = Load::load_from(parser)?;
        if opcode != 0 {
            return Err(Error::InvalidTag);
        }
        let (msgs_modes, msgs) = read_up_to_4_msgs(parser)?;
        Ok(Self {
            subwallet_id,
            valid_until,
            msg_seqno,
            opcode,
            msgs_modes,
            msgs,
        })
    }
}

impl Store for WalletV4ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        if self.opcode != 0 {
            return Err(Error::InvalidTag);
        }
        self.subwallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        self.opcode.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs, &self.msgs_modes)?;
        Ok(())
    }
}

impl WalletV4ExtMsgBody {
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
    use crate::wallet::{WALLET_ID_DEFAULT, WalletV4Data, WalletV4ExtMsgBody};
    use std::str::FromStr;

    #[test]
    fn test_wallet_data_v4() -> anyhow::Result<()> {
        // https://tonviewer.com/UQCS65EGyiApUTLOYXDs4jOLoQNCE0o8oNnkmfIcm0iX5FRT
        let src_boc_hex = "b5ee9c7241010101002b0000510000001429a9a317cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f440a6c9f37d";
        let wallet_data = BocRepr::decode_hex::<WalletV4Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 20);
        assert_eq!(wallet_data.wallet_id, WALLET_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        assert_eq!(wallet_data.plugins, None);

        let serial_boc_hex = BocRepr::encode_hex(&wallet_data)?;
        let restored = BocRepr::decode_hex::<WalletV4Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v4() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/891dbceffb986251768d4c33bb8dcf11d522408ff78b8e683d135304ca377b8b
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c7201010201008700019c9dcd3a68926ad6fb9d094c5b72901bfc359ada50f22b648c6c2223c767135d397c7489c121071e45a5316a94a533d80c41450049ebeed406c419fea99117f40629a9a31767ad328900000013000301006842007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c200989680000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(512, 0)?;
        let body_no_sign = CellBuilder::build_from(parser.load_remaining())?;

        let body = WalletV4ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;
        assert_eq!(body.subwallet_id, WALLET_ID_DEFAULT);
        assert_eq!(body.valid_until, 1739403913);
        assert_eq!(body.msg_seqno, 19);
        assert_eq!(body.opcode, 0);
        assert_eq!(body.msgs_modes, vec![3]);
        assert_eq!(body.msgs.len(), 1);

        let serial_cell = CellBuilder::build_from(&body)?;
        assert_eq!(body_no_sign, serial_cell);
        Ok(())
    }
}
