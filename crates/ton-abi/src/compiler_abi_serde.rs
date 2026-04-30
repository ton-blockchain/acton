use crate::abi_serde::{Data, DataField, DataObject};
use crate::snake_string::parse_snake_string;
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use tolk_compiler::abi::{
    ABICustomPackUnpack, ABIDeclaration, ABIEnumMember, ContractABI, Ty, UnionVariant,
};
use tolk_compiler::types_kernel::instantiate_generics;
use tycho_types::cell::{Cell, CellBuilder, CellSlice, Load};
use tycho_types::dict;
use tycho_types::models::{AnyAddr, IntAddr, StdAddr};

pub fn decode(data: &mut CellSlice<'_>, abi: &ContractABI, ty: &Ty) -> anyhow::Result<Data> {
    decode_type(data, abi, ty)
}

fn decode_type(data: &mut CellSlice<'_>, abi: &ContractABI, ty: &Ty) -> anyhow::Result<Data> {
    match ty {
        Ty::Int => unsupported_type("int"),
        Ty::Bool => Ok(Data::Bool(data.load_bit()?)),
        Ty::Cell => Ok(Data::Cell(data.load_reference_cloned()?)),
        Ty::Slice => unsupported_type("slice"),
        Ty::Builder => unsupported_type("builder"),
        Ty::Callable => unsupported_type("callable"),
        Ty::String => {
            let cell = data.load_reference_cloned()?;
            let string =
                parse_snake_string(&cell).ok_or_else(|| anyhow!("expected snake string"))?;
            Ok(Data::String(string))
        }
        Ty::Coins => Ok(Data::Number(data.load_var_bigint(4, false)?)),
        Ty::Void => unsupported_type("void"),
        Ty::Address => Ok(Data::Address(IntAddr::load_from(data)?)),
        Ty::AddressExt => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::Ext(ext_addr) => Data::ExtAddress(ext_addr),
            _ => anyhow::bail!("expected external address for addressExt"),
        }),
        Ty::AddressAny => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => Data::Null,
            AnyAddr::Ext(ext_addr) => Data::ExtAddress(ext_addr),
            AnyAddr::Std(addr) => Data::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => Data::Address(IntAddr::Var(addr)),
        }),
        Ty::AddressOpt => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => Data::Null,
            AnyAddr::Std(addr) => Data::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => Data::Address(IntAddr::Var(addr)),
            AnyAddr::Ext(_) => anyhow::bail!("expected internal address or null for addressOpt"),
        }),
        Ty::UintN { n } => Ok(Data::Number(data.load_bigint(*n as u16, false)?)),
        Ty::IntN { n } => Ok(Data::Number(data.load_bigint(*n as u16, true)?)),
        Ty::VaruintN { n } => {
            let len_bits = varint_len_bits(*n)?;
            Ok(Data::Number(data.load_var_bigint(len_bits, false)?))
        }
        Ty::VarintN { n } => {
            let len_bits = varint_len_bits(*n)?;
            Ok(Data::Number(data.load_var_bigint(len_bits, true)?))
        }
        Ty::BitsN { n } => Ok(Data::Bits(load_bits(data, *n)?)),
        Ty::ArrayOf { .. } => unsupported_type("arrayOf"),
        Ty::Tensor { items } | Ty::ShapedTuple { items } => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(decode_type(data, abi, item)?);
            }
            Ok(Data::Array(values))
        }
        Ty::NullLiteral => Ok(Data::Null),
        Ty::GenericT { name_t } => anyhow::bail!("unresolved generic type {name_t}"),
        Ty::StructRef {
            struct_name,
            type_args,
        } => decode_struct(data, abi, struct_name, type_args.as_deref().unwrap_or(&[])),
        Ty::EnumRef { enum_name } => decode_enum(data, abi, enum_name),
        Ty::AliasRef {
            alias_name,
            type_args,
        } => decode_alias(data, abi, alias_name, type_args.as_deref().unwrap_or(&[])),
        Ty::Remaining => Ok(Data::RemainingBitsAndRefs(remaining_as_cell(data)?)),
        Ty::CellOf { inner } => {
            let mut ref_slice = data.load_reference_as_slice()?;
            let value = decode_type(&mut ref_slice, abi, inner)?;
            ensure_fully_consumed(&ref_slice, "Cell<T> payload")?;
            Ok(Data::Object(DataObject {
                name: "Cell".to_owned(),
                fields: vec![DataField {
                    name: "ref".to_owned(),
                    field_type: inner.as_ref().clone(),
                    value,
                }],
            }))
        }
        Ty::LispListOf { .. } => unsupported_type("lispListOf"),
        Ty::Union { variants, .. } => decode_union(data, abi, variants),
        Ty::Nullable { inner, .. } => {
            if !data.load_bit()? {
                return Ok(Data::Null);
            }
            decode_type(data, abi, inner)
        }
        Ty::MapKV { k, v } => decode_map(data, abi, k, v),
        Ty::Unknown => unsupported_type("unknown"),
    }
}

