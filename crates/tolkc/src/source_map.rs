use serde::Deserialize;
use std::collections::HashMap;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily, CellSlice, Load};
use tycho_types::dict::{Dict, RawDict};

#[derive(Debug, Clone, Deserialize)]
pub struct SourceMap {
    pub high_level: HighLevelSourceMap,
    pub debug_marks: HashMap<String, Vec<(i32, i32)>>,
}

/// Source map data structure for Tolk compiler output
#[derive(Debug, Clone, Deserialize)]
pub struct HighLevelSourceMap {
    pub version: String,
    pub language: Option<String>,
    pub compiler_version: Option<String>,
    pub files: Vec<SourceFile>,
    pub globals: Vec<GlobalVariable>,
    pub locations: Vec<DebugLocation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub is_stdlib: bool,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlobalVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: i64,
    pub column: i64,
    pub end_line: i64,
    pub end_column: i64,
    pub length: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DebugLocation {
    pub idx: i64,
    pub loc: SourceLocation,
    pub variables: Vec<Variable>,
    pub context: EntryContext,
    pub debug: Option<DebugInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    pub is_temporary: Option<bool>,
    pub constant_value: Option<String>,
    pub possible_qualifier_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryContext {
    pub description: EntryContextDescription,
    pub inlining: InliningInfo,
    pub event: Option<String>,
    pub containing_function: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct InliningInfo {
    pub inlined_to_func: Option<String>,
    pub containing_func_inline_mode: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DebugInfo {
    pub opcode: String,
    pub line_str: String,
    pub line_off: String,
}

pub fn parse_marks_dict(code_boc64: &String) -> HashMap<String, Vec<(i32, i32)>> {
    let debug_marks_cell = Boc::decode_base64(&*code_boc64).unwrap();

    let dict = RawDict::<256>::from(Some(debug_marks_cell));
    let mut marks = HashMap::<String, Vec<(i32, i32)>>::new();

    dict.iter().for_each(|kv| {
        let kv = kv.unwrap();
        let hash = kv.0.as_data_slice().load_biguint(256).unwrap();
        let hash = format!("{:x}", hash).to_uppercase();

        let mut slice = kv.1;
        let is_normal = slice.load_bit().unwrap();
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

                let old_value = marks.get_mut(&hash);
                if let Some(old_value) = old_value {
                    old_value.push((offset as i32, debug_id as i32))
                } else {
                    marks.insert(hash.clone(), vec![(offset as i32, debug_id as i32)]);
                }
            });
        });
    });
    marks
}
