use crate::abi::{ABICustomPackUnpack, ABIDeclaration, ContractABI, Ty, UnionVariant};
use crate::source_map::{Declaration, SourceMap};
use crate::types_kernel::{TyIdx, TyResolver, render_ty};
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use tycho_types::cell::{Cell, CellBuilder, CellSlice, Load};
use tycho_types::dict;
use tycho_types::models::{AnyAddr, ExtAddr, IntAddr, StdAddr};

#[derive(Debug, Clone)]
pub enum UnpackedValue {
    Null,
    Number(BigInt),
    Bool(bool),
    String(String),
    Address(IntAddr),
    ExtAddress(ExtAddr),
    Cell(Cell),
    RemainingBitsAndRefs(Cell),
    Bits((Vec<u8>, usize)),
    Array(Vec<UnpackedValue>),
    Map(Vec<(UnpackedValue, UnpackedValue)>),
    Object {
        name: String,
        fields: Vec<(String, UnpackedValue)>,
    },
}

#[derive(Clone, Copy)]
struct SchemaPrefix {
    prefix_num: u64,
    prefix_len: i32,
}

struct SchemaStructDecl<'a> {
    prefix: Option<SchemaPrefix>,
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

struct SchemaAliasDecl<'a> {
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

struct SchemaEnumDecl<'a> {
    encoded_as_ty_idx: TyIdx,
    members: Vec<SchemaEnumMember>,
    custom_pack_unpack: Option<&'a ABICustomPackUnpack>,
}

#[derive(Clone)]
struct SchemaField {
    name: String,
    ty_idx: TyIdx,
}

#[derive(Clone)]
struct SchemaEnumMember {
    name: String,
    value: String,
}

trait UnpackSchema: TyResolver {
    fn struct_decl_info(&self, target_name: &str) -> Option<SchemaStructDecl<'_>>;
    fn alias_decl_info(&self, target_name: &str) -> Option<SchemaAliasDecl<'_>>;
    fn enum_decl_info(&self, target_name: &str) -> Option<SchemaEnumDecl<'_>>;
    fn struct_fields_for(&self, ty_idx: TyIdx) -> Option<Vec<SchemaField>>;
    fn alias_target_for(&self, ty_idx: TyIdx) -> Option<TyIdx>;
}

impl UnpackSchema for SourceMap {
    fn struct_decl_info(&self, target_name: &str) -> Option<SchemaStructDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Struct(struct_decl) if struct_decl.name == target_name => {
                Some(SchemaStructDecl {
                    prefix: struct_decl.prefix.as_ref().map(|prefix| SchemaPrefix {
                        prefix_num: prefix.prefix_num,
                        prefix_len: prefix.prefix_len,
                    }),
                    custom_pack_unpack: struct_decl.custom_pack_unpack.as_ref(),
                })
            }
            _ => None,
        })
    }

    fn alias_decl_info(&self, target_name: &str) -> Option<SchemaAliasDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Alias(alias_decl) if alias_decl.name == target_name => {
                Some(SchemaAliasDecl {
                    custom_pack_unpack: alias_decl.custom_pack_unpack.as_ref(),
                })
            }
            _ => None,
        })
    }

    fn enum_decl_info(&self, target_name: &str) -> Option<SchemaEnumDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Enum(enum_decl) if enum_decl.name == target_name => Some(SchemaEnumDecl {
                encoded_as_ty_idx: enum_decl.encoded_as_ty_idx,
                members: enum_decl
                    .members
                    .iter()
                    .map(|member| SchemaEnumMember {
                        name: member.name.clone(),
                        value: member.value.clone(),
                    })
                    .collect(),
                custom_pack_unpack: enum_decl.custom_pack_unpack.as_ref(),
            }),
            _ => None,
        })
    }

    fn struct_fields_for(&self, ty_idx: TyIdx) -> Option<Vec<SchemaField>> {
        self.struct_fields_of(ty_idx).map(|fields| {
            fields
                .into_iter()
                .map(|field| SchemaField {
                    name: field.name,
                    ty_idx: field.ty_idx,
                })
                .collect()
        })
    }

    fn alias_target_for(&self, ty_idx: TyIdx) -> Option<TyIdx> {
        self.alias_target_of(ty_idx)
    }
}