fn decode_struct(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    struct_name: &str,
    type_args: &[Ty],
) -> anyhow::Result<Data> {
    let decl = find_struct_decl(abi, struct_name)
        .ok_or_else(|| anyhow!("struct {struct_name} referenced by ABI was not found"))?;
    ensure_standard_layout(struct_name, decl.custom_pack_unpack)?;

    if let Some(prefix) = decl.prefix {
        check_prefix(data, &prefix.prefix_str, prefix.prefix_len, struct_name)?;
    }

    let type_params = validate_type_args(struct_name, decl.type_params, type_args)?;
    let mut result = DataObject {
        name: struct_name.to_owned(),
        fields: Vec::with_capacity(decl.fields.len()),
    };
    for field in decl.fields {
        let field_ty = instantiate_generics(&field.ty, type_params, type_args);
        let value = decode_type(data, abi, &field_ty)
            .with_context(|| format!("failed to decode field {struct_name}.{}", field.name))?;
        result.fields.push(DataField {
            name: field.name.clone(),
            field_type: field_ty,
            value,
        });
    }

    Ok(Data::Object(result))
}

fn decode_alias(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    alias_name: &str,
    type_args: &[Ty],
) -> anyhow::Result<Data> {
    let decl = find_alias_decl(abi, alias_name)
        .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
    ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
    let type_params = validate_type_args(alias_name, decl.type_params, type_args)?;
    let target_ty = instantiate_generics(decl.target_ty, type_params, type_args);
    decode_type(data, abi, &target_ty)
}

fn decode_enum(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    enum_name: &str,
) -> anyhow::Result<Data> {
    let decl = find_enum_decl(abi, enum_name)
        .ok_or_else(|| anyhow!("enum {enum_name} referenced by ABI was not found"))?;
    ensure_standard_layout(enum_name, decl.custom_pack_unpack)?;

    let encoded_ty = parse_enum_encoded_as(decl.encoded_as)?;
    let value = decode_type(data, abi, encoded_ty)?;
    let label = find_enum_member_name(&value, decl.members).map_or_else(
        || format!("{enum_name}({})", format_enum_encoded_value(&value)),
        |member_name| format!("{enum_name}.{member_name}"),
    );

    Ok(Data::Object(DataObject {
        name: label,
        fields: vec![DataField {
            name: "value".to_owned(),
            field_type: encoded_ty.clone(),
            value,
        }],
    }))
}

