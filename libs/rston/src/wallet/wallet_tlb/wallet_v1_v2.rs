use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::wallet_tlb::wallet_ext_msg_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Wallet v1/v2 data.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV1V2Data {
    pub seqno: u32,
    pub public_key: HashBytes,
}

impl WalletV1V2Data {
    /// Creates wallet data with sequence number zero.
    pub fn new(public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
            public_key,
        }
    }
}

/// Unsigned wallet v2 external-message body.
///
/// Schema: <https://docs.ton.org/participate/wallets/contracts#wallet-v2>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV2ExtMsgBody {
    pub msg_seqno: u32,
    pub valid_until: u32,
    pub msgs_modes: Vec<u8>,
    pub msgs: Vec<Cell>,
}

impl<'a> Load<'a> for WalletV2ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let msg_seqno = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let (msgs_modes, msgs) = read_up_to_4_msgs(parser)?;
        Ok(Self {
            msg_seqno,
            valid_until,
            msgs_modes,
            msgs,
        })
    }
}

impl Store for WalletV2ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        self.msg_seqno.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs, &self.msgs_modes)?;
        Ok(())
    }
}

impl WalletV2ExtMsgBody {
    /// Reads a signature followed by the message body.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((Self::load_from(parser)?, signature))
    }
}