impl UnpackSchema for ContractABI {
    fn struct_decl_info(&self, target_name: &str) -> Option<SchemaStructDecl<'_>> {
        self.declarations.iter().find_map(|decl| match decl {
            ABIDeclaration::Struct {
                name,
                prefix,
                custom_pack_unpack,
                ..
            } if name == target_name => Some(SchemaStructDecl {
                prefix: prefix.as_ref().map(|prefix| SchemaPrefix {
                    prefix_num: prefix.prefix_num,
                    prefix_len: prefix.prefix_len,
                }),
                custom_pack_unpack: custom_pack_unpack.as_ref(),
            }),
            _ => None,
        })
    }

    fn alias_decl_info(&self, target_name: &str) -> Option<SchemaAliasDecl<'_>> {
        self.declarations.iter().find_map(|decl| match decl {
            ABIDeclaration::Alias {
                name,
                custom_pack_unpack,
                ..
            } if name == target_name => Some(SchemaAliasDecl {
                custom_pack_unpack: custom_pack_unpack.as_ref(),
            }),
            _ => None,
        })
    }

    fn enum_decl_info(&self, target_name: &str) -> Option<SchemaEnumDecl<'_>> {
        self.declarations.iter().find_map(|decl| match decl {
            ABIDeclaration::Enum {
                name,
                encoded_as_ty_idx,
                members,
                custom_pack_unpack,
                ..
            } if name == target_name => Some(SchemaEnumDecl {
                encoded_as_ty_idx: *encoded_as_ty_idx,
                members: members
                    .iter()
                    .map(|member| SchemaEnumMember {
                        name: member.name.clone(),
                        value: member.value.clone(),
                    })
                    .collect(),
                custom_pack_unpack: custom_pack_unpack.as_ref(),
            }),
            _ => None,
        })
    }

    fn struct_fields_for(&self, ty_idx: TyIdx) -> Option<Vec<SchemaField>> {
        self.struct_fields_of(ty_idx).ok().map(|fields| {
            fields
                .into_iter()
                .map(|field| SchemaField {
                    name: field.name,
                    ty_idx: field.ty_idx,
                })
                .collect()
        })
    }

    fn alias_target_for(&self, ty_idx: TyIdx) -> Option<TyIdx> {
        self.alias_target_of(ty_idx).ok()
    }
}

pub fn unpack_from_slice(
    data: &mut CellSlice<'_>,
    symbols: &SourceMap,
    ty_idx: TyIdx,
) -> anyhow::Result<UnpackedValue> {
    unpack_type(data, symbols, ty_idx)
}

pub fn unpack_from_abi_slice(
    data: &mut CellSlice<'_>,
    abi: &ContractABI,
    ty_idx: TyIdx,
) -> anyhow::Result<UnpackedValue> {
    unpack_type(data, abi, ty_idx)
}