fn format_enum_encoded_value(value: &Data) -> String {
    match value {
        Data::Number(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        other => format!("{other:?}"),
    }
}

fn decode_union(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    variants: &[UnionVariant],
) -> anyhow::Result<Data> {
    let variants = resolve_union_variants(abi, variants)?;
    for variant in variants {
        if !matches_prefix(data, &variant.prefix_str, variant.prefix_len)? {
            continue;
        }

        if variant.prefix_eat_in_place {
            data.skip_first(
                u16::try_from(variant.prefix_len).context("union prefix length exceeds u16")?,
                0,
            )?;
        }

        let value = decode_type(data, abi, &variant.variant_ty)?;
        if !variant.has_value_field {
            return Ok(value);
        }

        return Ok(Data::Object(DataObject {
            name: variant.label,
            fields: vec![DataField {
                name: "value".to_owned(),
                field_type: variant.variant_ty.clone(),
                value,
            }],
        }));
    }

    anyhow::bail!("none of union prefixes matched")
}

fn decode_map(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    key_ty: &Ty,
    value_ty: &Ty,
) -> anyhow::Result<Data> {
    let key_bits = map_key_bit_len(abi, key_ty)?;
    let dict = Option::<Cell>::load_from(data)?;

    let mut entries = Vec::new();
    for entry in dict::RawIter::new(&dict, key_bits) {
        let (key_data, mut value_slice) = entry?;
        let key = decode_type(&mut key_data.as_data_slice(), abi, key_ty)
            .context("failed to decode map key")?;
        let value =
            decode_type(&mut value_slice, abi, value_ty).context("failed to decode map value")?;
        ensure_fully_consumed(&value_slice, "map value")?;
        entries.push((key, value));
    }

    Ok(Data::Map(entries))
}

fn validate_type_args<'a>(
    type_name: &str,
    type_params: Option<&'a [String]>,
    type_args: &[Ty],
) -> anyhow::Result<&'a [String]> {
    let Some(type_params) = type_params else {
        if type_args.is_empty() {
            return Ok(&[]);
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

    Ok(type_params)
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

fn matches_prefix(
    data: &CellSlice<'_>,
    prefix_str: &str,
    prefix_len: usize,
) -> anyhow::Result<bool> {
    let prefix_len = u16::try_from(prefix_len).context("union prefix length exceeds u16")?;
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

fn varint_len_bits(n: u32) -> anyhow::Result<u16> {
    if !n.is_power_of_two() {
        anyhow::bail!("invalid variadic integer size {n}");
    }
    Ok(n.ilog2() as u16)
}

fn load_bits(data: &mut CellSlice<'_>, bits: u32) -> anyhow::Result<(Vec<u8>, usize)> {
    let bits = data.load_prefix(u16::try_from(bits).context("bits width exceeds u16")?, 0)?;
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

fn parse_enum_encoded_as(encoded_as: &Ty) -> anyhow::Result<&Ty> {
    match encoded_as {
        Ty::Bool
        | Ty::Coins
        | Ty::UintN { .. }
        | Ty::IntN { .. }
        | Ty::VaruintN { .. }
        | Ty::VarintN { .. } => Ok(encoded_as),
        other => anyhow::bail!("unsupported enum encoding {}", other.render_type()),
    }
}

fn find_enum_member_name<'a>(value: &Data, members: &'a [ABIEnumMember]) -> Option<&'a str> {
    members.iter().find_map(|member| match value {
        Data::Number(number) => member
            .value
            .parse::<BigInt>()
            .ok()
            .filter(|member_value| member_value == number)
            .map(|_| member.name.as_str()),
        Data::Bool(boolean) => match member.value.as_str() {
            "false" if !boolean => Some(member.name.as_str()),
            "true" if *boolean => Some(member.name.as_str()),
            _ => None,
        },
        _ => None,
    })
}

fn map_key_bit_len(abi: &ContractABI, ty: &Ty) -> anyhow::Result<u16> {
    match ty {
        Ty::Bool => Ok(1),
        Ty::IntN { n } | Ty::UintN { n } => u16::try_from(*n).context("map key width exceeds u16"),
        Ty::Address => Ok(StdAddr::BITS_WITHOUT_ANYCAST),
        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            let decl = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let type_args = type_args.as_deref().unwrap_or(&[]);
            let type_params = validate_type_args(alias_name, decl.type_params, type_args)?;
            let target_ty = instantiate_generics(decl.target_ty, type_params, type_args);
            map_key_bit_len(abi, &target_ty)
        }
        Ty::EnumRef { enum_name } => {
            let decl = find_enum_decl(abi, enum_name)
                .ok_or_else(|| anyhow!("enum {enum_name} referenced by ABI was not found"))?;
            ensure_standard_layout(enum_name, decl.custom_pack_unpack)?;
            map_key_bit_len(abi, parse_enum_encoded_as(decl.encoded_as)?)
        }
        _ => anyhow::bail!("unsupported map key type {}", ty.render_type()),
    }
}

#[derive(Clone)]
struct ResolvedUnionVariant {
    variant_ty: Ty,
    prefix_str: String,
    prefix_len: usize,
    prefix_eat_in_place: bool,
    label: String,
    has_value_field: bool,
}

fn resolve_union_variants(
    abi: &ContractABI,
    variants: &[UnionVariant],
) -> anyhow::Result<Vec<ResolvedUnionVariant>> {
    let mut simple_labels = Vec::with_capacity(variants.len());
    let mut concrete_variants = Vec::with_capacity(variants.len());
    for variant in variants {
        let concrete = variant.variant_ty.clone();
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
            let is_null = matches!(concrete, Ty::NullLiteral);
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

fn union_label_simple(abi: &ContractABI, ty: &Ty) -> anyhow::Result<String> {
    Ok(match ty {
        Ty::Int => "int".to_owned(),
        Ty::IntN { n } => format!("int{n}"),
        Ty::UintN { n } => format!("uint{n}"),
        Ty::VarintN { n } => format!("varint{n}"),
        Ty::VaruintN { n } => format!("varuint{n}"),
        Ty::Coins => "coins".to_owned(),
        Ty::Bool => "bool".to_owned(),
        Ty::Cell => "cell".to_owned(),
        Ty::Builder => "builder".to_owned(),
        Ty::Slice => "slice".to_owned(),
        Ty::Remaining => "RemainingBitsAndRefs".to_owned(),
        Ty::Address => "address".to_owned(),
        Ty::AddressOpt => "address?".to_owned(),
        Ty::AddressExt => "ext_address".to_owned(),
        Ty::AddressAny => "any_address".to_owned(),
        Ty::BitsN { n } => format!("bits{n}"),
        Ty::NullLiteral => "null".to_owned(),
        Ty::Callable => "callable".to_owned(),
        Ty::Void => "void".to_owned(),
        Ty::Nullable { inner, .. } => format!("{}?", union_label_simple(abi, inner)?),
        Ty::CellOf { .. } => "Cell".to_owned(),
        Ty::Tensor { .. } | Ty::ShapedTuple { .. } => "tensor".to_owned(),
        Ty::MapKV { .. } => "map".to_owned(),
        Ty::EnumRef { enum_name } => enum_name.clone(),
        Ty::StructRef { struct_name, .. } => struct_name.clone(),
        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            let decl = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let type_args = type_args.as_deref().unwrap_or(&[]);
            let type_params = validate_type_args(alias_name, decl.type_params, type_args)?;
            let target_ty = instantiate_generics(decl.target_ty, type_params, type_args);
            union_label_simple(abi, &target_ty)?
        }
        Ty::GenericT { name_t } => name_t.clone(),
        Ty::Union { variants, .. } => variants
            .iter()
            .map(|variant| union_label_simple(abi, &variant.variant_ty))
            .collect::<anyhow::Result<Vec<_>>>()?
            .join("|"),
        Ty::ArrayOf { .. } => "array".to_owned(),
        Ty::LispListOf { .. } => "lisp_list".to_owned(),
        Ty::Unknown => "unknown".to_owned(),
        Ty::String => "string".to_owned(),
    })
}

fn type_has_own_label(abi: &ContractABI, ty: &Ty) -> anyhow::Result<bool> {
    Ok(match ty {
        Ty::StructRef { .. } => true,
        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            let decl = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by ABI was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let type_args = type_args.as_deref().unwrap_or(&[]);
            let type_params = validate_type_args(alias_name, decl.type_params, type_args)?;
            let target_ty = instantiate_generics(decl.target_ty, type_params, type_args);
            type_has_own_label(abi, &target_ty)?
        }
        _ => false,
    })
}

struct StructDeclRef<'a> {
    type_params: Option<&'a [String]>,
    prefix: Option<&'a tolk_compiler::abi::ABIOpcode>,
    fields: &'a [tolk_compiler::abi::ABIStructField],
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

struct AliasDeclRef<'a> {
    type_params: Option<&'a [String]>,
    target_ty: &'a Ty,
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

struct EnumDeclRef<'a> {
    encoded_as: &'a Ty,
    members: &'a [ABIEnumMember],
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

fn find_struct_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<StructDeclRef<'a>> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Struct {
            name,
            type_params,
            prefix,
            fields,
            custom_pack_unpack,
        } if name == target_name => Some(StructDeclRef {
            type_params: type_params.as_deref(),
            prefix: prefix.as_ref(),
            fields: fields.as_slice(),
            custom_pack_unpack: custom_pack_unpack.as_ref(),
        }),
        _ => None,
    })
}

