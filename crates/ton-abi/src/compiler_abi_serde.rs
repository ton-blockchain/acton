use crate::abi_serde::{Data, DataField, DataObject};
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use std::collections::BTreeMap;
use tolkc::abi::{
    ABICustomPackUnpack, ABIDeclaration, ABIEnumMember, ABIType, ABIUnionVariant, ContractABI,
    OneOrMany,
};
use tycho_types::cell::{Cell, CellBuilder, CellSlice, Load};
use tycho_types::dict;
use tycho_types::models::{AnyAddr, IntAddr, StdAddr};

pub fn decode(data: &mut CellSlice<'_>, abi: &ContractABI, ty: &ABIType) -> anyhow::Result<Data> {
    decode_type(data, abi, ty, &BTreeMap::new())
}

fn decode_type(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    ty: &ABIType,
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Data> {
    let ty = instantiate_type(ty, bindings)?;

    match ty {
        ABIType::Int => unsupported_type("int"),
        ABIType::Bool => Ok(Data::Bool(data.load_bit()?)),
        ABIType::Cell => Ok(Data::Cell(data.load_reference_cloned()?)),
        ABIType::Slice => unsupported_type("slice"),
        ABIType::Builder => unsupported_type("builder"),
        ABIType::Callable => unsupported_type("callable"),
        ABIType::String => unsupported_type("string"),
        ABIType::Coins => Ok(Data::Number(data.load_var_bigint(4, false)?)),
        ABIType::Void => unsupported_type("void"),
        ABIType::Address => Ok(Data::Address(IntAddr::load_from(data)?)),
        ABIType::AddressAny => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => Data::Null,
            AnyAddr::Ext(ext_addr) => Data::ExtAddress(ext_addr),
            AnyAddr::Std(addr) => Data::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => Data::Address(IntAddr::Var(addr)),
        }),
        ABIType::AddressOpt => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => Data::Null,
            AnyAddr::Std(addr) => Data::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => Data::Address(IntAddr::Var(addr)),
            AnyAddr::Ext(_) => anyhow::bail!("expected internal address or null for addressOpt"),
        }),
        ABIType::UintN { n } => Ok(Data::Number(data.load_bigint(n as u16, false)?)),
        ABIType::IntN { n } => Ok(Data::Number(data.load_bigint(n as u16, true)?)),
        ABIType::VarUintN { n } => {
            let len_bits = varint_len_bits(n)?;
            Ok(Data::Number(data.load_var_bigint(len_bits, false)?))
        }
        ABIType::VarIntN { n } => {
            let len_bits = varint_len_bits(n)?;
            Ok(Data::Number(data.load_var_bigint(len_bits, true)?))
        }
        ABIType::BitsN { n } => Ok(Data::Bits(load_bits(data, n)?)),
        ABIType::ArrayOf { .. } => unsupported_type("arrayOf"),
        ABIType::Tensor { items } | ABIType::ShapedTuple { items } => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(decode_type(data, abi, &item, bindings)?);
            }
            Ok(Data::Array(values))
        }
        ABIType::NullLiteral => Ok(Data::Null),
        ABIType::GenericT { name_t } => anyhow::bail!("unresolved generic type {name_t}"),
        ABIType::StructRef {
            struct_name,
            type_args,
        } => decode_struct(data, abi, &struct_name, &type_args),
        ABIType::EnumRef { enum_name } => decode_enum(data, abi, &enum_name),
        ABIType::AliasRef {
            alias_name,
            type_args,
        } => decode_alias(data, abi, &alias_name, &type_args),
        ABIType::Remaining => Ok(Data::RemainingBitsAndRefs(remaining_as_cell(data)?)),
        ABIType::CellOf { inner } => {
            let mut ref_slice = data.load_reference_as_slice()?;
            let value = decode_one_or_many(&mut ref_slice, abi, &inner, bindings)?;
            ensure_fully_consumed(&ref_slice, "Cell<T> payload")?;
            Ok(Data::Object(DataObject {
                name: "Cell".to_owned(),
                fields: vec![DataField {
                    name: "ref".to_owned(),
                    value,
                }],
            }))
        }
        ABIType::LispListOf { .. } => unsupported_type("lispListOf"),
        ABIType::Union { variants } => decode_union(data, abi, &variants, bindings),
        ABIType::Nullable { inner } => {
            if !data.load_bit()? {
                return Ok(Data::Null);
            }
            decode_type(data, abi, &inner, bindings)
        }
        ABIType::MapKV { k, v } => decode_map(data, abi, &k, &v, bindings),
        ABIType::Unknown => unsupported_type("unknown"),
    }
}