fn unpack_type<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    ty_idx: TyIdx,
) -> anyhow::Result<UnpackedValue> {
    let ty = symbols
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
    match ty {
        Ty::Int => unsupported_type("int"),
        Ty::Bool => Ok(UnpackedValue::Bool(data.load_bit()?)),
        Ty::Cell => Ok(UnpackedValue::Cell(data.load_reference_cloned()?)),
        Ty::Slice => unsupported_type("slice"),
        Ty::Builder => unsupported_type("builder"),
        Ty::Callable => unsupported_type("callable"),
        Ty::String => {
            let cell = data.load_reference_cloned()?;
            let string =
                parse_snake_string(&cell).ok_or_else(|| anyhow!("expected snake string"))?;
            Ok(UnpackedValue::String(string))
        }
        Ty::Coins => Ok(UnpackedValue::Number(data.load_var_bigint(4, false)?)),
        Ty::Void => unsupported_type("void"),
        Ty::Address => Ok(UnpackedValue::Address(IntAddr::load_from(data)?)),
        Ty::AddressExt => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::Ext(ext_addr) => UnpackedValue::ExtAddress(ext_addr),
            _ => anyhow::bail!("expected external address for addressExt"),
        }),
        Ty::AddressAny => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => UnpackedValue::Null,
            AnyAddr::Ext(ext_addr) => UnpackedValue::ExtAddress(ext_addr),
            AnyAddr::Std(addr) => UnpackedValue::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => UnpackedValue::Address(IntAddr::Var(addr)),
        }),
        Ty::AddressOpt => Ok(match AnyAddr::load_from(data)? {
            AnyAddr::None => UnpackedValue::Null,
            AnyAddr::Std(addr) => UnpackedValue::Address(IntAddr::Std(addr)),
            AnyAddr::Var(addr) => UnpackedValue::Address(IntAddr::Var(addr)),
            AnyAddr::Ext(_) => anyhow::bail!("expected internal address or null for addressOpt"),
        }),
        Ty::UintN { n } => Ok(UnpackedValue::Number(data.load_bigint(*n as u16, false)?)),
        Ty::IntN { n } => Ok(UnpackedValue::Number(data.load_bigint(*n as u16, true)?)),
        Ty::VaruintN { n } => {
            let len_bits = varint_len_bits(*n)?;
            Ok(UnpackedValue::Number(
                data.load_var_bigint(len_bits, false)?,
            ))
        }
        Ty::VarintN { n } => {
            let len_bits = varint_len_bits(*n)?;
            Ok(UnpackedValue::Number(data.load_var_bigint(len_bits, true)?))
        }
        Ty::BitsN { n } => Ok(UnpackedValue::Bits(load_bits(data, *n)?)),
        Ty::ArrayOf { .. } => unsupported_type("arrayOf"),
        Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
            let mut values = Vec::with_capacity(items_ty_idx.len());
            for &item_ty_idx in items_ty_idx {
                values.push(unpack_type(data, symbols, item_ty_idx)?);
            }
            Ok(UnpackedValue::Array(values))
        }
        Ty::NullLiteral => Ok(UnpackedValue::Null),
        Ty::GenericT { name_t } => anyhow::bail!("unresolved generic type {name_t}"),
        Ty::StructRef { struct_name, .. } => unpack_struct(data, symbols, ty_idx, struct_name),
        Ty::EnumRef { enum_name } => unpack_enum(data, symbols, enum_name),
        Ty::AliasRef { alias_name, .. } => unpack_alias(data, symbols, ty_idx, alias_name),
        Ty::Remaining => Ok(UnpackedValue::RemainingBitsAndRefs(remaining_as_cell(
            data,
        )?)),
        Ty::CellOf { inner_ty_idx } => {
            let mut ref_slice = data.load_reference_as_slice()?;
            let value = unpack_type(&mut ref_slice, symbols, *inner_ty_idx)?;
            ensure_fully_consumed(&ref_slice, "Cell<T> payload")?;
            Ok(UnpackedValue::Object {
                name: "Cell".to_owned(),
                fields: vec![("ref".to_owned(), value)],
            })
        }
        Ty::LispListOf { .. } => unsupported_type("lispListOf"),
        Ty::Union { variants, .. } => unpack_union(data, symbols, variants),
        Ty::Nullable { inner_ty_idx, .. } => {
            if !data.load_bit()? {
                return Ok(UnpackedValue::Null);
            }
            unpack_type(data, symbols, *inner_ty_idx)
        }
        Ty::MapKV {
            key_ty_idx,
            value_ty_idx,
        } => unpack_map(data, symbols, *key_ty_idx, *value_ty_idx),
        Ty::Unknown => unsupported_type("unknown"),
    }
}

fn unpack_struct<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    ty_idx: TyIdx,
    struct_name: &str,
) -> anyhow::Result<UnpackedValue> {
    let decl = symbols
        .struct_decl_info(struct_name)
        .ok_or_else(|| anyhow!("struct {struct_name} referenced by type was not found"))?;
    ensure_standard_layout(struct_name, decl.custom_pack_unpack)?;

    if let Some(prefix) = decl.prefix {
        check_prefix(data, prefix.prefix_num, prefix.prefix_len, struct_name)?;
    }

    let struct_fields = symbols
        .struct_fields_for(ty_idx)
        .ok_or_else(|| anyhow!("failed to resolve fields for {struct_name}"))?;
    let mut fields = Vec::with_capacity(struct_fields.len());
    for field in &struct_fields {
        let value = unpack_type(data, symbols, field.ty_idx)
            .with_context(|| format!("failed to decode field {struct_name}.{}", field.name))?;
        fields.push((field.name.clone(), value));
    }

    Ok(UnpackedValue::Object {
        name: struct_name.to_owned(),
        fields,
    })
}

fn unpack_alias<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    ty_idx: TyIdx,
    alias_name: &str,
) -> anyhow::Result<UnpackedValue> {
    let decl = symbols
        .alias_decl_info(alias_name)
        .ok_or_else(|| anyhow!("alias {alias_name} referenced by type was not found"))?;
    ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
    let target_ty_idx = symbols
        .alias_target_for(ty_idx)
        .ok_or_else(|| anyhow!("failed to resolve target for alias {alias_name}"))?;
    unpack_type(data, symbols, target_ty_idx)
}

