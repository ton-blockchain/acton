//! ABI field filters for storage streams. Only fingerprints survive a publication;
//! the original data cell is sent unchanged when a selected value differs.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail, ensure};
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, HashBytes};
use rston::models::AnyAddr;
use sha2::{Digest, Sha256};
use tolk_source_map::abi::{ABICustomPackUnpack, ContractABI};
use tolk_source_map::dynamic_unpack::{UnpackSchema, UnpackedValue, unpack_from_slice_with_limits};
use tolk_source_map::types_kernel::{Ty, TyIdx};

const MAX_FIELDS: usize = 64;
const MAX_SCHEMA_DEPTH: usize = 32;
const MAX_VALUES: usize = 16_384;

/// One ABI and selection shared by all accounts on a connection. Baselines are
/// independent per account and advance even when only unselected values change.
pub(super) struct StorageWatch {
    abi: ContractABI,
    root: TyIdx,
    fields: Vec<(String, Vec<String>)>,
    baselines: HashMap<String, Baseline>,
}

struct Baseline {
    seqno: u32,
    data: Option<HashBytes>,
    fields: Vec<HashBytes>,
}

impl StorageWatch {
    pub(super) fn new(abi: ContractABI, fields: Vec<String>) -> Result<Self> {
        ensure!(
            abi.abi_schema_version == "1.0",
            "unsupported ABI schema version"
        );
        ensure!(
            serde_json::to_vec(&abi)?.len() <= 256 * 1024,
            "ABI exceeds 256 KiB"
        );
        ensure!(
            !fields.is_empty() && fields.len() <= MAX_FIELDS,
            "expected 1 to 64 fields"
        );
        ensure!(
            abi.unique_types.len() <= 1024 && abi.declarations.len() <= 1024,
            "ABI type limit exceeded"
        );
        let root = abi
            .storage
            .storage_ty_idx
            .context("ABI has no storage type")?;
        validate_type(&abi, root, &mut Vec::new(), &mut 4096)?;
        let mut unique = HashSet::new();
        let mut selected = Vec::new();
        for field in fields {
            ensure!(field.len() <= 256, "field path exceeds 256 bytes");
            ensure!(unique.insert(field.clone()), "duplicate field path");
            let mut ty_idx = root;
            let mut path = Vec::new();
            for name in field.split('.') {
                ensure!(!name.is_empty(), "empty field path component");
                // Typed cells are transparent in user paths, although the shared
                // decoder represents their contents as an object with a ref field.
                loop {
                    match abi.ty_by_idx(ty_idx).context("missing field type")? {
                        Ty::AliasRef { .. } => {
                            ty_idx = abi
                                .alias_target_for(ty_idx)
                                .context("missing alias target")?
                                .ty_idx
                        }
                        Ty::Nullable { inner_ty_idx, .. } => ty_idx = *inner_ty_idx,
                        Ty::CellOf { inner_ty_idx } => {
                            path.push("ref".to_owned());
                            ty_idx = *inner_ty_idx;
                        }
                        _ => break,
                    }
                }
                let fields = abi
                    .struct_fields_for(ty_idx)
                    .context("field paths must traverse structs")?;
                let field = fields
                    .iter()
                    .find(|field| field.name == name)
                    .context("unknown storage field")?;
                path.push(name.to_owned());
                ty_idx = field.ty_idx;
            }
            selected.push((field, path));
        }
        Ok(Self {
            abi,
            root,
            fields: selected,
            baselines: HashMap::new(),
        })
    }