fn decode_struct(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    struct_name: &str,
    type_args: &[ABIType],
) -> anyhow::Result<Data> {
    let (type_params, prefix, fields, custom_pack_unpack) = find_struct_decl(abi, struct_name)
        .ok_or_else(|| anyhow!("struct {struct_name} referenced by ABI was not found"))?;
    ensure_standard_layout(struct_name, custom_pack_unpack)?;

    if let Some(prefix) = prefix {
        check_prefix(data, &prefix.prefix_str, prefix.prefix_len, struct_name)?;
    }

    let bindings = bind_type_params(struct_name, type_params, type_args)?;
    let mut result = DataObject {
        name: struct_name.to_owned(),
        fields: Vec::with_capacity(fields.len()),
    };
    for field in fields {
        let value = decode_type(data, abi, &field.ty, &bindings)
            .with_context(|| format!("failed to decode field {struct_name}.{}", field.name))?;
        result.fields.push(DataField {
            name: field.name.clone(),
            value,
        });
    }

    Ok(Data::Object(result))
}

fn decode_alias(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    alias_name: &str,
    type_args: &[ABIType],
) -> anyhow::Result<Data> {
    let (type_params, target_ty, custom_pack_unpack) = find_alias_decl(abi, alias_name)
        .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
    ensure_standard_layout(alias_name, custom_pack_unpack)?;
    let bindings = bind_type_params(alias_name, type_params, type_args)?;
    decode_type(data, abi, target_ty, &bindings)
}

fn decode_enum(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    enum_name: &str,
) -> anyhow::Result<Data> {
    let (encoded_as, members, custom_pack_unpack) = find_enum_decl(abi, enum_name)
        .ok_or_else(|| anyhow!("enum {enum_name} referenced by ABI was not found"))?;
    ensure_standard_layout(enum_name, custom_pack_unpack)?;

    let encoded_ty = parse_enum_encoded_as(encoded_as)?;
    let value = decode_type(data, abi, &encoded_ty, &BTreeMap::new())?;

    if let Data::Number(number) = &value
        && let Some(member_name) = find_enum_member_name(number, members)
    {
        return Ok(Data::Symbol(format!("{enum_name}.{member_name}")));
    }

    Ok(Data::Object(DataObject {
        name: enum_name.to_owned(),
        fields: vec![DataField {
            name: "value".to_owned(),
            value,
        }],
    }))
}

fn decode_one_or_many(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    inner: &OneOrMany<ABIType>,
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Data> {
    match inner {
        OneOrMany::One(inner) => decode_type(data, abi, inner, bindings),
        OneOrMany::Many(items) => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(decode_type(data, abi, item, bindings)?);
            }
            Ok(Data::Array(values))
        }
    }
}

