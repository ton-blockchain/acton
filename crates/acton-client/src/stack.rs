use crate::{AbiError, BitString, Dictionary, OwnedSlice, Tuple, TupleItem};
use num_bigint::BigInt;
use std::collections::VecDeque;
use tycho_types::cell::{Cell, CellBuilder, CellSliceRange};
use tycho_types::cell::{Load, Store};

pub fn invalid_data<T>(message: impl Into<String>) -> Result<T, AbiError> {
    Err(AbiError::InvalidData(message.into()))
}

pub fn unsupported<T>(message: impl Into<String>) -> Result<T, AbiError> {
    Err(AbiError::Unsupported(message.into()))
}

#[derive(Debug, Clone)]
pub struct StackReader {
    items: VecDeque<TupleItem>,
}

impl StackReader {
    #[must_use]
    pub fn new(items: Vec<TupleItem>) -> Self {
        Self {
            items: items.into(),
        }
    }

    pub fn from_tuple(tuple: Tuple, expected_width: usize) -> Result<Self, AbiError> {
        if tuple.0.len() != expected_width {
            return Err(AbiError::InvalidData(format!(
                "expected {expected_width} stack items, got {}",
                tuple.0.len()
            )));
        }
        Ok(Self::new(tuple.0))
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.items.len()
    }

    pub fn ensure_empty(&self) -> Result<(), AbiError> {
        if self.items.is_empty() {
            Ok(())
        } else {
            Err(AbiError::InvalidData(format!(
                "expected end of stack, got {} items",
                self.items.len()
            )))
        }
    }

    pub fn pop(&mut self) -> Result<TupleItem, AbiError> {
        self.items
            .pop_front()
            .ok_or_else(|| AbiError::InvalidData("unexpected end of stack".to_owned()))
    }

    pub fn peek(&self, index: usize) -> Result<&TupleItem, AbiError> {
        self.items
            .get(index)
            .ok_or_else(|| AbiError::InvalidData("unexpected end of stack".to_owned()))
    }

    pub fn skip(&mut self, count: usize) -> Result<(), AbiError> {
        if self.items.len() < count {
            return Err(AbiError::InvalidData("unexpected end of stack".to_owned()));
        }
        self.items.drain(..count);
        Ok(())
    }

    pub fn read_int(&mut self) -> Result<BigInt, AbiError> {
        match self.pop()? {
            TupleItem::Int(value) => Ok(value),
            _ => Err(type_mismatch("int")),
        }
    }

    pub fn read_bool(&mut self) -> Result<bool, AbiError> {
        Ok(self.read_int()? != BigInt::from(0))
    }

    pub fn read_cell(&mut self) -> Result<Cell, AbiError> {
        match self.pop()? {
            TupleItem::Cell(cell) | TupleItem::Slice(cell) | TupleItem::Builder(cell) => Ok(cell),
            _ => Err(type_mismatch("cell/slice/builder")),
        }
    }

    pub fn read_owned_slice(&mut self) -> Result<OwnedSlice, AbiError> {
        Ok(OwnedSlice::full(self.read_cell()?))
    }

    pub fn read_builder(&mut self) -> Result<CellBuilder, AbiError> {
        let cell = self.read_cell()?;
        let mut builder = CellBuilder::new();
        builder.store_slice(cell.as_slice()?)?;
        Ok(builder)
    }

    pub fn read_string(&mut self) -> Result<String, AbiError> {
        let cell = self.read_cell()?;
        Tuple::parse_snake_string(&cell)
            .ok_or_else(|| AbiError::InvalidData("expected a snake string".to_owned()))
    }

    pub fn read_tuple(&mut self, expected_width: Option<usize>) -> Result<StackReader, AbiError> {
        let TupleItem::Tuple(tuple) = self.pop()? else {
            return Err(type_mismatch("tuple"));
        };
        if let Some(expected_width) = expected_width
            && tuple.0.len() != expected_width
        {
            return Err(AbiError::InvalidData(format!(
                "expected {expected_width} tuple items, got {}",
                tuple.0.len()
            )));
        }
        Ok(Self::new(tuple.0))
    }

    pub fn read_nullable<T>(
        &mut self,
        load: impl FnOnce(&mut Self) -> Result<T, AbiError>,
    ) -> Result<Option<T>, AbiError> {
        if matches!(self.items.front(), Some(TupleItem::Null)) {
            self.items.pop_front();
            Ok(None)
        } else {
            load(self).map(Some)
        }
    }

