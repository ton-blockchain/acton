use rston::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use rston::error::Error;
use rston::models::{ExtInMsgInfo, Message, MsgInfo};
use rston::num::Tokens;

/// Computes the TEP-467 lookup key shared by external-message senders and indexers.
///
/// Normalization clears the source, import fee and state init, and stores the body
/// by reference. It preserves the destination and body, but does not validate a
/// signature or provide replay protection. Internal and external-out messages
/// return [`Error::InvalidTag`]. The original signed message is never modified.
pub fn normalized_external_message_hash(message: &Message<'_>) -> Result<HashBytes, Error> {
    let MsgInfo::ExtIn(info) = &message.info else {
        return Err(Error::InvalidTag);
    };
    let body = CellBuilder::build_from(message.body)?;
    let info = ExtInMsgInfo {
        src: None,
        dst: info.dst.clone(),
        import_fee: Tokens::ZERO,
    };

    let mut builder = CellBuilder::new();
    builder.store_small_uint(0b10, 2)?;
    info.store_into(&mut builder, Cell::empty_context())?;
    builder.store_bit_zero()?;
    builder.store_bit_one()?;
    builder.store_reference(body)?;
    Ok(*builder.build()?.repr_hash())
}
