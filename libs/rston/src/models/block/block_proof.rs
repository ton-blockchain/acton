use super::{BlockId, BlockSignature};
use crate::cell::*;
use crate::dict::Dict;
use crate::error::Error;
use crate::models::shard::ValidatorBaseInfo;

/// Typed block proof.
#[derive(Clone, Debug)]
pub struct BlockProof {
    /// Id of the related block.
    pub proof_for: BlockId,
    /// Merkle proof cell.
    pub root: Cell,
    /// Optional references for the masterchain block.
    pub signatures: Option<BlockSignatures>,
}

impl BlockProof {
    const TAG: u8 = 0xc3;
}

impl Store for BlockProof {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let child_cell = match &self.signatures {
            Some(signatures) => {
                let mut builder = CellBuilder::new();
                ok!(signatures.store_into(&mut builder, context));
                Some(ok!(builder.build_ext(context)))
            }
            None => None,
        };

        ok!(builder.store_u8(Self::TAG));
        ok!(self.proof_for.store_into(builder, context));
        ok!(builder.store_reference(self.root.clone()));
        match child_cell {
            Some(cell) => {
                ok!(builder.store_bit_one());
                builder.store_reference(cell)
            }
            None => builder.store_bit_zero(),
        }
    }
}

impl<'a> Load<'a> for BlockProof {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_u8() {
            Ok(Self::TAG) => {}
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        }

        Ok(Self {
            proof_for: ok!(BlockId::load_from(slice)),
            root: ok!(slice.load_reference_cloned()),
            signatures: if ok!(slice.load_bit()) {
                match slice.load_reference() {
                    Ok(cell) => Some(ok!(cell.parse::<BlockSignatures>())),
                    Err(e) => return Err(e),
                }
            } else {
                None
            },
        })
    }
}

/// Masterchain block signatures.
#[derive(Debug, Clone, Store, Load)]
#[tlb(tag = "#11")]
pub struct BlockSignatures {
    /// Brief validator basic info.
    pub validator_info: ValidatorBaseInfo,
    /// Total number of signatures.
    pub signature_count: u32,
    /// Total validators weight.
    pub total_weight: u64,
    /// Block signatures from all signers.
    pub signatures: Dict<u16, BlockSignature>,
}