    /// Returns an initial snapshot or changed selected fields. A checkpoint at
    /// or before the baseline is ignored: subscription setup may pin a checkpoint
    /// whose batch is still being published. Missing data clears every selection.
    pub(super) fn update(
        &mut self,
        address: &str,
        seqno: u32,
        data: Option<&Cell>,
    ) -> Result<Option<(bool, Vec<String>)>> {
        let hash = data.map(|data| *data.repr_hash());
        let previous = self.baselines.get_mut(address);
        if let Some(previous) = previous {
            if seqno <= previous.seqno {
                return Ok(None);
            }
            if hash == previous.data {
                previous.seqno = seqno;
                return Ok(None);
            }
        }
        let value = data
            .map(|data| {
                // Limit both raw input and expansion of shared cells/zero-width types.
                ensure!(
                    Boc::encode(data).len() <= 700 * 1024,
                    "storage data exceeds 700 KiB"
                );
                let mut slice = data.as_slice()?;
                let value = unpack_from_slice_with_limits(
                    &mut slice,
                    &self.abi,
                    self.root,
                    MAX_VALUES,
                    1024 * 1024,
                )?;
                ensure!(slice.is_empty(), "storage data does not match ABI layout");
                Ok(value)
            })
            .transpose()?;
        let mut fingerprints = Vec::with_capacity(self.fields.len());
        for (_, path) in &self.fields {
            let mut value = value.as_ref();
            for component in path {
                value = match value {
                    Some(UnpackedValue::Object { fields, .. }) => Some(
                        &fields
                            .iter()
                            .find(|(name, _)| name == component)
                            .context("storage field is unavailable")?
                            .1,
                    ),
                    None | Some(UnpackedValue::Null) => None,
                    _ => bail!("storage field does not match ABI layout"),
                };
            }
            let mut hasher = Sha256::new();
            hasher.update([u8::from(data.is_some())]);
            if let Some(value) = value {
                fingerprint(value, &mut hasher)?;
            }
            fingerprints.push(HashBytes(hasher.finalize().into()));
        }
        let previous = self.baselines.get(address);
        let initial = previous.is_none();
        let changed = self
            .fields
            .iter()
            .zip(&fingerprints)
            .enumerate()
            .filter(|(index, (_, hash))| {
                previous.is_some_and(|previous| previous.fields[*index] != **hash)
            })
            .map(|(_, ((name, _), _))| name.clone())
            .collect::<Vec<_>>();
        self.baselines.insert(
            address.to_owned(),
            Baseline {
                seqno,
                data: hash,
                fields: fingerprints,
            },
        );
        Ok((initial || !changed.is_empty()).then_some((initial, changed)))
    }
}

/// Validate the reachable graph before invoking the shared decoder. Recursive
/// schemas and custom serializers need execution semantics that this stream does
/// not provide. Bounds also protect helper routines that resolve aliases/labels.
fn validate_type(
    abi: &ContractABI,
    ty_idx: TyIdx,
    stack: &mut Vec<TyIdx>,
    remaining: &mut usize,
) -> Result<()> {
    ensure!(
        *remaining > 0 && stack.len() < MAX_SCHEMA_DEPTH,
        "ABI complexity limit exceeded"
    );
    *remaining -= 1;
    ensure!(
        !stack.contains(&ty_idx),
        "recursive storage schemas are unsupported"
    );
    let ty = abi.ty_by_idx(ty_idx).context("missing ABI type")?;
    let mut children = Vec::new();
    match ty {
        Ty::UintN { n } => ensure!(*n <= 256, "invalid uint width"),
        Ty::IntN { n } => ensure!(*n > 0 && *n <= 257, "invalid int width"),
        Ty::BitsN { n } => ensure!(*n <= 1023, "invalid bits width"),
        Ty::VarintN { n } | Ty::VaruintN { n } => {
            ensure!(n.is_power_of_two() && *n <= 32, "invalid varint width")
        }
        Ty::StructRef {
            struct_name,
            type_args_ty_idx,
        } => {
            ensure!(struct_name.len() <= 256, "struct name exceeds 256 bytes");
            let decl = abi
                .struct_decl_info(struct_name)
                .context("missing struct declaration")?;
            standard_layout(decl.custom_pack_unpack)?;
            if let Some(prefix) = decl.prefix {
                ensure!(
                    (0..=64).contains(&prefix.prefix_len),
                    "invalid struct prefix"
                );
            }
            let fields = abi
                .struct_fields_for(ty_idx)
                .context("missing struct fields")?;
            let mut names = HashSet::new();
            for field in fields {
                ensure!(field.name.len() <= 256, "field name exceeds 256 bytes");
                ensure!(names.insert(field.name), "duplicate struct field");
                children.push(field.ty_idx);
                children.extend(field.u_label_ty_idx);
            }
            children.extend(type_args_ty_idx.iter().flatten().copied());
        }
        Ty::AliasRef {
            alias_name,
            type_args_ty_idx,
        } => {
            let decl = abi
                .alias_decl_info(alias_name)
                .context("missing alias declaration")?;
            standard_layout(decl.custom_pack_unpack)?;
            let target = abi
                .alias_target_for(ty_idx)
                .context("missing alias target")?;
            children.push(target.ty_idx);
            children.extend(target.u_label_ty_idx);
            children.extend(type_args_ty_idx.iter().flatten().copied());
        }
        Ty::EnumRef { enum_name } => {
            let decl = abi
                .enum_decl_info(enum_name)
                .context("missing enum declaration")?;
            standard_layout(decl.custom_pack_unpack)?;
            children.push(decl.encoded_as_ty_idx);
        }
        Ty::CellOf { inner_ty_idx }
        | Ty::Nullable { inner_ty_idx, .. }
        | Ty::ArrayOf { inner_ty_idx }
        | Ty::LispListOf { inner_ty_idx } => children.push(*inner_ty_idx),
        Ty::MapKV {
            key_ty_idx,
            value_ty_idx,
        } => children.extend([*key_ty_idx, *value_ty_idx]),
        Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
            children.extend(items_ty_idx)
        }
        Ty::Union { variants, .. } => {
            for variant in variants {
                ensure!(variant.prefix_len <= 64, "invalid union prefix");
                children.push(variant.variant_ty_idx);
            }
        }
        // Generic type parameters can appear in label metadata, but decoding an
        // unresolved parameter as an actual value still fails in the shared decoder.
        Ty::GenericT { .. }
        | Ty::Bool
        | Ty::Coins
        | Ty::Cell
        | Ty::String
        | Ty::Remaining
        | Ty::Address
        | Ty::AddressOpt
        | Ty::AddressExt
        | Ty::AddressAny
        | Ty::NullLiteral
        | Ty::Void => {}
        _ => bail!("unsupported storage type"),
    }
    stack.push(ty_idx);
    for child in children {
        validate_type(abi, child, stack, remaining)?;
    }
    stack.pop();
    Ok(())
}

