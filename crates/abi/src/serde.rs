use crate::{BaseTypeInfo, TypeAbi, TypeInfo};
use num_bigint::BigInt;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellSlice, Load};
use tycho_types::models::{ExtAddr, IntAddr};

#[derive(Debug)]
pub enum Data {
    Null,
    Number(BigInt),
    Bool(bool),
    Address(IntAddr),
    ExtAddress(ExtAddr),
    Cell(Cell),
    RemainingBitsAndRefs(Cell),
    Bits((Vec<u8>, usize)),
    Object(DataObject),
}

#[derive(Debug)]
pub struct DataObject {
    pub name: String,
    pub fields: Vec<DataField>,
}

#[derive(Debug)]
pub struct DataField {
    pub name: String,
    pub value: Data,
}

pub fn decode(
    data: &mut CellSlice,
    abi: &Vec<TypeAbi>,
    type_abi: &TypeAbi,
) -> anyhow::Result<Data> {
    let mut object = DataObject {
        name: type_abi.name.clone(),
        fields: vec![],
    };

    if let Some(opcode) = type_abi.opcode
        && let Some(opcode_width) = type_abi.opcode_width
    {
        let actual_opcode = data.load_uint(opcode_width as u16)?;
        if actual_opcode != opcode as u64 {
            anyhow::bail!(
                "Invalid opcode for type '{}': expected 0x{:x}, got 0x${:x}",
                type_abi.name,
                opcode,
                actual_opcode
            );
        }
    }

    for field in &type_abi.fields {
        let value = decode_field(data, abi, &field.type_info)?;
        object.fields.push(DataField {
            name: field.name.clone(),
            value,
        })
    }

    Ok(Data::Object(object))
}