    pub fn read_wide_nullable<T>(
        &mut self,
        stack_width: usize,
        load: impl FnOnce(&mut Self) -> Result<T, AbiError>,
    ) -> Result<Option<T>, AbiError> {
        let TupleItem::Int(type_id) = self.peek(stack_width.saturating_sub(1))? else {
            return Err(type_mismatch("nullable type id"));
        };
        if type_id == &BigInt::from(0) {
            self.skip(stack_width)?;
            return Ok(None);
        }
        let value = load(self)?;
        match self.pop()? {
            TupleItem::Int(_) => Ok(Some(value)),
            _ => Err(type_mismatch("nullable type id")),
        }
    }

    pub fn read_union_tag(&self, stack_width: usize) -> Result<BigInt, AbiError> {
        match self.peek(stack_width.saturating_sub(1))? {
            TupleItem::Int(value) => Ok(value.clone()),
            _ => Err(type_mismatch("union type id")),
        }
    }

    pub fn prepare_union_variant(
        &mut self,
        total_width: usize,
        variant_width: usize,
    ) -> Result<(), AbiError> {
        let padding = total_width
            .checked_sub(variant_width + 1)
            .ok_or_else(|| AbiError::InvalidData("invalid union stack width".to_owned()))?;
        self.skip(padding)
    }

    pub fn finish_union_variant(&mut self) -> Result<(), AbiError> {
        match self.pop()? {
            TupleItem::Int(_) => Ok(()),
            _ => Err(type_mismatch("union type id")),
        }
    }
}

pub fn write_int(value: &BigInt, output: &mut Vec<TupleItem>) {
    output.push(TupleItem::Int(value.clone()));
}

pub fn write_bool(value: bool, output: &mut Vec<TupleItem>) {
    output.push(TupleItem::Int(BigInt::from(if value { -1 } else { 0 })));
}

pub fn write_cell(cell: &Cell, output: &mut Vec<TupleItem>) {
    output.push(TupleItem::Cell(cell.clone()));
}

pub fn write_slice(slice: &OwnedSlice, output: &mut Vec<TupleItem>) -> Result<(), AbiError> {
    let mut builder = CellBuilder::new();
    builder.store_slice(slice.as_slice()?)?;
    output.push(TupleItem::Slice(builder.build()?));
    Ok(())
}

pub fn write_bits(bits: &BitString, output: &mut Vec<TupleItem>) -> Result<(), AbiError> {
    write_slice(&bits.0, output)
}

pub fn write_string(value: &str, output: &mut Vec<TupleItem>) {
    output.push(TupleItem::Cell(crate::cell::string_to_cell(value)));
}

pub fn write_tlb_slice<T: Store>(value: &T, output: &mut Vec<TupleItem>) -> Result<(), AbiError> {
    let mut builder = CellBuilder::new();
    crate::cell::store_tlb(&mut builder, value)?;
    output.push(TupleItem::Slice(builder.build()?));
    Ok(())
}

pub fn read_tlb_slice<T>(reader: &mut StackReader) -> Result<T, AbiError>
where
    T: for<'a> Load<'a>,
{
    let cell = reader.read_cell()?;
    let mut slice = cell.as_slice()?;
    let value = crate::cell::load_tlb(&mut slice)?;
    crate::cell::ensure_empty(&slice)?;
    Ok(value)
}

pub fn write_tuple(
    output: &mut Vec<TupleItem>,
    write: impl FnOnce(&mut Vec<TupleItem>) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let mut items = Vec::new();
    write(&mut items)?;
    output.push(TupleItem::Tuple(Tuple(items)));
    Ok(())
}

pub fn write_wide_nullable<T>(
    value: Option<&T>,
    stack_width: usize,
    stack_type_id: usize,
    output: &mut Vec<TupleItem>,
    write: impl FnOnce(&T, &mut Vec<TupleItem>) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    match value {
        None => {
            output.extend((0..stack_width.saturating_sub(1)).map(|_| TupleItem::Null));
            output.push(TupleItem::Int(BigInt::from(0)));
        }
        Some(value) => {
            write(value, output)?;
            output.push(TupleItem::Int(BigInt::from(stack_type_id)));
        }
    }
    Ok(())
}