fn decode_union(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    variants: &[ABIUnionVariant],
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Data> {
    let variants = resolve_union_variants(abi, variants, bindings)?;
    for variant in variants {
        if !matches_prefix(data, &variant.prefix_str, variant.prefix_len)? {
            continue;
        }

        if variant.prefix_eat_in_place {
            data.skip_first(variant.prefix_len as u16, 0)?;
        }

        let value = decode_type(data, abi, &variant.variant_ty, &BTreeMap::new())?;
        if !variant.has_value_field {
            return Ok(value);
        }

        return Ok(Data::Object(DataObject {
            name: variant.label,
            fields: vec![DataField {
                name: "value".to_owned(),
                value,
            }],
        }));
    }

    anyhow::bail!("none of union prefixes matched")
}

fn decode_map(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    key_ty: &ABIType,
    value_ty: &ABIType,
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Data> {
    let key_ty = instantiate_type(key_ty, bindings)?;
    let value_ty = instantiate_type(value_ty, bindings)?;
    let key_bits = map_key_bit_len(abi, &key_ty)?;
    let dict = Option::<Cell>::load_from(data)?;

    let mut entries = Vec::new();
    for entry in dict::RawIter::new(&dict, key_bits) {
        let (key_data, mut value_slice) = entry?;
        let key = decode_type(
            &mut key_data.as_data_slice(),
            abi,
            &key_ty,
            &BTreeMap::new(),
        )
        .context("failed to decode map key")?;
        let value = decode_type(&mut value_slice, abi, &value_ty, &BTreeMap::new())
            .context("failed to decode map value")?;
        ensure_fully_consumed(&value_slice, "map value")?;
        entries.push((key, value));
    }

    Ok(Data::Map(entries))
}

fn instantiate_type(ty: &ABIType, bindings: &BTreeMap<String, ABIType>) -> anyhow::Result<ABIType> {
    match ty {
        ABIType::GenericT { name_t } => bindings
            .get(name_t)
            .cloned()
            .ok_or_else(|| anyhow!("missing ABI type argument for generic {name_t}")),
        ABIType::ArrayOf { inner } => Ok(ABIType::ArrayOf {
            inner: Box::new(instantiate_type(inner, bindings)?),
        }),
        ABIType::Tensor { items } => Ok(ABIType::Tensor {
            items: instantiate_items(items, bindings)?,
        }),
        ABIType::ShapedTuple { items } => Ok(ABIType::ShapedTuple {
            items: instantiate_items(items, bindings)?,
        }),
        ABIType::StructRef {
            struct_name,
            type_args,
        } => Ok(ABIType::StructRef {
            struct_name: struct_name.clone(),
            type_args: instantiate_items(type_args, bindings)?,
        }),
        ABIType::AliasRef {
            alias_name,
            type_args,
        } => Ok(ABIType::AliasRef {
            alias_name: alias_name.clone(),
            type_args: instantiate_items(type_args, bindings)?,
        }),
        ABIType::CellOf { inner } => Ok(ABIType::CellOf {
            inner: instantiate_one_or_many(inner, bindings)?,
        }),
        ABIType::LispListOf { inner } => Ok(ABIType::LispListOf {
            inner: instantiate_one_or_many(inner, bindings)?,
        }),
        ABIType::Union { variants } => Ok(ABIType::Union {
            variants: variants
                .iter()
                .map(|variant| {
                    Ok(ABIUnionVariant {
                        variant_ty: instantiate_type(&variant.variant_ty, bindings)?,
                        prefix_str: variant.prefix_str.clone(),
                        prefix_len: variant.prefix_len,
                        is_prefix_implicit: variant.is_prefix_implicit,
                        stack_type_id: variant.stack_type_id,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        }),
        ABIType::Nullable { inner } => Ok(ABIType::Nullable {
            inner: Box::new(instantiate_type(inner, bindings)?),
        }),
        ABIType::MapKV { k, v } => Ok(ABIType::MapKV {
            k: Box::new(instantiate_type(k, bindings)?),
            v: Box::new(instantiate_type(v, bindings)?),
        }),
        other => Ok(other.clone()),
    }
}

fn instantiate_items(
    items: &[ABIType],
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Vec<ABIType>> {
    items
        .iter()
        .map(|item| instantiate_type(item, bindings))
        .collect()
}

fn instantiate_one_or_many(
    inner: &OneOrMany<ABIType>,
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<OneOrMany<ABIType>> {
    Ok(match inner {
        OneOrMany::One(inner) => OneOrMany::One(Box::new(instantiate_type(inner, bindings)?)),
        OneOrMany::Many(items) => OneOrMany::Many(instantiate_items(items, bindings)?),
    })
}

fn bind_type_params(
    type_name: &str,
    type_params: Option<&[String]>,
    type_args: &[ABIType],
) -> anyhow::Result<BTreeMap<String, ABIType>> {
    let Some(type_params) = type_params else {
        if type_args.is_empty() {
            return Ok(BTreeMap::new());
        }
        anyhow::bail!("{type_name} does not accept type arguments");
    };

    if type_params.len() != type_args.len() {
        anyhow::bail!(
            "{type_name} expected {} type arguments, got {}",
            type_params.len(),
            type_args.len()
        );
    }

    Ok(type_params
        .iter()
        .cloned()
        .zip(type_args.iter().cloned())
        .collect())
}

fn ensure_standard_layout(
    type_name: &str,
    custom_pack_unpack: Option<&ABICustomPackUnpack>,
) -> anyhow::Result<()> {
    if custom_pack_unpack.is_some() {
        anyhow::bail!("cannot decode {type_name} because it uses custom pack/unpack");
    }
    Ok(())
}

fn check_prefix(
    data: &mut CellSlice<'_>,
    prefix_str: &str,
    prefix_len: i32,
    type_name: &str,
) -> anyhow::Result<()> {
    let prefix_len = u16::try_from(prefix_len).context("negative prefix length")?;
    let expected = parse_prefix(prefix_str)?;
    let actual = data.load_uint(prefix_len)?;
    if actual != expected {
        anyhow::bail!(
            "incorrect prefix for '{type_name}': expected {prefix_str}, got 0x{actual:x}"
        );
    }
    Ok(())
}

fn matches_prefix(data: &CellSlice<'_>, prefix_str: &str, prefix_len: i32) -> anyhow::Result<bool> {
    let prefix_len = u16::try_from(prefix_len).context("negative prefix length")?;
    if !data.has_remaining(prefix_len, 0) {
        return Ok(false);
    }
    Ok(data.get_uint(0, prefix_len)? == parse_prefix(prefix_str)?)
}

fn parse_prefix(prefix_str: &str) -> anyhow::Result<u64> {
    if let Some(hex) = prefix_str.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16)
            .with_context(|| format!("failed to parse hex prefix {prefix_str}"));
    }
    if let Some(bits) = prefix_str.strip_prefix("0b") {
        return u64::from_str_radix(bits, 2)
            .with_context(|| format!("failed to parse binary prefix {prefix_str}"));
    }
    prefix_str
        .parse::<u64>()
        .with_context(|| format!("failed to parse decimal prefix {prefix_str}"))
}

fn varint_len_bits(n: usize) -> anyhow::Result<u16> {
    if !n.is_power_of_two() {
        anyhow::bail!("invalid variadic integer size {n}");
    }
    Ok(n.ilog2() as u16)
}

fn load_bits(data: &mut CellSlice<'_>, bits: usize) -> anyhow::Result<(Vec<u8>, usize)> {
    let bits = data.load_prefix(bits as u16, 0)?;
    let bytes = bits.size_bits().div_ceil(8) as usize;
    let mut raw = vec![0; bytes];
    bits.get_raw(0, &mut raw, bits.size_bits())?;
    Ok((raw, bits.size_bits() as usize))
}

fn remaining_as_cell(data: &mut CellSlice<'_>) -> anyhow::Result<Cell> {
    let mut builder = CellBuilder::new();
    builder.store_slice(data.load_remaining())?;
    Ok(builder.build()?)
}

fn ensure_fully_consumed(data: &CellSlice<'_>, what: &str) -> anyhow::Result<()> {
    if data.size_bits() == 0 && data.size_refs() == 0 {
        return Ok(());
    }
    anyhow::bail!("{what} was not fully consumed")
}

fn unsupported_type(name: &str) -> anyhow::Result<Data> {
    anyhow::bail!("cannot decode unsupported ABI type {name}")
}

fn parse_enum_encoded_as(encoded_as: &str) -> anyhow::Result<ABIType> {
    if encoded_as == "bool" {
        return Ok(ABIType::Bool);
    }
    if encoded_as == "coins" {
        return Ok(ABIType::Coins);
    }
    if let Some(bits) = encoded_as.strip_prefix("uint") {
        return Ok(ABIType::UintN {
            n: bits.parse().context("invalid enum uint width")?,
        });
    }
    if let Some(bits) = encoded_as.strip_prefix("int") {
        return Ok(ABIType::IntN {
            n: bits.parse().context("invalid enum int width")?,
        });
    }
    if let Some(bits) = encoded_as.strip_prefix("varuint") {
        return Ok(ABIType::VarUintN {
            n: bits.parse().context("invalid enum varuint width")?,
        });
    }
    if let Some(bits) = encoded_as.strip_prefix("varint") {
        return Ok(ABIType::VarIntN {
            n: bits.parse().context("invalid enum varint width")?,
        });
    }
    anyhow::bail!("unsupported enum encoding {encoded_as}")
}

fn find_enum_member_name<'a>(value: &BigInt, members: &'a [ABIEnumMember]) -> Option<&'a str> {
    members.iter().find_map(|member| {
        member
            .value
            .parse::<BigInt>()
            .ok()
            .filter(|member_value| member_value == value)
            .map(|_| member.name.as_str())
    })
}

fn map_key_bit_len(abi: &ContractABI, ty: &ABIType) -> anyhow::Result<u16> {
    match ty {
        ABIType::Bool => Ok(1),
        ABIType::IntN { n } | ABIType::UintN { n } => {
            u16::try_from(*n).context("map key width exceeds u16")
        }
        ABIType::Address => Ok(StdAddr::BITS_WITHOUT_ANYCAST),
        ABIType::AliasRef { alias_name, .. } => {
            let (_, target_ty, custom_pack_unpack) = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, custom_pack_unpack)?;
            map_key_bit_len(abi, target_ty)
        }
        ABIType::EnumRef { enum_name } => {
            let (encoded_as, _, custom_pack_unpack) = find_enum_decl(abi, enum_name)
                .ok_or_else(|| anyhow!("enum {enum_name} referenced by ABI was not found"))?;
            ensure_standard_layout(enum_name, custom_pack_unpack)?;
            map_key_bit_len(abi, &parse_enum_encoded_as(encoded_as)?)
        }
        _ => anyhow::bail!("unsupported map key type {}", ty.render_type()),
    }
}

#[derive(Clone)]
struct ResolvedUnionVariant {
    variant_ty: ABIType,
    prefix_str: String,
    prefix_len: i32,
    prefix_eat_in_place: bool,
    label: String,
    has_value_field: bool,
}

fn resolve_union_variants(
    abi: &ContractABI,
    variants: &[ABIUnionVariant],
    bindings: &BTreeMap<String, ABIType>,
) -> anyhow::Result<Vec<ResolvedUnionVariant>> {
    let mut simple_labels = Vec::with_capacity(variants.len());
    let mut concrete_variants = Vec::with_capacity(variants.len());
    for variant in variants {
        let concrete = instantiate_type(&variant.variant_ty, bindings)?;
        simple_labels.push(union_label_simple(abi, &concrete)?);
        concrete_variants.push(concrete);
    }

    let has_duplicates = simple_labels
        .iter()
        .enumerate()
        .any(|(idx, label)| simple_labels[..idx].contains(label));

    variants
        .iter()
        .zip(concrete_variants)
        .zip(simple_labels)
        .map(|((variant, concrete), simple_label)| {
            let is_null = matches!(concrete, ABIType::NullLiteral);
            Ok(ResolvedUnionVariant {
                variant_ty: concrete.clone(),
                prefix_str: variant.prefix_str.clone(),
                prefix_len: variant.prefix_len,
                prefix_eat_in_place: variant.is_prefix_implicit.unwrap_or(false),
                label: if is_null {
                    String::new()
                } else if has_duplicates {
                    concrete.render_type()
                } else {
                    simple_label
                },
                has_value_field: !is_null
                    && (has_duplicates || !type_has_own_label(abi, &concrete)?),
            })
        })
        .collect()
}

fn union_label_simple(abi: &ContractABI, ty: &ABIType) -> anyhow::Result<String> {
    Ok(match ty {
        ABIType::Int => "int".to_owned(),
        ABIType::IntN { n } => format!("int{n}"),
        ABIType::UintN { n } => format!("uint{n}"),
        ABIType::VarIntN { n } => format!("varint{n}"),
        ABIType::VarUintN { n } => format!("varuint{n}"),
        ABIType::Coins => "coins".to_owned(),
        ABIType::Bool => "bool".to_owned(),
        ABIType::Cell => "cell".to_owned(),
        ABIType::Builder => "builder".to_owned(),
        ABIType::Slice => "slice".to_owned(),
        ABIType::Remaining => "RemainingBitsAndRefs".to_owned(),
        ABIType::Address => "address".to_owned(),
        ABIType::AddressOpt => "address?".to_owned(),
        ABIType::AddressAny => "any_address".to_owned(),
        ABIType::BitsN { n } => format!("bits{n}"),
        ABIType::NullLiteral => "null".to_owned(),
        ABIType::Callable => "callable".to_owned(),
        ABIType::Void => "void".to_owned(),
        ABIType::Nullable { inner } => format!("{}?", union_label_simple(abi, inner)?),
        ABIType::CellOf { .. } => "Cell".to_owned(),
        ABIType::Tensor { .. } | ABIType::ShapedTuple { .. } => "tensor".to_owned(),
        ABIType::MapKV { .. } => "map".to_owned(),
        ABIType::EnumRef { enum_name } => enum_name.clone(),
        ABIType::StructRef { struct_name, .. } => struct_name.clone(),
        ABIType::AliasRef { alias_name, .. } => {
            let (_, target_ty, custom_pack_unpack) = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, custom_pack_unpack)?;
            union_label_simple(abi, target_ty)?
        }
        ABIType::GenericT { name_t } => name_t.clone(),
        ABIType::Union { variants } => variants
            .iter()
            .map(|variant| union_label_simple(abi, &variant.variant_ty))
            .collect::<anyhow::Result<Vec<_>>>()?
            .join("|"),
        ABIType::ArrayOf { .. } => "array".to_owned(),
        ABIType::LispListOf { .. } => "lisp_list".to_owned(),
        ABIType::Unknown => "unknown".to_owned(),
        ABIType::String => "string".to_owned(),
    })
}

fn type_has_own_label(abi: &ContractABI, ty: &ABIType) -> anyhow::Result<bool> {
    Ok(match ty {
        ABIType::StructRef { .. } => true,
        ABIType::AliasRef { alias_name, .. } => {
            let (_, target_ty, custom_pack_unpack) = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, custom_pack_unpack)?;
            type_has_own_label(abi, target_ty)?
        }
        _ => false,
    })
}

type StructDeclRef<'a> = (
    Option<&'a [String]>,
    Option<&'a tolkc::abi::ABIOpcode>,
    &'a [tolkc::abi::ABIStructField],
    Option<&'a ABICustomPackUnpack>,
);