fn unpack_enum<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    enum_name: &str,
) -> anyhow::Result<UnpackedValue> {
    let decl = symbols
        .enum_decl_info(enum_name)
        .ok_or_else(|| anyhow!("enum {enum_name} referenced by type was not found"))?;
    ensure_standard_layout(enum_name, decl.custom_pack_unpack)?;

    let encoded_ty_idx = parse_enum_encoded_as(symbols, decl.encoded_as_ty_idx)?;
    let value = unpack_type(data, symbols, encoded_ty_idx)?;
    let label = find_enum_member_name(&value, &decl.members).map_or_else(
        || format!("{enum_name}({})", format_enum_encoded_value(&value)),
        |member_name| format!("{enum_name}.{member_name}"),
    );

    Ok(UnpackedValue::Object {
        name: label,
        fields: vec![("value".to_owned(), value)],
    })
}

fn format_enum_encoded_value(value: &UnpackedValue) -> String {
    match value {
        UnpackedValue::Number(value) => value.to_string(),
        UnpackedValue::Bool(value) => value.to_string(),
        other => format!("{other:?}"),
    }
}

fn unpack_union<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    variants: &[UnionVariant],
) -> anyhow::Result<UnpackedValue> {
    let variants = resolve_union_variants(symbols, variants)?;
    for variant in variants {
        if !matches_prefix(data, variant.prefix_num, variant.prefix_len)? {
            continue;
        }

        if variant.prefix_eat_in_place {
            data.skip_first(
                u16::try_from(variant.prefix_len).context("union prefix length exceeds u16")?,
                0,
            )?;
        }

        let value = unpack_type(data, symbols, variant.variant_ty_idx)?;
        if !variant.has_value_field {
            return Ok(value);
        }

        return Ok(UnpackedValue::Object {
            name: variant.label,
            fields: vec![("value".to_owned(), value)],
        });
    }

    anyhow::bail!("none of union prefixes matched")
}

fn unpack_map<S: UnpackSchema + ?Sized>(
    data: &mut CellSlice<'_>,
    symbols: &S,
    key_ty_idx: TyIdx,
    value_ty_idx: TyIdx,
) -> anyhow::Result<UnpackedValue> {
    let key_bits = map_key_bit_len(symbols, key_ty_idx)?;
    let dict = Option::<Cell>::load_from(data)?;

    let mut entries = Vec::new();
    for entry in dict::RawIter::new(&dict, key_bits) {
        let (key_data, mut value_slice) = entry?;
        let key = unpack_type(&mut key_data.as_data_slice(), symbols, key_ty_idx)
            .context("failed to decode map key")?;
        let value = unpack_type(&mut value_slice, symbols, value_ty_idx)
            .context("failed to decode map value")?;
        ensure_fully_consumed(&value_slice, "map value")?;
        entries.push((key, value));
    }

    Ok(UnpackedValue::Map(entries))
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
    prefix_num: u64,
    prefix_len: i32,
    type_name: &str,
) -> anyhow::Result<()> {
    let prefix_len = u16::try_from(prefix_len).context("negative prefix length")?;
    let actual = data.load_uint(prefix_len)?;
    if actual != prefix_num {
        anyhow::bail!(
            "incorrect prefix for '{type_name}': expected 0x{prefix_num:x}, got 0x{actual:x}"
        );
    }
    Ok(())
}

fn matches_prefix(
    data: &CellSlice<'_>,
    prefix_num: u64,
    prefix_len: usize,
) -> anyhow::Result<bool> {
    let prefix_len = u16::try_from(prefix_len).context("union prefix length exceeds u16")?;
    if !data.has_remaining(prefix_len, 0) {
        return Ok(false);
    }
    Ok(data.get_uint(0, prefix_len)? == prefix_num)
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

fn unsupported_type(name: &str) -> anyhow::Result<UnpackedValue> {
    anyhow::bail!("cannot decode unsupported Tolk type {name}")
}

fn parse_enum_encoded_as<S: UnpackSchema + ?Sized>(
    symbols: &S,
    encoded_as_ty_idx: TyIdx,
) -> anyhow::Result<TyIdx> {
    let encoded_as = symbols
        .ty_by_idx(encoded_as_ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {encoded_as_ty_idx} was not found"))?;
    match encoded_as {
        Ty::Bool
        | Ty::Coins
        | Ty::UintN { .. }
        | Ty::IntN { .. }
        | Ty::VaruintN { .. }
        | Ty::VarintN { .. } => Ok(encoded_as_ty_idx),
        _ => anyhow::bail!(
            "unsupported enum encoding {}",
            render_ty(symbols, encoded_as_ty_idx)
        ),
    }
}

fn find_enum_member_name<'a>(
    value: &UnpackedValue,
    members: &'a [SchemaEnumMember],
) -> Option<&'a str> {
    members.iter().find_map(|member| match value {
        UnpackedValue::Number(number) => member
            .value
            .parse::<BigInt>()
            .ok()
            .filter(|member_value| member_value == number)
            .map(|_| member.name.as_str()),
        UnpackedValue::Bool(boolean) => match member.value.as_str() {
            "false" if !boolean => Some(member.name.as_str()),
            "true" if *boolean => Some(member.name.as_str()),
            _ => None,
        },
        _ => None,
    })
}

