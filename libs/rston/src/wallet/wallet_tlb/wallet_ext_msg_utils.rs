use crate::cell::{Cell, CellBuilder, CellContext, CellFamily, CellSlice, Load, Store};
use crate::error::Error;
use crate::models::OutAction;

pub(super) fn write_up_to_4_msgs(
    dst: &mut CellBuilder,
    msgs: &[Cell],
    msgs_modes: &[u8],
) -> Result<(), Error> {
    validate_msgs_count(msgs, msgs_modes, 4)?;
    for (msg, mode) in msgs.iter().zip(msgs_modes.iter()) {
        dst.store_u8(*mode)?;
        dst.store_reference(msg.to_owned())?;
    }
    Ok(())
}

pub(super) fn read_up_to_4_msgs(parser: &mut CellSlice<'_>) -> Result<(Vec<u8>, Vec<Cell>), Error> {
    let msgs_cnt = parser.size_refs() as usize;
    let mut msgs_modes = Vec::with_capacity(msgs_cnt);
    let mut msgs = Vec::with_capacity(msgs_cnt);
    for _ in 0..msgs_cnt {
        msgs_modes.push(Load::load_from(parser)?);
        msgs.push(parser.load_reference_cloned()?);
    }
    Ok((msgs_modes, msgs))
}

pub(super) fn validate_msgs_count(
    msgs: &[Cell],
    msgs_modes: &[u8],
    max_cnt: usize,
) -> Result<(), Error> {
    if msgs.len() > max_cnt || msgs_modes.len() != msgs.len() {
        return Err(Error::InvalidData);
    }
    Ok(())
}

// V5 support
// https://github.com/ton-blockchain/wallet-contract-v5/blob/88557ebc33047a95207f6e47ac8aadb102dff744/types.tlb#L26
#[derive(Debug, PartialEq, Clone)]
pub(super) struct InnerRequest {
    out_actions: Option<Cell>, // there is Option<TLBRef<OutList>>, but we don't support such description in TLBDerive
                               // other_actions: Option<()> unsupported
}

impl<'a> Load<'a> for InnerRequest {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        if !parser.load_bit()? {
            return Ok(Self { out_actions: None });
        }
        let out_actions = parser.load_reference_cloned()?;
        if parser.load_bit()? {
            return Err(Error::InvalidData);
        }
        Ok(Self {
            out_actions: Some(out_actions),
        })
    }
}

impl Store for InnerRequest {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_bit(self.out_actions.is_some())?;
        if let Some(actions) = &self.out_actions {
            builder.store_reference(actions.clone())?;
        }
        builder.store_bit(false)?; // other_actions are not supported
        Ok(())
    }
}

pub(super) fn parse_inner_request(request: InnerRequest) -> Result<(Vec<Cell>, Vec<u8>), Error> {
    let mut out_list = match request.out_actions {
        Some(out_list) => out_list,
        None => return Ok((vec![], vec![])),
    };
    let mut msgs = vec![];
    let mut msgs_modes = vec![];
    while out_list.bit_len() != 0 {
        let mut parser = out_list.as_slice()?;
        let next = parser.load_reference_cloned()?;
        if parser.load_u32()? == OutAction::TAG_SEND_MSG {
            let mode = parser.load_u8()?;
            msgs.push(parser.load_reference_cloned()?);
            msgs_modes.push(mode);
        } else {
            return Err(Error::InvalidData);
        }
        out_list = next;
    }

    msgs.reverse();
    msgs_modes.reverse();
    Ok((msgs, msgs_modes))
}

pub(super) fn build_inner_request(msgs: &[Cell], msgs_modes: &[u8]) -> Result<InnerRequest, Error> {
    if msgs.is_empty() {
        return Ok(InnerRequest { out_actions: None });
    }

    validate_msgs_count(msgs, msgs_modes, 255)?;
    let mut actions = Cell::empty_cell();
    for (msg, mode) in msgs.iter().zip(msgs_modes) {
        let mut builder = CellBuilder::new();
        builder.store_reference(actions)?;
        builder.store_u32(OutAction::TAG_SEND_MSG)?;
        builder.store_u8(*mode)?;
        builder.store_reference(msg.clone())?;
        actions = builder.build()?;
    }

    let out_list = actions;

    Ok(InnerRequest {
        out_actions: Some(out_list),
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_write_up_to_4_msgs() -> anyhow::Result<()> {
        let mut builder = CellBuilder::new();
        let msgs = vec![Cell::empty_cell().to_owned(), Cell::empty_cell().to_owned()];
        let msgs_modes = vec![1, 2];
        assert!(write_up_to_4_msgs(&mut builder, &msgs, &msgs_modes).is_ok());

        let mut builder = CellBuilder::new();
        let msgs = vec![Cell::empty_cell().to_owned(); 4];
        let msgs_modes = vec![1, 2, 3, 4];
        assert!(write_up_to_4_msgs(&mut builder, &msgs, &msgs_modes).is_ok());

        let mut builder = CellBuilder::new();
        let msgs = vec![Cell::empty_cell().to_owned(); 4];
        let msgs_modes = vec![1, 2, 3, 4, 5];
        assert!(write_up_to_4_msgs(&mut builder, &msgs, &msgs_modes).is_err());

        Ok(())
    }
}
