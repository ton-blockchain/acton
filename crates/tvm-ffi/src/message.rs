//! Message-body views shared by host-side decoders and transaction matchers.

#[cfg(test)]
mod tests;

use tycho_types::cell::CellSlice;

/// Returns the original payload for opcode lookup and ABI decoding.
///
/// The caller decides whether bounce unwrapping is appropriate, normally from the
/// internal message's `bounced` flag. Legacy `0xffffffff` bodies keep the payload
/// inline; rich `0xfffffffe` bodies store it in their first reference. Only one
/// envelope is removed. Other tags retain the existing 32-bit prefix handling
/// when the caller explicitly requests bounce unwrapping.
///
/// This borrows the payload without validating the rich bounce's diagnostic fields.
/// A missing or unreadable original-body reference returns `None`, so callers never
/// mistake phase/exit-code data for the original message opcode.
#[must_use]
pub fn original_message_body(body: CellSlice<'_>, bounced: bool) -> Option<CellSlice<'_>> {
    if !bounced {
        return Some(body);
    }

    let mut parser = body;
    match parser.load_u32().ok()? {
        0xffff_fffe => parser.load_reference().ok()?.as_slice().ok(),
        _ => Some(parser),
    }
}