fn parse_snake_string(cell: &Cell) -> Option<String> {
    String::from_utf8(parse_snake_bytes(cell)?).ok()
}

fn parse_snake_bytes(cell: &Cell) -> Option<Vec<u8>> {
    let mut parser = cell.as_slice_allow_exotic();
    parse_snake_bytes_slice(&mut parser)
}

fn parse_snake_bytes_slice(parser: &mut CellSlice<'_>) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let bits_to_load = parser.size_bits();
    if !bits_to_load.is_multiple_of(8) {
        return None;
    }

    let mut chunk = vec![0u8; bits_to_load.div_ceil(8) as usize];
    parser.load_raw(&mut chunk, bits_to_load).ok()?;
    bytes.extend_from_slice(&chunk);

    if parser.size_refs() == 0 {
        return Some(bytes);
    }

    let next_cell = parser.load_reference_cloned().ok()?;
    let mut next_parser = next_cell.as_slice_allow_exotic();
    bytes.extend(parse_snake_bytes_slice(&mut next_parser)?);
    Some(bytes)
}

fn map_key_bit_len<S: UnpackSchema + ?Sized>(symbols: &S, ty_idx: TyIdx) -> anyhow::Result<u16> {
    let ty = symbols
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
    match ty {
        Ty::Bool => Ok(1),
        Ty::IntN { n } | Ty::UintN { n } => u16::try_from(*n).context("map key width exceeds u16"),
        Ty::Address => Ok(StdAddr::BITS_WITHOUT_ANYCAST),
        Ty::AliasRef { alias_name, .. } => {
            let decl = symbols
                .alias_decl_info(alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by type was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let target_ty_idx = symbols
                .alias_target_for(ty_idx)
                .ok_or_else(|| anyhow!("failed to resolve target for alias {alias_name}"))?;
            map_key_bit_len(symbols, target_ty_idx)
        }
        Ty::EnumRef { enum_name } => {
            let decl = symbols
                .enum_decl_info(enum_name)
                .ok_or_else(|| anyhow!("enum {enum_name} referenced by type was not found"))?;
            ensure_standard_layout(enum_name, decl.custom_pack_unpack)?;
            map_key_bit_len(
                symbols,
                parse_enum_encoded_as(symbols, decl.encoded_as_ty_idx)?,
            )
        }
        _ => anyhow::bail!("unsupported map key type {}", render_ty(symbols, ty_idx)),
    }
}

#[derive(Clone)]
struct ResolvedUnionVariant {
    variant_ty_idx: TyIdx,
    prefix_num: u64,
    prefix_len: usize,
    prefix_eat_in_place: bool,
    label: String,
    has_value_field: bool,
}

fn resolve_union_variants<S: UnpackSchema + ?Sized>(
    symbols: &S,
    variants: &[UnionVariant],
) -> anyhow::Result<Vec<ResolvedUnionVariant>> {
    let mut simple_labels = Vec::with_capacity(variants.len());
    let mut concrete_variants = Vec::with_capacity(variants.len());
    for variant in variants {
        simple_labels.push(union_label_simple(symbols, variant.variant_ty_idx)?);
        concrete_variants.push(variant.variant_ty_idx);
    }

    let has_duplicates = simple_labels
        .iter()
        .enumerate()
        .any(|(idx, label)| simple_labels[..idx].contains(label));

    variants
        .iter()
        .zip(concrete_variants)
        .zip(simple_labels)
        .map(|((variant, concrete_ty_idx), simple_label)| {
            let is_null = matches!(symbols.ty_by_idx(concrete_ty_idx), Some(Ty::NullLiteral));
            Ok(ResolvedUnionVariant {
                variant_ty_idx: concrete_ty_idx,
                prefix_num: variant.prefix_num,
                prefix_len: variant.prefix_len,
                prefix_eat_in_place: variant.is_prefix_implicit.unwrap_or(false),
                label: if is_null {
                    String::new()
                } else if has_duplicates {
                    render_ty(symbols, concrete_ty_idx)
                } else {
                    simple_label
                },
                has_value_field: !is_null
                    && (has_duplicates || !type_has_own_label(symbols, concrete_ty_idx)?),
            })
        })
        .collect()
}

fn union_label_simple<S: UnpackSchema + ?Sized>(
    symbols: &S,
    ty_idx: TyIdx,
) -> anyhow::Result<String> {
    let ty = symbols
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
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
        Ty::Nullable { inner_ty_idx, .. } => {
            format!("{}?", union_label_simple(symbols, *inner_ty_idx)?)
        }
        Ty::CellOf { .. } => "Cell".to_owned(),
        Ty::Tensor { .. } | Ty::ShapedTuple { .. } => "tensor".to_owned(),
        Ty::MapKV { .. } => "map".to_owned(),
        Ty::EnumRef { enum_name } => enum_name.clone(),
        Ty::StructRef { struct_name, .. } => struct_name.clone(),
        Ty::AliasRef { alias_name, .. } => {
            let decl = symbols
                .alias_decl_info(alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by type was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let target_ty_idx = symbols
                .alias_target_for(ty_idx)
                .ok_or_else(|| anyhow!("failed to resolve target for alias {alias_name}"))?;
            union_label_simple(symbols, target_ty_idx)?
        }
        Ty::GenericT { name_t } => name_t.clone(),
        Ty::Union { variants, .. } => variants
            .iter()
            .map(|variant| union_label_simple(symbols, variant.variant_ty_idx))
            .collect::<anyhow::Result<Vec<_>>>()?
            .join("|"),
        Ty::ArrayOf { .. } => "array".to_owned(),
        Ty::LispListOf { .. } => "lisp_list".to_owned(),
        Ty::Unknown => "unknown".to_owned(),
        Ty::String => "string".to_owned(),
    })
}

