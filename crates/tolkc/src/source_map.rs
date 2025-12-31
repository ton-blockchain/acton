use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSlice, Load};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::prelude::DynCell;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMap {
    pub high_level: HighLevelSourceMap,
    pub debug_marks: HashMap<String, Vec<(i32, i32)>>,
}

impl SourceMap {
    pub fn hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.high_level.version.hash(&mut hasher);
        self.high_level.language.hash(&mut hasher);
        self.high_level.compiler_version.hash(&mut hasher);
        for loc in &self.high_level.locations {
            loc.loc.file.hash(&mut hasher);
            loc.loc.line.hash(&mut hasher);
            loc.loc.column.hash(&mut hasher);
        }
        format!("{:x}", hasher.finish())
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self {
            high_level: Default::default(),
            debug_marks: Default::default(),
        }
    }
}

/// Source map data structure for Tolk compiler output
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HighLevelSourceMap {
    pub version: String,
    pub language: Option<String>,
    pub compiler_version: Option<String>,
    pub globals: Vec<GlobalVariable>,
    pub locations: Vec<DebugLocation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub is_stdlib: bool,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SourceLocation {
    pub file: String,
    pub line: i64,
    pub column: i64,
    pub end_line: i64,
    pub end_column: i64,
    pub length: i64,
}

impl SourceLocation {
    pub fn format(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_path(&self.file),
            self.line + 1,
            self.column + 2
        )
    }

    pub fn normalize_path(file: &String) -> String {
        let normalized = file.replace(".test.tolk.test.tolk", ".test.tolk");

        if let Ok(cwd) = std::env::current_dir() {
            let file_path = Path::new(&normalized);

            if let Ok(relative) = file_path.strip_prefix(&cwd) {
                let relative_str = relative.to_string_lossy();
                if relative_str.len() < normalized.len()
                    || normalized.starts_with(cwd.to_string_lossy().as_ref())
                {
                    return relative_str.to_string();
                }
            }
        }

        normalized
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BytecodeLocation {
    pub hash: String,
    pub offset: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DebugLocation {
    pub idx: i64,
    pub loc: SourceLocation,
    pub variables: Vec<Variable>,
    pub context: EntryContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_temporary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constant_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_qualifier_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EntryContext {
    pub description: EntryContextDescription,
    pub inlining: InliningInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_function: Option<String>,
    pub containing_function: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EntryContextDescription {
    Basic {
        ast_kind: String,
    },
    Assert {
        ast_kind: String,
        is_assert_throw: bool,
        condition: String,
    },
    BinaryOperator {
        ast_kind: String,
        description: String,
    },
}

impl Default for EntryContextDescription {
    fn default() -> Self {
        EntryContextDescription::Basic {
            ast_kind: "".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InliningInfo {
    pub inlined_to_func: Option<String>,
    pub containing_func_inline_mode: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugInfo {
    pub opcode: String,
    pub line_str: String,
    pub line_off: String,
}

fn slice_to_string(slice: &mut CellSlice, len: usize) -> String {
    let mut out = String::new();
    for _ in 0..len {
        let bit = slice.load_bit().unwrap();
        out.push(if bit { '1' } else { '0' });
    }
    out
}

fn read_label(slice: &mut CellSlice, m: usize) -> String {
    if slice.load_bit().unwrap() {
        if slice.load_bit().unwrap() {
            let bit = slice.load_bit().unwrap();
            let len_bits = (m as f64 + 1.0).log2().ceil() as usize;
            let len = slice.load_uint(len_bits as u16).unwrap() as usize;
            (if bit { "1" } else { "0" }).repeat(len)
        } else {
            let len_bits = (m as f64 + 1.0).log2().ceil() as usize;
            let len = slice.load_uint(len_bits as u16).unwrap() as usize;
            slice_to_string(slice, len)
        }
    } else {
        let mut len = 0;
        while slice.load_bit().unwrap() {
            len += 1;
        }
        slice_to_string(slice, len)
    }
}

fn get_final_slice(dc: &Cell, key: &str) -> Cell {
    let mut dict = dc.as_slice().unwrap();
    let lbl = read_label(&mut dict, key.len());

    if !key.starts_with(&lbl) {
        panic!("Invalid label");
    }

    if lbl.len() == key.len() {
        return dc.clone();
    }

    let mut child = dyn_cell_to_cell(dict.load_reference().unwrap());
    if key.chars().nth(lbl.len()) == Some('1') {
        child = dyn_cell_to_cell(dict.load_reference().unwrap());
    }

    get_final_slice(&child, &key[(lbl.len() + 1)..])
}

fn dyn_cell_to_cell(cell: &DynCell) -> Cell {
    Boc::decode(Boc::encode(cell)).unwrap()
}

fn get_real_code_hashes(code: &Cell) -> HashMap<String, (String, i32)> {
    let mut dict_c = code.as_slice().unwrap();
    let dict_cell = dyn_cell_to_cell(dict_c.load_reference().unwrap());
    let mut dict_slice = dict_cell.as_slice().unwrap();
    let d = RawDict::<19>::load_from_root_ext(&mut dict_slice, Cell::empty_context()).unwrap();

    let mut r = HashMap::new();

    for kv in d.iter() {
        let kv = kv.unwrap();
        let key_slice = kv.0.as_data_slice();

        let mut builder = CellBuilder::new();
        builder.store_slice(kv.1).unwrap();
        let v = builder.build().unwrap();

        let idx_key = slice_to_string(&mut key_slice.clone(), 19);

        let final_slice = get_final_slice(&dict_cell, &idx_key);
        let original_slice = kv.1;

        r.insert(
            v.repr_hash().to_string().to_uppercase(),
            (
                final_slice.repr_hash().to_string().to_uppercase(),
                final_slice.bit_len() as i32 - original_slice.size_bits() as i32,
            ),
        );
    }

    r
}

pub fn parse_marks_dict(
    marks_boc64: &String,
    code_boc64: &String,
) -> HashMap<String, Vec<(i32, i32)>> {
    let code_cell = Boc::decode_base64(code_boc64).unwrap();

    let real_code_hashes = get_real_code_hashes(&code_cell);

    let debug_marks_cell = Boc::decode_base64(marks_boc64).unwrap();

    let dict = RawDict::<256>::from(Some(debug_marks_cell));
    let mut marks = HashMap::<String, Vec<(i32, i32)>>::new();

    dict.iter().for_each(|kv| {
        let kv = kv.unwrap();
        let hash = kv.0.as_data_slice().load_biguint(256).unwrap();
        let mut hash = format!("{:x}", hash).to_uppercase();
        if hash.len() < 64 {
            hash = "0".repeat(64 - hash.len()) + hash.as_str()
        }

        let mut slice = kv.1;
        let is_normal = slice.load_bit().unwrap();

        let final_hash = if is_normal {
            hash.clone()
        } else if real_code_hashes.contains_key(&hash) {
            real_code_hashes.get(&hash).unwrap().0.clone()
        } else {
            hash.clone() // TODO: or return?
        };

        let dict_inner = Dict::<u32, CellSlice>::load_from(&mut slice).unwrap();

        dict_inner.iter().for_each(|kv| {
            let mut kv = kv.unwrap();
            let debug_id = kv.0;
            let mut ref_ = kv.1.load_reference().unwrap().as_slice().unwrap();
            let dict_marks_inner =
                RawDict::<10>::load_from_root_ext(&mut ref_, Cell::empty_context()).unwrap();

            dict_marks_inner.iter().for_each(|kv| {
                let kv = kv.unwrap();
                let offset = kv.0.as_data_slice().load_uint(10).unwrap();

                let adjusted_offset = offset as i32
                    + (if is_normal {
                        0
                    } else {
                        real_code_hashes.get(&hash).map(|r| r.1).unwrap_or(0)
                    });

                let old_value = marks.get_mut(&final_hash);
                if let Some(old_value) = old_value {
                    old_value.push((adjusted_offset, debug_id as i32))
                } else {
                    marks.insert(final_hash.clone(), vec![(adjusted_offset, debug_id as i32)]);
                }
            });
        });
    });
    marks
}