fn find_struct_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<StructDeclRef<'a>> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Struct {
            name,
            type_params,
            prefix,
            fields,
            custom_pack_unpack,
        } if name == target_name => Some((
            type_params.as_deref(),
            prefix.as_ref(),
            fields.as_slice(),
            custom_pack_unpack.as_ref(),
        )),
        _ => None,
    })
}

fn find_alias_decl<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(
    Option<&'a [String]>,
    &'a ABIType,
    Option<&'a ABICustomPackUnpack>,
)> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name,
            target_ty,
            type_params,
            custom_pack_unpack,
        } if name == target_name => Some((
            type_params.as_deref(),
            target_ty,
            custom_pack_unpack.as_ref(),
        )),
        _ => None,
    })
}

fn find_enum_decl<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(
    &'a str,
    &'a [ABIEnumMember],
    Option<&'a ABICustomPackUnpack>,
)> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Enum {
            name,
            encoded_as,
            members,
            custom_pack_unpack,
        } if name == target_name => Some((
            encoded_as.as_str(),
            members.as_slice(),
            custom_pack_unpack.as_ref(),
        )),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolkc::abi::{
        ABIInternalMessage, ABIOpcode, ABIStorage, ABIStructField, ABIUnionVariant, ContractABI,
    };
    use tycho_types::cell::{CellBuilder, CellFamily, Store};
    use tycho_types::models::{AnyAddr, ExtAddr, StdAddr};

    fn empty_abi() -> ContractABI {
        ContractABI {
            abi_schema_version: "1".to_owned(),
            contract_name: "Test".to_owned(),
            author: String::new(),
            version: String::new(),
            description: String::new(),
            declarations: Vec::new(),
            incoming_messages: Vec::new(),
            incoming_external: Vec::new(),
            outgoing_messages: Vec::new(),
            emitted_events: Vec::new(),
            storage: ABIStorage {
                storage_ty: None,
                storage_at_deployment_ty: None,
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
            constants: Vec::new(),
            compiler_name: "tolk".to_owned(),
            compiler_version: "0".to_owned(),
        }
    }

    #[test]
    fn decodes_struct_with_prefix_and_fields() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "MyMessage".to_owned(),
            type_params: None,
            prefix: Some(ABIOpcode {
                prefix_str: "0x12345678".to_owned(),
                prefix_len: 32,
            }),
            fields: vec![
                ABIStructField {
                    name: "queryId".to_owned(),
                    ty: ABIType::UintN { n: 64 },
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "flag".to_owned(),
                    ty: ABIType::Bool,
                    default_value: None,
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
        }];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: ABIType::StructRef {
                struct_name: "MyMessage".to_owned(),
                type_args: Vec::new(),
            },
            description: String::new(),
            minimal_msg_value: None,
            preferred_send_mode: None,
        }];

        let mut builder = CellBuilder::new();
        builder.store_uint(0x12345678, 32).unwrap();
        builder.store_uint(7, 64).unwrap();
        builder.store_bit(true).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &ABIType::StructRef {
                struct_name: "MyMessage".to_owned(),
                type_args: Vec::new(),
            },
        )
        .unwrap();

        assert_eq!(
            format!("{data:?}"),
            "Object(DataObject { name: \"MyMessage\", fields: [DataField { name: \"queryId\", value: Number(7) }, DataField { name: \"flag\", value: Bool(true) }] })"
        );
        assert_eq!(slice.size_bits(), 0);
    }

    #[test]
    fn decodes_address_any_and_map() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "Payload".to_owned(),
            type_params: None,
            prefix: None,
            fields: vec![
                ABIStructField {
                    name: "owner".to_owned(),
                    ty: ABIType::AddressAny,
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "items".to_owned(),
                    ty: ABIType::MapKV {
                        k: Box::new(ABIType::UintN { n: 8 }),
                        v: Box::new(ABIType::Bool),
                    },
                    default_value: None,
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
        }];

        let owner = ExtAddr::new(8, vec![0xaa]).unwrap();
        let mut map = dict::Dict::<u8, bool>::new();
        map.set(1, true).unwrap();
        map.set(2, false).unwrap();

        let mut builder = CellBuilder::new();
        AnyAddr::Ext(owner)
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        map.store_into(&mut builder, Cell::empty_context()).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &ABIType::StructRef {
                struct_name: "Payload".to_owned(),
                type_args: Vec::new(),
            },
        )
        .unwrap();

        assert!(matches!(data, Data::Object(_)));
    }

    #[test]
    fn decodes_auto_prefixed_union() {
        let abi = empty_abi();
        let mut builder = CellBuilder::new();
        builder.store_bit(true).unwrap();
        builder.store_uint(99, 16).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &ABIType::Union {
                variants: vec![
                    ABIUnionVariant {
                        variant_ty: ABIType::UintN { n: 8 },
                        prefix_str: "0".to_owned(),
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                    },
                    ABIUnionVariant {
                        variant_ty: ABIType::UintN { n: 16 },
                        prefix_str: "1".to_owned(),
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                    },
                ],
            },
        )
        .unwrap();

        assert_eq!(
            format!("{data:?}"),
            "Object(DataObject { name: \"uint16\", fields: [DataField { name: \"value\", value: Number(99) }] })"
        );
    }

    #[test]
    fn decodes_enum_to_symbol() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Enum {
            name: "Color".to_owned(),
            encoded_as: "uint8".to_owned(),
            members: vec![
                ABIEnumMember {
                    name: "Red".to_owned(),
                    value: "1".to_owned(),
                    description: String::new(),
                },
                ABIEnumMember {
                    name: "Blue".to_owned(),
                    value: "2".to_owned(),
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
        }];

        let mut builder = CellBuilder::new();
        builder.store_uint(2, 8).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &ABIType::EnumRef {
                enum_name: "Color".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(format!("{data:?}"), "Symbol(\"Color.Blue\")");
    }

    #[test]
    fn decodes_address_opt_none() {
        let abi = empty_abi();
        let mut builder = CellBuilder::new();
        builder.store_uint(0, 2).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(&mut slice, &abi, &ABIType::AddressOpt).unwrap();
        assert!(matches!(data, Data::Null));
    }

    #[test]
    fn decodes_internal_address_any() {
        let abi = empty_abi();
        let mut builder = CellBuilder::new();
        StdAddr::new(0, Default::default())
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(&mut slice, &abi, &ABIType::AddressAny).unwrap();
        assert!(matches!(data, Data::Address(_)));
    }
}