fn type_has_own_label<S: UnpackSchema + ?Sized>(
    symbols: &S,
    ty_idx: TyIdx,
) -> anyhow::Result<bool> {
    let ty = symbols
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
    Ok(match ty {
        Ty::StructRef { .. } => true,
        Ty::AliasRef { alias_name, .. } => {
            let decl = symbols
                .alias_decl_info(alias_name)
                .ok_or_else(|| anyhow!("alias {alias_name} referenced by type was not found"))?;
            ensure_standard_layout(alias_name, decl.custom_pack_unpack)?;
            let target_ty_idx = symbols
                .alias_target_for(ty_idx)
                .ok_or_else(|| anyhow!("failed to resolve target for alias {alias_name}"))?;
            type_has_own_label(symbols, target_ty_idx)?
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{
        ABIDeclaration, ABIEnumMember, ABIInternalMessage, ABIOpcode, ABIStorage, ABIStructField,
        ContractABI, StructInstantiation, UnionVariant,
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
            unique_types: Vec::new(),
            struct_instantiations: Vec::new(),
            alias_instantiations: Vec::new(),
            declarations: Vec::new(),
            incoming_messages: Vec::new(),
            incoming_external: Vec::new(),
            outgoing_messages: Vec::new(),
            emitted_events: Vec::new(),
            storage: ABIStorage {
                storage_ty_idx: None,
                storage_at_deployment_ty_idx: None,
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
            compiler_name: "tolk".to_owned(),
            compiler_version: "0".to_owned(),
        }
    }

    fn add_ty(abi: &mut ContractABI, ty: Ty) -> TyIdx {
        if let Some(idx) = abi.unique_types.iter().position(|existing| existing == &ty) {
            return idx;
        }
        let idx = abi.unique_types.len();
        abi.unique_types.push(ty);
        idx
    }

    fn struct_ref(abi: &mut ContractABI, struct_name: &str) -> TyIdx {
        add_ty(
            abi,
            Ty::StructRef {
                struct_name: struct_name.to_owned(),
                type_args_ty_idx: None,
            },
        )
    }

    fn enum_ref(abi: &mut ContractABI, enum_name: &str) -> TyIdx {
        add_ty(
            abi,
            Ty::EnumRef {
                enum_name: enum_name.to_owned(),
            },
        )
    }

    fn build_snake_bytes_cell(bytes: &[u8]) -> Cell {
        let total_bits = bytes.len() * 8;
        if total_bits <= 1015 {
            let mut builder = CellBuilder::new();
            builder.store_raw(bytes, total_bits as u16).unwrap();
            return builder.build().unwrap();
        }

        let mut remaining_bytes = bytes;
        let mut cell_data = Vec::new();
        while !remaining_bytes.is_empty() {
            let chunk_size = std::cmp::min(remaining_bytes.len(), 126);
            let chunk = &remaining_bytes[..chunk_size];
            cell_data.push((chunk, chunk.len() * 8));
            remaining_bytes = &remaining_bytes[chunk_size..];
        }

        let mut next_cell: Option<Cell> = None;
        for (chunk, bits) in cell_data.into_iter().rev() {
            let mut builder = CellBuilder::new();
            builder.store_raw(chunk, bits as u16).unwrap();
            if let Some(next) = next_cell {
                builder.store_reference(next).unwrap();
            }
            next_cell = Some(builder.build().unwrap());
        }
        next_cell.unwrap()
    }

    #[test]
    fn decodes_struct_with_prefix_and_fields() {
        let mut abi = empty_abi();
        let body_ty_idx = struct_ref(&mut abi, "MyMessage");
        let query_ty_idx = add_ty(&mut abi, Ty::UintN { n: 64 });
        let flag_ty_idx = add_ty(&mut abi, Ty::Bool);
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "MyMessage".to_owned(),
            ty_idx: body_ty_idx,
            type_params: None,
            prefix: Some(ABIOpcode {
                prefix_num: 0x12345678,
                prefix_len: 32,
            }),
            fields: vec![
                ABIStructField {
                    name: "queryId".to_owned(),
                    ty_idx: query_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "flag".to_owned(),
                    ty_idx: flag_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
            description: String::new(),
        }];
        abi.incoming_messages = vec![ABIInternalMessage { body_ty_idx }];

        let mut builder = CellBuilder::new();
        builder.store_uint(0x12345678, 32).unwrap();
        builder.store_uint(7, 64).unwrap();
        builder.store_bit(true).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, body_ty_idx).unwrap();

        let UnpackedValue::Object { name, fields } = data else {
            panic!("expected object");
        };
        assert_eq!(name, "MyMessage");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "queryId");
        assert!(matches!(fields[0].1, UnpackedValue::Number(_)));
        assert_eq!(fields[1].0, "flag");
        assert!(matches!(fields[1].1, UnpackedValue::Bool(true)));
        assert_eq!(slice.size_bits(), 0);
    }

    #[test]
    fn decodes_address_any_and_map() {
        let mut abi = empty_abi();
        let payload_ty_idx = struct_ref(&mut abi, "Payload");
        let owner_ty_idx = add_ty(&mut abi, Ty::AddressAny);
        let key_ty_idx = add_ty(&mut abi, Ty::UintN { n: 8 });
        let value_ty_idx = add_ty(&mut abi, Ty::Bool);
        let map_ty_idx = add_ty(
            &mut abi,
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            },
        );
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "Payload".to_owned(),
            ty_idx: payload_ty_idx,
            type_params: None,
            prefix: None,
            fields: vec![
                ABIStructField {
                    name: "owner".to_owned(),
                    ty_idx: owner_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                },
                ABIStructField {
                    name: "items".to_owned(),
                    ty_idx: map_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                },
            ],
            custom_pack_unpack: None,
            description: String::new(),
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

        let data = unpack_from_abi_slice(&mut slice, &abi, payload_ty_idx).unwrap();

        assert!(matches!(data, UnpackedValue::Object { .. }));
    }

    #[test]
    fn decodes_auto_prefixed_union() {
        let mut abi = empty_abi();
        let uint8_ty_idx = add_ty(&mut abi, Ty::UintN { n: 8 });
        let uint16_ty_idx = add_ty(&mut abi, Ty::UintN { n: 16 });
        let union_ty_idx = add_ty(
            &mut abi,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: uint8_ty_idx,
                        prefix_num: 0,
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty_idx: uint16_ty_idx,
                        prefix_num: 1,
                        prefix_len: 1,
                        is_prefix_implicit: Some(true),
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            },
        );
        let mut builder = CellBuilder::new();
        builder.store_bit(true).unwrap();
        builder.store_uint(99, 16).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, union_ty_idx).unwrap();

        let UnpackedValue::Object { name, fields } = data else {
            panic!("expected object");
        };
        assert_eq!(name, "uint16");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert!(matches!(fields[0].1, UnpackedValue::Number(_)));
    }

    #[test]
    fn decodes_generic_struct_fields_with_instantiated_types() {
        let mut abi = empty_abi();
        let generic_ty_idx = add_ty(
            &mut abi,
            Ty::GenericT {
                name_t: "T".to_owned(),
            },
        );
        let boxed_generic_ty_idx = add_ty(
            &mut abi,
            Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args_ty_idx: Some(vec![generic_ty_idx]),
            },
        );
        let value_ty_idx = add_ty(&mut abi, Ty::UintN { n: 32 });
        let boxed_uint_ty_idx = add_ty(
            &mut abi,
            Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args_ty_idx: Some(vec![value_ty_idx]),
            },
        );
        abi.struct_instantiations.push(StructInstantiation {
            ty_idx: boxed_uint_ty_idx,
            struct_name: "Boxed".to_owned(),
            monomorphic_fields_ty_idx: vec![value_ty_idx],
        });
        abi.declarations = vec![ABIDeclaration::Struct {
            name: "Boxed".to_owned(),
            ty_idx: boxed_generic_ty_idx,
            type_params: Some(vec!["T".to_owned()]),
            prefix: None,
            fields: vec![ABIStructField {
                name: "value".to_owned(),
                ty_idx: generic_ty_idx,
                client_ty_idx: None,
                default_value: None,
                description: String::new(),
            }],
            custom_pack_unpack: None,
            description: String::new(),
        }];

        let mut builder = CellBuilder::new();
        builder.store_uint(7, 32).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, boxed_uint_ty_idx).unwrap();

        let UnpackedValue::Object { name, fields } = data else {
            panic!("expected object");
        };
        assert_eq!(name, "Boxed");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert!(matches!(fields[0].1, UnpackedValue::Number(_)));
    }

    #[test]
    fn decodes_enum_to_object_with_raw_value() {
        let mut abi = empty_abi();
        let enum_ty_idx = enum_ref(&mut abi, "Color");
        let encoded_as_ty_idx = add_ty(&mut abi, Ty::UintN { n: 8 });
        abi.declarations = vec![ABIDeclaration::Enum {
            name: "Color".to_owned(),
            ty_idx: enum_ty_idx,
            encoded_as_ty_idx,
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
            description: String::new(),
        }];

        let mut builder = CellBuilder::new();
        builder.store_uint(2, 8).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, enum_ty_idx).unwrap();

        let UnpackedValue::Object { name, fields } = data else {
            panic!("expected object");
        };
        assert_eq!(name, "Color.Blue");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert!(matches!(fields[0].1, UnpackedValue::Number(_)));
    }

    #[test]
    fn decodes_bool_encoded_enum_to_object_with_raw_value() {
        let mut abi = empty_abi();
        let enum_ty_idx = enum_ref(&mut abi, "Toggle");
        let encoded_as_ty_idx = add_ty(&mut abi, Ty::Bool);
        abi.declarations = vec![ABIDeclaration::Enum {
            name: "Toggle".to_owned(),
            ty_idx: enum_ty_idx,
            encoded_as_ty_idx,
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
            description: String::new(),
        }];

        let mut builder = CellBuilder::new();
        builder.store_bit(true).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, enum_ty_idx).unwrap();

        let UnpackedValue::Object { name, fields } = data else {
            panic!("expected object");
        };
        assert_eq!(name, "Toggle.On");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "value");
        assert!(matches!(fields[0].1, UnpackedValue::Bool(true)));
    }

    #[test]
    fn decodes_address_opt_none() {
        let mut abi = empty_abi();
        let ty_idx = add_ty(&mut abi, Ty::AddressOpt);
        let mut builder = CellBuilder::new();
        builder.store_uint(0, 2).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, ty_idx).unwrap();
        assert!(matches!(data, UnpackedValue::Null));
    }

    #[test]
    fn decodes_internal_address_any() {
        let mut abi = empty_abi();
        let ty_idx = add_ty(&mut abi, Ty::AddressAny);
        let mut builder = CellBuilder::new();
        StdAddr::new(0, Default::default())
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, ty_idx).unwrap();
        assert!(matches!(data, UnpackedValue::Address(_)));
    }

    #[test]
    fn decodes_string_from_ref_cell() {
        let mut abi = empty_abi();
        let ty_idx = add_ty(&mut abi, Ty::String);
        let string_cell = build_snake_bytes_cell(b"hello");

        let mut builder = CellBuilder::new();
        builder.store_reference(string_cell).unwrap();
        let cell = builder.build().unwrap();
        let mut slice = cell.as_slice_allow_exotic();

        let data = unpack_from_abi_slice(&mut slice, &abi, ty_idx).unwrap();
        assert!(matches!(data, UnpackedValue::String(value) if value == "hello"));
    }
}
