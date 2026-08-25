use crate::{AbiError, Cell, StdAddr};
use tycho_types::cell::{CellBuilder, HashBytes};

/// Initial state attached to a contract created from deployment storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractInit {
    pub code: Cell,
    pub data: Cell,
}

/// Forces the calculated address into the same shard prefix as `close_to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToShard {
    pub fixed_prefix_length: u8,
    pub close_to: StdAddr,
}

/// Address and code options used when a generated wrapper is created from storage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployedAddressOptions {
    pub workchain: i8,
    pub to_shard: Option<ToShard>,
    pub override_contract_code: Option<Cell>,
}

/// Decodes the contract code embedded in a Tolk ABI.
pub fn decode_code_boc64(code_boc64: &str) -> Result<Cell, AbiError> {
    tycho_types::boc::Boc::decode_base64(code_boc64)
        .map_err(|error| AbiError::InvalidData(format!("invalid contract code BOC: {error}")))
}

/// Calculates the deployed address exactly like the upstream Tolk wrapper generator.
pub fn calculate_deployed_address(
    code: &Cell,
    data: &Cell,
    options: &DeployedAddressOptions,
) -> Result<StdAddr, AbiError> {
    let mut state_init = CellBuilder::new();
    if let Some(to_shard) = &options.to_shard {
        if to_shard.fixed_prefix_length > 31 {
            return Err(AbiError::InvalidData(
                "fixed shard prefix length exceeds 31 bits".to_owned(),
            ));
        }
        state_init.store_bit(true)?;
        state_init.store_small_uint(to_shard.fixed_prefix_length, 5)?;
    } else {
        state_init.store_bit(false)?;
    }
    state_init.store_bit(false)?; // special:(Maybe TickTock)
    state_init.store_bit(true)?;
    state_init.store_reference(code.clone())?;
    state_init.store_bit(true)?;
    state_init.store_reference(data.clone())?;
    state_init.store_bit(false)?; // library:(HashmapE 256 SimpleLib)

    let state_init = state_init.build()?;
    let state_hash = *state_init.repr_hash();
    let address = if let Some(to_shard) = &options.to_shard {
        splice_hash_prefix(
            state_hash,
            to_shard.close_to.address,
            to_shard.fixed_prefix_length,
        )
    } else {
        state_hash
    };
    Ok(StdAddr::new(options.workchain, address))
}

fn splice_hash_prefix(mut hash: HashBytes, close_to: HashBytes, prefix_len: u8) -> HashBytes {
    let whole_bytes = usize::from(prefix_len / 8);
    hash.0[..whole_bytes].copy_from_slice(&close_to.0[..whole_bytes]);

    let remaining_bits = prefix_len % 8;
    if remaining_bits != 0 {
        let mask = u8::MAX << (8 - remaining_bits);
        hash.0[whole_bytes] = (close_to.0[whole_bytes] & mask) | (hash.0[whole_bytes] & !mask);
    }
    hash
}
