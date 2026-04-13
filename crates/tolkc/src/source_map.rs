// Parsing of the source map JSON produced by the Tolk compiler.
// The JSON contains type declarations, function metadata, and debug marks
// that map IR variables and stack positions back to the original Tolk source.

use crate::types_kernel::Ty;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
// ---------------------------------------------------------------------------
// Top-level structure
// ---------------------------------------------------------------------------

type BigintAsString = String;

#[derive(Clone, Serialize, Debug, Default)]
pub struct SourceMap {
    files: Vec<SrcFileInfo>,
    declarations: Vec<Declaration>,
    unique_ty: Vec<UniqueTy>,
    functions: Vec<FunctionInfo>,
    debug_marks: Vec<DebugMark>,

    #[serde(skip)]
    structs: HashMap<String, AbiStruct>,
    #[serde(skip)]
    aliases: HashMap<String, AbiAlias>,
    #[serde(skip)]
    enums: HashMap<String, AbiEnum>,
}

impl SourceMap {
    pub fn from_json_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(path)?;
        Self::from_json_str(&text)
    }

    pub fn from_json_str(json: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(json)?)
    }

    fn index_declarations(&mut self) {
        self.structs.clear();
        self.aliases.clear();
        self.enums.clear();

        for decl in &self.declarations {
            match decl {
                Declaration::Struct(s) => {
                    self.structs.insert(s.name.clone(), s.clone());
                }
                Declaration::Alias(a) => {
                    self.aliases.insert(a.name.clone(), a.clone());
                }
                Declaration::Enum(e) => {
                    self.enums.insert(e.name.clone(), e.clone());
                }
            }
        }
    }

    #[must_use]
    pub fn get_function_by_idx(&self, f_idx: usize) -> Option<&FunctionInfo> {
        self.functions.get(f_idx)
    }

    #[must_use]
    pub fn get_function_name_by_idx(&self, f_idx: usize) -> String {
        if let Some(f_info) = self.functions.get(f_idx) {
            f_info.name.clone()
        } else {
            "unknown-function".to_string()
        }
    }

    #[must_use]
    pub fn innermost_function_at(&self, file_id: usize, line: usize) -> Option<&FunctionInfo> {
        self.functions
            .iter()
            .filter(|function| {
                function.ident_loc.file_id() == file_id
                    && line >= function.ident_loc.start_line()
                    && line <= function.end_loc.end_line()
            })
            .min_by_key(|function| {
                function
                    .end_loc
                    .end_line()
                    .saturating_sub(function.ident_loc.start_line())
            })
    }

    #[must_use]
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    #[must_use]
    pub fn get_struct(&self, name: &str) -> &AbiStruct {
        self.structs
            .get(name)
            .unwrap_or_else(|| panic!("struct `{name}` not found"))
    }

    #[must_use]
    pub fn get_alias(&self, name: &str) -> &AbiAlias {
        self.aliases
            .get(name)
            .unwrap_or_else(|| panic!("alias `{name}` not found"))
    }

    #[must_use]
    pub fn get_enum(&self, name: &str) -> &AbiEnum {
        self.enums
            .get(name)
            .unwrap_or_else(|| panic!("enum `{name}` not found"))
    }

    #[must_use]
    pub fn resolve_file_name(&self, file_id: usize) -> &str {
        for f in &self.files {
            if f.file_id == file_id {
                return f.file_name.rsplit('/').next().unwrap_or(&f.file_name);
            }
        }
        "unknown-file"
    }

    #[must_use]
    pub fn resolve_file_full_path(&self, file_id: usize) -> Option<&str> {
        self.files
            .iter()
            .find(|f| f.file_id == file_id)
            .map(|f| f.file_name.as_str())
    }

    #[must_use]
    pub fn path_to_file_id(&self, path: &str) -> Option<usize> {
        let normalized_path = Self::normalize_path(path);

        if let Some(file) = self
            .files
            .iter()
            .find(|file| Self::normalize_path(&file.file_name) == normalized_path)
        {
            return Some(file.file_id);
        }

        None
    }

    #[must_use]
    pub fn resolve_ty(&self, ty_idx: usize) -> Option<&Ty> {
        self.unique_ty
            .iter()
            .find(|u| u.ty_idx == ty_idx)
            .map(|u| &u.ty)
    }

    /// Collect all source lines that have a stoppable debug mark (LOC,
    /// inlined `ENTER_FUN`, or `LEAVE_FUN`) for a given file, sorted and deduped.
    #[must_use]
    pub fn stoppable_lines_for_file(&self, file_id: usize) -> Vec<usize> {
        let mut lines: Vec<usize> = self
            .debug_marks
            .iter()
            .filter_map(|mark| match mark {
                DebugMark::Loc { range, .. } if range.file_id() == file_id => {
                    Some(range.start_line())
                }
                DebugMark::EnterFun {
                    range,
                    is_inlined: true,
                    ..
                } if range.file_id() == file_id => Some(range.start_line()),
                DebugMark::LeaveFun { range, .. } if range.file_id() == file_id => {
                    Some(range.start_line())
                }
                _ => None,
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    #[must_use]
    pub const fn debug_marks_count(&self) -> usize {
        self.debug_marks.len()
    }

    #[must_use]
    pub fn get_debug_mark(&self, mark_id: usize) -> &DebugMark {
        &self.debug_marks[mark_id]
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.debug_marks.is_empty()
    }

    fn normalize_path(path: &str) -> String {
        // DAP clients may send `file:///...` URIs, while the source map stores
        // plain paths. We also normalize Windows separators to `/`.
        let mut normalized = path
            .trim_start_matches("file://")
            .trim_start_matches("file:")
            .replace('\\', "/");

        if normalized.starts_with('/') {
            let bytes = normalized.as_bytes();
            // `file:///C:/...` becomes `/C:/...` after stripping the URI scheme.
            // Drop that extra leading slash so it matches `C:/...` from source maps.
            if bytes.len() >= 3
                && bytes[0] == b'/'
                && bytes[1].is_ascii_alphabetic()
                && bytes[2] == b':'
            {
                normalized.remove(0);
            }
        }

        normalized
    }
}

#[derive(Deserialize)]
struct SourceMapDe {
    files: Vec<SrcFileInfo>,
    declarations: Vec<Declaration>,
    unique_ty: Vec<UniqueTy>,
    functions: Vec<FunctionInfo>,
    debug_marks: Vec<DebugMark>,
}

impl<'de> Deserialize<'de> for SourceMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = SourceMapDe::deserialize(deserializer)?;
        let mut sm = SourceMap {
            files: raw.files,
            declarations: raw.declarations,
            unique_ty: raw.unique_ty,
            functions: raw.functions,
            debug_marks: raw.debug_marks,
            structs: HashMap::new(),
            aliases: HashMap::new(),
            enums: HashMap::new(),
        };
        sm.index_declarations();
        Ok(sm)
    }
}