fn standard_layout(custom: Option<&ABICustomPackUnpack>) -> Result<()> {
    ensure!(
        !custom.is_some_and(|custom| custom.unpack_from_slice.unwrap_or(false)
            || custom.pack_to_builder.unwrap_or(false)),
        "custom storage serializers are unsupported"
    );
    Ok(())
}

/// Length-delimited, tagged semantic fingerprints avoid storing decoded account
/// data between blocks. Raw cells use their representation hash, not `BoC` bytes.
fn fingerprint(value: &UnpackedValue, hash: &mut Sha256) -> Result<()> {
    let mut bytes = |tag: u8, bytes: &[u8]| {
        hash.update([tag]);
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    };
    match value {
        UnpackedValue::Null => bytes(0, &[]),
        UnpackedValue::Void => bytes(1, &[]),
        UnpackedValue::Number(value) => bytes(2, &value.to_signed_bytes_be()),
        UnpackedValue::Bool(value) => bytes(3, &[u8::from(*value)]),
        UnpackedValue::String(value) => bytes(4, value.as_bytes()),
        UnpackedValue::AddressNone => bytes(5, &[]),
        UnpackedValue::Address(value) => {
            bytes(6, CellBuilder::build_from(value)?.repr_hash().as_slice())
        }
        UnpackedValue::ExtAddress(value) => bytes(
            7,
            CellBuilder::build_from(AnyAddr::Ext(value.clone()))?
                .repr_hash()
                .as_slice(),
        ),
        UnpackedValue::Cell(value) => bytes(8, value.repr_hash().as_slice()),
        UnpackedValue::RemainingBitsAndRefs(value) => bytes(9, value.repr_hash().as_slice()),
        UnpackedValue::Bits((value, len)) => {
            bytes(10, value);
            hash.update((*len as u64).to_be_bytes());
        }
        UnpackedValue::Array(values) => {
            bytes(11, &(values.len() as u64).to_be_bytes());
            for value in values {
                fingerprint(value, hash)?;
            }
        }
        UnpackedValue::Map(values) => {
            bytes(12, &(values.len() as u64).to_be_bytes());
            for (key, value) in values {
                fingerprint(key, hash)?;
                fingerprint(value, hash)?;
            }
        }
        UnpackedValue::Object { name, fields } => {
            bytes(13, name.as_bytes());
            hash.update((fields.len() as u64).to_be_bytes());
            for (name, value) in fields {
                hash.update((name.len() as u64).to_be_bytes());
                hash.update(name.as_bytes());
                fingerprint(value, hash)?;
            }
        }
    }
    Ok(())
}
