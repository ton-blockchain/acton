use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSlice, Load};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::prelude::DynCell;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OffsetAndId(pub u16, pub i32);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceMap {
    pub high_level: HighLevelSourceMap,
    pub debug_marks: HashMap<String, Vec<OffsetAndId>>,
}

impl SourceMap {
    #[must_use]
    pub fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.high_level.version.hash(&mut hasher);
        self.high_level.language.hash(&mut hasher);
        self.high_level.compiler_version.hash(&mut hasher);
        for loc in &self.high_level.locations {
            loc.loc.file.hash(&mut hasher);
            loc.loc.line.hash(&mut hasher);
            loc.loc.column.hash(&mut hasher);
        }
        hasher.finish()
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
    #[must_use]
    pub fn format(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_path(&self.file),
            self.line,
            self.column,
        )
    }

    #[must_use]
    pub fn format_normalized(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_path(&self.file),
            self.line + 1,
            self.column + 2,
        )
    }

    #[must_use]
    pub fn format_full(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_temp_name(&self.file),
            self.line,
            self.column
        )
    }

    #[must_use]
    pub fn normalize_path(file: &str) -> String {
        let normalized = Self::normalize_temp_name(file);

        if let Ok(cwd) = std::env::current_dir()
            && let Some(relative) = pathdiff::diff_paths(&normalized, cwd)
        {
            return relative.display().to_string();
        }

        normalized
    }

    fn normalize_temp_name(file: &str) -> String {
        file.replace(".test.tolk.test.tolk", ".test.tolk")
    }

    pub fn parse(s: &str) -> anyhow::Result<Option<Self>> {
        if s.is_empty() {
            return Ok(None);
        }

        let parts = s.rsplitn(3, ':').collect::<Vec<_>>();
        if parts.len() != 3 {
            anyhow::bail!("invalid source location, expected file:line:col, got {s}");
        }

        let file = parts[2].to_owned();
        let line = parts[1].parse::<i64>()?;
        let column = parts[0].parse::<i64>()?;

        Ok(Some(SourceLocation {
            file,
            line,
            column,
            end_line: line,
            end_column: column,
            length: 0,
        }))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BytecodeLocation {
    pub hash: String,
    pub offset: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DebugLocation {
    pub idx: i32,
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
    pub event_function: Option<Arc<str>>,
    pub containing_function: Arc<str>,
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
            ast_kind: String::new(),
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

fn slice_to_string(slice: &mut CellSlice<'_>, len: usize) -> anyhow::Result<String> {
    let mut out = String::new();
    for _ in 0..len {
        let bit = slice.load_bit()?;
        out.push(if bit { '1' } else { '0' });
    }
    Ok(out)
}

fn read_label(slice: &mut CellSlice<'_>, m: usize) -> anyhow::Result<String> {
    if slice.load_bit()? {
        if slice.load_bit()? {
            let bit = slice.load_bit()?;
            let len_bits = (m as f64 + 1.0).log2().ceil() as usize;
            let len = slice.load_uint(len_bits as u16)? as usize;
            Ok((if bit { "1" } else { "0" }).repeat(len))
        } else {
            let len_bits = (m as f64 + 1.0).log2().ceil() as usize;
            let len = slice.load_uint(len_bits as u16)? as usize;
            slice_to_string(slice, len)
        }
    } else {
        let mut len = 0;
        while slice.load_bit()? {
            len += 1;
        }
        slice_to_string(slice, len)
    }
}

#[allow(clippy::useless_let_if_seq)]
fn get_final_slice(dc: &Cell, key: &str) -> anyhow::Result<Cell> {
    let mut dict = dc.as_slice()?;
    let lbl = read_label(&mut dict, key.len())?;

    if !key.starts_with(&lbl) {
        anyhow::bail!("Invalid label");
    }

    if lbl.len() == key.len() {
        return Ok(dc.clone());
    }

    let mut child = dyn_cell_to_cell(dict.load_reference()?);
    if key.chars().nth(lbl.len()) == Some('1') {
        child = dyn_cell_to_cell(dict.load_reference()?);
    }

    get_final_slice(&child, &key[(lbl.len() + 1)..])
}

fn dyn_cell_to_cell(cell: &DynCell) -> Cell {
    Boc::decode(Boc::encode(cell)).expect("cannot decode encoded cell")
}

fn get_real_code_hashes(code: &Cell) -> anyhow::Result<HashMap<String, (String, u16)>> {
    let mut dict_c = code.as_slice_allow_exotic();
    let dict_cell = dyn_cell_to_cell(dict_c.load_reference()?);
    let mut dict_slice = dict_cell.as_slice_allow_exotic();
    let d = RawDict::<19>::load_from_root_ext(&mut dict_slice, Cell::empty_context())?;

    let mut r = HashMap::new();

    for kv in d.iter() {
        let kv = kv?;
        let key_slice = kv.0.as_data_slice();

        let mut builder = CellBuilder::new();
        builder.store_slice(kv.1)?;
        let v = builder.build()?;

        let idx_key = slice_to_string(&mut key_slice.clone(), 19)?;

        let final_slice = get_final_slice(&dict_cell, &idx_key)?;
        let original_slice = kv.1;

        r.insert(
            v.repr_hash().to_string().to_uppercase(),
            (
                final_slice.repr_hash().to_string().to_uppercase(),
                (i32::from(final_slice.bit_len()) - i32::from(original_slice.size_bits())) as u16,
            ),
        );
    }

    Ok(r)
}

pub fn parse_marks_dict(
    marks_boc64: &str,
    code_boc64: &str,
) -> anyhow::Result<HashMap<String, Vec<OffsetAndId>>> {
    let code_cell = Boc::decode_base64(code_boc64)?;

    let real_code_hashes = get_real_code_hashes(&code_cell)?;
    let debug_marks_cell = Boc::decode_base64(marks_boc64)?;

    let dict = RawDict::<256>::from(Some(debug_marks_cell));
    let mut marks = HashMap::<String, Vec<OffsetAndId>>::new();

    for kv in dict.iter() {
        let Ok(kv) = kv else { continue };
        let Ok(hash) = kv.0.as_data_slice().load_biguint(256) else {
            continue;
        };

        let mut hash = format!("{hash:x}").to_uppercase();
        if hash.len() < 64 {
            hash = "0".repeat(64 - hash.len()) + hash.as_str();
        }

        let mut slice = kv.1;
        let is_normal = slice.load_bit().unwrap_or(false);

        let final_hash = if is_normal {
            hash.clone()
        } else if let Some((hash, _)) = real_code_hashes.get(&hash) {
            hash.clone()
        } else {
            hash.clone() // TODO: or return?
        };

        let dict_inner = Dict::<u32, CellSlice<'_>>::load_from(&mut slice)?;

        for kv in dict_inner.iter() {
            let Ok(mut kv) = kv else { continue };
            let debug_id = kv.0;
            let mut ref_ = kv.1.load_reference()?.as_slice()?;
            let dict_marks_inner =
                RawDict::<10>::load_from_root_ext(&mut ref_, Cell::empty_context())?;

            for kv in dict_marks_inner.iter() {
                let Ok(kv) = kv else { continue };

                #[allow(clippy::cast_possible_truncation)] // always safe, we load only 10 bits
                let offset = kv.0.as_data_slice().load_uint(10)? as u16;

                let adjusted_offset = offset
                    + (if is_normal {
                        0u16
                    } else {
                        real_code_hashes.get(&hash).map_or(0, |r| r.1)
                    });

                let old_value = marks.get_mut(&final_hash);
                if let Some(old_value) = old_value {
                    old_value.push(OffsetAndId(
                        adjusted_offset,
                        i32::try_from(debug_id).unwrap_or(0),
                    ));
                } else {
                    marks.insert(
                        final_hash.clone(),
                        vec![OffsetAndId(
                            adjusted_offset,
                            i32::try_from(debug_id).unwrap_or(0),
                        )],
                    );
                }
            }
        }
    }

    Ok(marks)
}