pub fn write_union_variant<T>(
    value: &T,
    total_width: usize,
    variant_width: usize,
    type_id: usize,
    output: &mut Vec<TupleItem>,
    write: impl FnOnce(&T, &mut Vec<TupleItem>) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let padding = total_width
        .checked_sub(variant_width + 1)
        .ok_or_else(|| AbiError::InvalidData("invalid union stack width".to_owned()))?;
    output.extend((0..padding).map(|_| TupleItem::Null));
    write(value, output)?;
    output.push(TupleItem::Int(BigInt::from(type_id)));
    Ok(())
}

pub fn write_array<T>(
    values: &[T],
    output: &mut Vec<TupleItem>,
    mut write: impl FnMut(&T, &mut Vec<TupleItem>) -> Result<(), AbiError>,
    nested: bool,
) -> Result<(), AbiError> {
    let mut items = Vec::with_capacity(values.len());
    for value in values {
        let mut value_items = Vec::new();
        write(value, &mut value_items)?;
        if nested || value_items.len() != 1 {
            items.push(TupleItem::Tuple(Tuple(value_items)));
        } else {
            items.push(value_items.pop().expect("one item was checked"));
        }
    }
    output.push(TupleItem::Tuple(Tuple(items)));
    Ok(())
}

pub fn read_array<T>(
    reader: &mut StackReader,
    mut load: impl FnMut(&mut StackReader) -> Result<T, AbiError>,
    nested: bool,
) -> Result<Vec<T>, AbiError> {
    let mut items = reader.read_tuple(None)?;
    let mut values = Vec::new();
    while items.remaining() != 0 {
        if nested {
            let mut item = items.read_tuple(None)?;
            values.push(load(&mut item)?);
            item.ensure_empty()?;
        } else {
            values.push(load(&mut items)?);
        }
    }
    Ok(values)
}

pub fn write_lisp_list<T>(
    values: &[T],
    output: &mut Vec<TupleItem>,
    mut write: impl FnMut(&T, &mut Vec<TupleItem>) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let mut tail = TupleItem::Null;
    for value in values.iter().rev() {
        let mut pair = Vec::new();
        let mut head = Vec::new();
        write(value, &mut head)?;
        pair.push(if head.len() == 1 {
            head.pop().expect("one item was checked")
        } else {
            TupleItem::Tuple(Tuple(head))
        });
        pair.push(tail);
        tail = TupleItem::Tuple(Tuple(pair));
    }
    output.push(tail);
    Ok(())
}

pub fn read_lisp_list<T>(
    reader: &mut StackReader,
    mut load: impl FnMut(&mut StackReader) -> Result<T, AbiError>,
    nested: bool,
) -> Result<Vec<T>, AbiError> {
    let mut current = reader.pop()?;
    let mut values = Vec::new();
    loop {
        match current {
            TupleItem::Null => return Ok(values),
            TupleItem::Tuple(tuple) if tuple.0.len() == 2 => {
                let mut pair = tuple.0.into_iter();
                let head = pair.next().expect("tuple length was checked");
                current = pair.next().expect("tuple length was checked");
                let mut head_reader = match (nested, head) {
                    (true, TupleItem::Tuple(tuple)) => StackReader::new(tuple.0),
                    (_, item) => StackReader::new(vec![item]),
                };
                values.push(load(&mut head_reader)?);
                head_reader.ensure_empty()?;
            }
            _ => {
                return Err(AbiError::InvalidData(
                    "malformed lisp_list on stack".to_owned(),
                ));
            }
        }
    }
}

pub fn write_dictionary<K, V>(
    values: &Dictionary<K, V>,
    output: &mut Vec<TupleItem>,
    make_cell: impl FnOnce(&Dictionary<K, V>) -> Result<Cell, AbiError>,
) -> Result<(), AbiError> {
    if values.is_empty() {
        output.push(TupleItem::Null);
    } else {
        output.push(TupleItem::Cell(make_cell(values)?));
    }
    Ok(())
}

#[must_use]
pub fn full_slice_parts(cell: Cell) -> (CellSliceRange, Cell) {
    (CellSliceRange::full(&cell), cell)
}

fn type_mismatch(expected: &str) -> AbiError {
    AbiError::InvalidData(format!("expected {expected} on TVM stack"))
}