fn find_alias_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<AliasDeclRef<'a>> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name,
            target_ty,
            type_params,
            custom_pack_unpack,
        } if name == target_name => Some(AliasDeclRef {
            type_params: type_params.as_deref(),
            target_ty,
            custom_pack_unpack: custom_pack_unpack.as_ref(),
        }),
        _ => None,
    })
}

fn find_enum_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<EnumDeclRef<'a>> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Enum {
            name,
            encoded_as,
            members,
            custom_pack_unpack,
        } if name == target_name => Some(EnumDeclRef {
            encoded_as,
            members: members.as_slice(),
            custom_pack_unpack: custom_pack_unpack.as_ref(),
        }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snake_string::build_snake_bytes_cell;
    use tolk_compiler::abi::{
        ABIInternalMessage, ABIOpcode, ABIStorage, ABIStructField, ContractABI, UnionVariant,
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
                description: String::new(),
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
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
                    ty: Ty::UintN { n: 64 },
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "flag".to_owned(),
                    ty: Ty::Bool,
                    default_value: None,
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
        }];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: Ty::StructRef {
                struct_name: "MyMessage".to_owned(),
                type_args: None,
            },
            description: String::new(),
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
            &Ty::StructRef {
                struct_name: "MyMessage".to_owned(),
                type_args: None,
            },
        )
        .unwrap();

        let Data::Object(object) = data else {
            panic!("expected object");
        };
        assert_eq!(object.name, "MyMessage");
        assert_eq!(object.fields.len(), 2);
        assert_eq!(object.fields[0].name, "queryId");
        assert!(matches!(object.fields[0].field_type, Ty::UintN { n: 64 }));
        assert!(matches!(object.fields[0].value, Data::Number(_)));
        assert_eq!(object.fields[1].name, "flag");
        assert!(matches!(object.fields[1].field_type, Ty::Bool));
        assert!(matches!(object.fields[1].value, Data::Bool(true)));
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
                    ty: Ty::AddressAny,
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "items".to_owned(),
                    ty: Ty::MapKV {
                        k: Box::new(Ty::UintN { n: 8 }),
                        v: Box::new(Ty::Bool),
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
            &Ty::StructRef {
                struct_name: "Payload".to_owned(),
                type_args: None,
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
            &Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty: Ty::UintN { n: 8 },
                        prefix_str: "0".to_owned(),
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty: Ty::UintN { n: 16 },
                        prefix_str: "1".to_owned(),
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            },
        )
        .unwrap();

        let Data::Object(object) = data else {
            panic!("expected object");
        };
        assert_eq!(object.name, "uint16");
        assert_eq!(object.fields.len(), 1);
        assert_eq!(object.fields[0].name, "value");
        assert!(matches!(object.fields[0].field_type, Ty::UintN { n: 16 }));
        assert!(matches!(object.fields[0].value, Data::Number(_)));
    }

    #[test]
    fn decodes_generic_struct_fields_with_instantiated_types() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "Boxed".to_owned(),
            type_params: Some(vec!["T".to_owned()]),
            prefix: None,
            fields: vec![ABIStructField {
                name: "value".to_owned(),
                ty: Ty::GenericT {
                    name_t: "T".to_owned(),
                },
                default_value: None,
                description: String::new(),
            }],
            custom_pack_unpack: None,
        }];

        let mut builder = CellBuilder::new();
        builder.store_uint(7, 32).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args: Some(vec![Ty::UintN { n: 32 }]),
            },
        )
        .unwrap();

        let Data::Object(object) = data else {
            panic!("expected object");
        };
        assert_eq!(object.name, "Boxed");
        assert_eq!(object.fields.len(), 1);
        assert_eq!(object.fields[0].name, "value");
        assert!(matches!(object.fields[0].field_type, Ty::UintN { n: 32 }));
        assert!(matches!(object.fields[0].value, Data::Number(_)));
    }

    #[test]
    fn decodes_enum_to_object_with_raw_value() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Enum {
            name: "Color".to_owned(),
            encoded_as: Ty::UintN { n: 8 },
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
            &Ty::EnumRef {
                enum_name: "Color".to_owned(),
            },
        )
        .unwrap();

        let Data::Object(object) = data else {
            panic!("expected object");
        };
        assert_eq!(object.name, "Color.Blue");
        assert_eq!(object.fields.len(), 1);
        assert_eq!(object.fields[0].name, "value");
        assert!(matches!(object.fields[0].field_type, Ty::UintN { n: 8 }));
        assert!(matches!(object.fields[0].value, Data::Number(_)));
    }

    #[test]
    fn decodes_bool_encoded_enum_to_object_with_raw_value() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Enum {
            name: "Toggle".to_owned(),
            encoded_as: Ty::Bool,
            members: vec![
                ABIEnumMember {
                    name: "Off".to_owned(),
                    value: "false".to_owned(),
                    description: String::new(),
                },
                ABIEnumMember {
                    name: "On".to_owned(),
                    value: "true".to_owned(),
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
        }];

        let mut builder = CellBuilder::new();
        builder.store_bit(true).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(
            &mut slice,
            &abi,
            &Ty::EnumRef {
                enum_name: "Toggle".to_owned(),
            },
        )
        .unwrap();

        let Data::Object(object) = data else {
            panic!("expected object");
        };
        assert_eq!(object.name, "Toggle.On");
        assert_eq!(object.fields.len(), 1);
        assert_eq!(object.fields[0].name, "value");
        assert!(matches!(object.fields[0].field_type, Ty::Bool));
        assert!(matches!(object.fields[0].value, Data::Bool(true)));
    }

    #[test]
    fn decodes_address_opt_none() {
        let abi = empty_abi();
        let mut builder = CellBuilder::new();
        builder.store_uint(0, 2).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(&mut slice, &abi, &Ty::AddressOpt).unwrap();
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

        let data = decode(&mut slice, &abi, &Ty::AddressAny).unwrap();
        assert!(matches!(data, Data::Address(_)));
    }

    #[test]
    fn decodes_string_from_ref_cell() {
        let abi = empty_abi();
        let string_cell = build_snake_bytes_cell(b"hello");

        let mut builder = CellBuilder::new();
        builder.store_reference(string_cell).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = decode(&mut slice, &abi, &Ty::String).unwrap();
        assert!(matches!(data, Data::String(value) if value == "hello"));
    }
}