fn decode_field(
    data: &mut CellSlice,
    abi: &Vec<TypeAbi>,
    type_info: &TypeInfo,
) -> anyhow::Result<Data> {
    match &type_info.base {
        BaseTypeInfo::Unserializable => {
            // anyhow::bail!(
            //     "cannot decode {} type since it unserializable",
            //     type_info.human_readable
            // );
            Ok(Data::Null)
        }
        BaseTypeInfo::Int { width } => {
            let num = data.load_bigint(*width as u16, false)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::UInt { width } => {
            let num = data.load_bigint(*width as u16, true)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::Coins => {
            let num = data.load_var_bigint(4, false)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::Bool => {
            let num = data.load_bit()?;
            Ok(Data::Bool(num))
        }
        BaseTypeInfo::Address => {
            if let Ok(int_addr) = IntAddr::load_from(data) {
                return Ok(Data::Address(int_addr));
            }

            anyhow::bail!("expected internal address for address type")
        }
        BaseTypeInfo::AnyAddress => {
            if let Ok(int_addr) = IntAddr::load_from(data) {
                return Ok(Data::Address(int_addr));
            }

            // TODO: load external address
            // if let Ok(int_addr) = ExtAddr::load_from(data) {
            //     return Ok(Data::Address(int_addr))
            // }

            anyhow::bail!("external addresses are not supported yet")
        }
        BaseTypeInfo::Bits { width } => {
            let bits = data.load_prefix(*width as u16, 0)?;
            let bytes = (*width).div_ceil(8);
            let mut data = Vec::with_capacity(bytes);
            data.resize_with(bytes, || 0);
            bits.get_raw(0, &mut data, *width as u16)?;
            Ok(Data::Bits((data, *width)))
        }
        BaseTypeInfo::Bytes { width } => {
            let bytes = *width;
            let width = width * 8; // normalize width to bits
            let bits = data.load_prefix(width as u16, 0)?;
            let mut data = Vec::with_capacity(bytes);
            data.resize_with(bytes, || 0);
            bits.get_raw(0, &mut data, width as u16)?;
            Ok(Data::Bits((data, width)))
        }
        BaseTypeInfo::Cell { inner: inner_type } => {
            let Some(inner_type) = inner_type else {
                // untyped cell
                return Ok(Data::Cell(data.load_reference_cloned()?));
            };

            let value = decode_field(data, abi, inner_type.as_ref())?;
            Ok(value)
        }
        BaseTypeInfo::VarInt16 => {
            let num = data.load_var_bigint(4, true)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::VarInt32 => {
            let num = data.load_var_bigint(8, true)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::VarUInt16 => {
            let num = data.load_var_bigint(4, false)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::VarUInt32 => {
            let num = data.load_var_bigint(8, false)?;
            Ok(Data::Number(num))
        }
        BaseTypeInfo::Struct { name: struct_name } => {
            let Some(type_abi) = abi.iter().find(|ty| &ty.name == struct_name) else {
                anyhow::bail!("Cannot find type '{}'", struct_name);
            };

            let value = decode(data, abi, type_abi)?;
            Ok(value)
        }
        BaseTypeInfo::Nullable { inner } => {
            if inner.base == BaseTypeInfo::Address {
                // address?
                // addr_none$00 or addr_std$10
                if data.has_remaining(2, 0) {
                    // we need at least 2 bits for addr_none
                    let prefix = data.get_uint(0, 2)?;
                    if prefix == 0b00 {
                        // addr_none become null
                        return Ok(Data::Null);
                    }
                }

                let value = decode_field(data, abi, inner)?;
                return Ok(value);
            }

            let has_value = data.load_bit()?;
            if !has_value {
                return Ok(Data::Null);
            }

            let value = decode_field(data, abi, inner)?;
            Ok(value)
        }
        BaseTypeInfo::RemainingBitsAndRefs => {
            // TODO: this is not correct
            let cloned = *data;
            let cell = cloned.cell();
            let cell = Boc::decode(Boc::encode(cell))?;
            let value = Data::RemainingBitsAndRefs(cell);
            Ok(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbiInfo, Field, Pos};
    use tycho_types::cell::CellBuilder;

    #[test]
    fn test_decode() {
        let mut builder = CellBuilder::new();
        builder.store_uint(999, 32).ok();
        builder.store_bit(true).ok();
        builder.store_raw(&[0x01, 0x02, 0x03], 24).ok();
        // builder.store_bit(false).ok();
        builder.store_bit(true).ok();
        builder.store_uint(888, 45).ok();
        let cell = builder.build().expect("build failed");
        let mut slice = cell.as_slice_allow_exotic();

        let abi_type = TypeAbi {
            name: "MyStruct".to_string(),
            opcode: Some(999),
            opcode_width: Some(32),
            fields: vec![
                Field {
                    name: "is_deployed".to_owned(),
                    type_info: TypeInfo {
                        base: BaseTypeInfo::Bool,
                        human_readable: "bool".to_owned(),
                    },
                },
                Field {
                    name: "data".to_owned(),
                    type_info: TypeInfo {
                        base: BaseTypeInfo::Bits { width: 24 },
                        human_readable: "bits24".to_owned(),
                    },
                },
                Field {
                    name: "opt".to_owned(),
                    type_info: TypeInfo {
                        base: BaseTypeInfo::Nullable {
                            inner: Box::new(TypeInfo {
                                base: BaseTypeInfo::Int { width: 45 },
                                human_readable: "int45".to_owned(),
                            }),
                        },
                        human_readable: "bits24".to_owned(),
                    },
                },
            ],
            pos: Pos {
                row: 0,
                column: 0,
                uri: "".to_string(),
            },
        };

        let abi = AbiInfo {
            get_methods: vec![],
            messages: vec![],
            types: vec![abi_type.clone()],
            storage: None,
            entry_point: None,
            external_entry_point: None,
        };

        let result = decode(&mut slice, &abi.types, &abi_type).expect("decode failed");
        assert_eq!(
            format!("{:?}", result),
            "Object(DataObject { name: \"MyStruct\", fields: [DataField { name: \"is_deployed\", value: Bool(true) }, DataField { name: \"data\", value: Bits(([1, 2, 3], 24)) }, DataField { name: \"opt\", value: Number(888) }] })"
        );
    }
}
