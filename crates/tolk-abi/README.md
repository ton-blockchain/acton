# tolk-abi

Tolk ABI JSON types and runtime decoding of TON cells. The decoder reads the
type table and declarations from `ContractABI` and returns an `UnpackedValue`
tree; it does not need generated Rust bindings, the Tolk compiler, or source maps.

Use the workspace crate through a path dependency:

```toml
[dependencies]
tolk-abi = { path = "path/to/acton/crates/tolk-abi" }
tycho-types = { version = "0.3.0", features = ["bigint"] }
serde_json = "1"
anyhow = "1"
```

```rust
use tolk_abi::{ContractABI, TyIdx, UnpackedValue, unpack_from_slice};
use tycho_types::boc::Boc;

fn decode(abi_json: &str, boc: &[u8], ty_idx: TyIdx) -> anyhow::Result<UnpackedValue> {
    let abi: ContractABI = serde_json::from_str(abi_json)?;
    let cell = Boc::decode(boc)?;
    let mut slice = cell.as_slice()?;
    let value = unpack_from_slice(&mut slice, &abi, ty_idx)?;
    Ok(value)
}
```

Select `ty_idx` at runtime from the ABI, for example from
`abi.incoming_messages[i].body_ty_idx` or `abi.storage.storage_ty_idx`. Pass the
cell containing that value, such as a message body or contract storage.
`UnpackSchema` also allows other schema providers to use the same decoder.

Supported layouts include fixed and variable width integers, coins, booleans,
addresses, strings, structs with prefixes, aliases, enums, nullable values,
unions, tuples, arrays, Lisp lists, dictionaries, raw cells and `Cell<T>`
references. ABI generic instantiations and client field types are resolved at
runtime. Numbers use `num_bigint::BigInt`; struct fields preserve their names.

This implements Tolk's standard cell layouts. It rejects unresolved generics,
`int`, `slice`, `builder`, `callable`, `unknown`, and arbitrary custom unpack
functions (the built-in `TlbVarUint7` and `TlbVarUint3` aliases are supported).
Dictionary keys must have a supported fixed width. Prefix mismatches and missing
cell data return errors, with field context for nested structures.

`unpack_from_slice` advances the supplied slice. It does not require complete
consumption of the root or `Cell<T>` payload, so check remaining bits and
references when exact consumption is required. The crate exposes decoding, not
a runtime ABI serializer or a TVM stack decoder.
