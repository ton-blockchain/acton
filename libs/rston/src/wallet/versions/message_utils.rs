//! Mode/reference streams shared by V2, V3, and V4 transfer requests.
//!
//! Each entry stores an 8-bit mode and a reference to a complete outgoing message.
//! There is no count prefix. The remaining references determine the entry count.
//! A cell can hold at most four references, which limits these wallets to four messages.
//! V5 uses its own linked `OutList` format instead.

use crate::cell::{Cell, CellBuilder, CellFamily, CellSlice, Load, Store};
use crate::error::Error;
use crate::wallet::WalletMessage;

/// Appends the mode/reference stream used by V2–V4 transfer bodies.
/// An excessive count fails before any write. Cell errors can leave a partially written stream.
pub(super) fn write_up_to_4_msgs(
    dst: &mut CellBuilder,
    msgs: &[WalletMessage],
) -> Result<(), Error> {
    if msgs.len() > 4 {
        return Err(Error::TooManyMessages {
            actual: msgs.len(),
            max: 4,
        });
    }

    for msg in msgs {
        msg.store_into(dst, Cell::empty_context())?;
    }
    Ok(())
}

/// Reads one mode/reference pair for each remaining reference, preserving order.
/// The caller must position the slice at the message stream. Unrelated trailing bits are left unread.
pub(super) fn read_up_to_4_msgs(parser: &mut CellSlice<'_>) -> Result<Vec<WalletMessage>, Error> {
    (0..parser.size_refs())
        .map(|_| WalletMessage::load_from(parser))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::wallet::SendMsgFlags;

    #[test]
    fn test_write_up_to_4_msgs() -> anyhow::Result<()> {
        let mut builder = CellBuilder::new();
        let msgs = vec![
            WalletMessage {
                mode: SendMsgFlags::PAY_FEE_SEPARATELY,
                msg: Cell::empty_cell(),
            },
            WalletMessage {
                mode: SendMsgFlags::IGNORE_ERROR,
                msg: Cell::empty_cell(),
            },
        ];
        write_up_to_4_msgs(&mut builder, &msgs)?;
        assert_eq!(read_up_to_4_msgs(&mut builder.build()?.as_slice()?)?, msgs);

        let mut builder = CellBuilder::new();
        let msg = WalletMessage {
            mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
            msg: Cell::empty_cell(),
        };
        let msgs = vec![msg.clone(); 4];
        assert!(write_up_to_4_msgs(&mut builder, &msgs).is_ok());

        let mut builder = CellBuilder::new();
        let msgs = vec![msg; 5];
        assert_eq!(
            write_up_to_4_msgs(&mut builder, &msgs),
            Err(Error::TooManyMessages { actual: 5, max: 4 })
        );

        Ok(())
    }
}