// ---------------------------------------------------------------------------
// Source location: [file_id, start_line, start_col, end_line, end_col]
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SrcRange(pub Vec<usize>);

impl SrcRange {
    #[must_use]
    pub fn file_id(&self) -> usize {
        self.0[0]
    }
    #[must_use]
    pub fn start_line(&self) -> usize {
        self.0[1]
    }
    #[must_use]
    pub fn start_col(&self) -> usize {
        self.0[2]
    }
    #[must_use]
    pub fn end_line(&self) -> usize {
        self.0[3]
    }
    #[must_use]
    pub fn end_col(&self) -> usize {
        self.0[4]
    }
}

impl fmt::Display for SrcRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line(),
            self.start_col(),
            self.end_line(),
            self.end_col()
        )
    }
}

// ---------------------------------------------------------------------------
// Source files
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SrcFileInfo {
    pub file_id: usize,
    pub file_name: String,
    pub size_chars: u64,
}

// ---------------------------------------------------------------------------
// Declarations (structs, aliases, enums)
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AbiStruct {
    pub name: String,
    pub ident_loc: SrcRange,
    #[serde(default)]
    pub type_params: Option<Vec<String>>,
    #[serde(default)]
    pub prefix: Option<PrefixInfo>,
    pub fields: Vec<FieldInfo>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AbiAlias {
    pub name: String,
    pub ident_loc: SrcRange,
    pub target_ty: Ty,
    #[serde(default)]
    pub type_params: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AbiEnum {
    pub name: String,
    pub ident_loc: SrcRange,
    pub encoded_as: Ty,
    pub members: Vec<EnumMemberInfo>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Declaration {
    #[serde(rename = "struct")]
    Struct(AbiStruct),
    #[serde(rename = "alias")]
    Alias(AbiAlias),
    #[serde(rename = "enum")]
    Enum(AbiEnum),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PrefixInfo {
    pub prefix_str: String,
    pub prefix_len: i32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EnumMemberInfo {
    pub name: String,
    pub value: BigintAsString,
}

// ---------------------------------------------------------------------------
// Unique type table (ty_idx -> Ty)
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UniqueTy {
    pub ty_idx: usize,
    pub ty: Ty,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FunctionInfo {
    pub f_idx: usize,
    pub name: String,
    pub return_ty_idx: usize,
    pub num_params: usize,
    pub ident_loc: SrcRange,
    pub end_loc: SrcRange,
}

// ---------------------------------------------------------------------------
// Debug marks — the core of source mapping.
// Each mark is emitted by the Tolk compiler at a specific point in the
// generated Fift code. The Fift assembler records the bytecode position
// (cell hash + bit offset) for each mark. During replay, we use these
// marks to reconstruct stack contents, call frames, and source locations.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum DebugMark {
    #[serde(rename = "loc")]
    Loc { mark_id: usize, range: SrcRange },
    #[serde(rename = "stack")]
    Stack { mark_id: usize, stack: Vec<usize> },
    #[serde(rename = "enter_fun")]
    EnterFun {
        mark_id: usize,
        f_idx: usize,
        is_inlined: bool,
        is_builtin: bool,
        range: SrcRange,
        ir_import: Vec<usize>,
    },
    #[serde(rename = "leave_fun")]
    LeaveFun {
        mark_id: usize,
        f_idx: usize,
        ir_return: Vec<usize>,
        range: SrcRange,
    },
    #[serde(rename = "var")]
    Var {
        mark_id: usize,
        var_name: String,
        is_parameter: bool,
        ty_idx: usize,
        ir_slots: Vec<usize>,
        is_lazy: Option<bool>,
    },
    #[serde(rename = "scope_start")]
    ScopeStart { mark_id: usize, range: SrcRange },
    #[serde(rename = "scope_end")]
    ScopeEnd { mark_id: usize },
    #[serde(rename = "smart_cast")]
    SmartCast {
        mark_id: usize,
        var_name: String,
        ty_idx: usize,
        ir_slots: Vec<usize>,
    },
    #[serde(rename = "set_glob")]
    SetGlob {
        mark_id: usize,
        glob_name: String,
        ty_idx: usize,
        ir_slots: Vec<usize>,
    },
}

impl fmt::Display for DebugMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugMark::Loc { mark_id, range } => {
                write!(f, "#{mark_id} LOC {range}")
            }
            DebugMark::Stack { mark_id, stack } => {
                write!(f, "#{mark_id} STACK [")?;
                for (i, ir) in stack.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "'{ir}")?;
                }
                write!(f, "]")
            }
            DebugMark::EnterFun {
                mark_id,
                f_idx,
                is_inlined,
                ir_import,
                ..
            } => {
                let inline_tag = if *is_inlined { " (inlined)" } else { "" };
                write!(
                    f,
                    "#{mark_id} ENTER {f_idx}{inline_tag}, import {ir_import:?}"
                )
            }
            DebugMark::LeaveFun {
                mark_id,
                f_idx,
                ir_return,
                ..
            } => {
                write!(f, "#{mark_id} LEAVE {f_idx}, return {ir_return:?}")
            }
            DebugMark::Var {
                mark_id,
                var_name,
                is_parameter,
                ir_slots,
                ..
            } => {
                let param_tag = if *is_parameter { "param" } else { "local" };
                write!(
                    f,
                    "#{mark_id} VAR {param_tag} {var_name}, slots {ir_slots:?}"
                )
            }
            DebugMark::ScopeStart { mark_id, range } => {
                write!(f, "#{mark_id} SCOPE_START {range}")
            }
            DebugMark::ScopeEnd { mark_id } => {
                write!(f, "#{mark_id} SCOPE_END")
            }
            DebugMark::SmartCast {
                mark_id,
                var_name,
                ty_idx,
                ir_slots,
            } => {
                write!(
                    f,
                    "#{mark_id} SMART_CAST {var_name} -> ty{ty_idx}, slots {ir_slots:?}"
                )
            }
            DebugMark::SetGlob {
                mark_id,
                glob_name,
                ty_idx,
                ir_slots,
            } => {
                write!(
                    f,
                    "#{mark_id} SET_GLOB {glob_name} ty{ty_idx}, slots {ir_slots:?}"
                )
            }
        }
    }
}
