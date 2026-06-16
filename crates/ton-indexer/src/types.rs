use tvm_ffi::from_stack::{ArgError, FromStack};
use tvm_ffi::stack::TupleItem;
use tycho_types::cell::{Cell, Load};
use tycho_types::dict::{Dict, DictKey, LoadDictKey};

#[derive(Debug, Clone)]
pub struct Map<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Map<K, V> {
    #[must_use]
    pub const fn new(entries: Vec<(K, V)>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn entries(&self) -> &[(K, V)] {
        &self.entries
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn into_entries(self) -> Vec<(K, V)> {
        self.entries
    }
}

impl<K, V> FromStack for Map<K, V>
where
    K: LoadDictKey,
    for<'a> V: Load<'a>,
{
    fn from_item(item: TupleItem) -> Result<Self, ArgError> {
        match item {
            TupleItem::Null => Ok(Self::new(Vec::new())),
            TupleItem::Tuple(tuple) if tuple.is_empty() => Ok(Self::new(Vec::new())),
            TupleItem::Cell(cell) | TupleItem::Slice(cell) => decode_map_cell(&cell),
            _ => Err(ArgError::TypeMismatch {
                expected: "Cell(Dictionary<K,V>) | Tuple(empty)",
            }),
        }
    }
}

fn decode_map_cell<K, V>(cell: &Cell) -> Result<Map<K, V>, ArgError>
where
    K: DictKey + LoadDictKey,
    for<'a> V: Load<'a>,
{
    let dict = Dict::<K, V>::from_raw(Some(cell.clone()));
    let mut entries = Vec::new();
    for entry in dict.iter() {
        entries.push(entry.map_err(|_| ArgError::CellParse)?);
    }
    Ok(Map::new(entries))
}
