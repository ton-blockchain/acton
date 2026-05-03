// Parsing of the symbol-types and debug-marks JSON produced by the Tolk compiler.
// Symbol types contain type declarations and function metadata; debug marks map
// IR variables and stack positions back to the original Tolk source.

use crate::abi::ABICustomPackUnpack;
use crate::debug_marks_dict::DebugMarksDict;
use crate::types_kernel::{AliasInstantiation, StructInstantiation, Ty, TyIdx, TyResolver};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use ton_source_map::SourceLocation;
// ---------------------------------------------------------------------------
// Top-level structure
// ---------------------------------------------------------------------------

type BigintAsString = String;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SourceMap {
    files: Vec<SrcFileInfo>,
    unique_types: Vec<Ty>,
    struct_instantiations: Vec<StructInstantiation>,
    alias_instantiations: Vec<AliasInstantiation>,
    declarations: Vec<Declaration>,
    functions: Vec<FunctionInfo>,

    #[serde(default)]
    debug_marks: Vec<DebugMark>,
    #[serde(default, skip_serializing_if = "DebugMarksDict::is_empty")]
    marks_dict: DebugMarksDict,

    #[serde(skip)]
    decl_index: OnceLock<DeclarationIndex>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SymbolTypesJson {
    files: Vec<SrcFileInfo>,
    unique_types: Vec<Ty>,
    struct_instantiations: Vec<StructInstantiation>,
    alias_instantiations: Vec<AliasInstantiation>,
    declarations: Vec<Declaration>,
    functions: Vec<FunctionInfo>,
}

impl SourceMap {
    #[must_use]
    pub fn from_parts(
        symbol_types: SymbolTypesJson,
        debug_marks: Vec<DebugMark>,
        marks_dict: DebugMarksDict,
    ) -> Self {
        SourceMap {
            files: symbol_types.files,
            declarations: symbol_types.declarations,
            unique_types: symbol_types.unique_types,
            struct_instantiations: symbol_types.struct_instantiations,
            alias_instantiations: symbol_types.alias_instantiations,
            functions: symbol_types.functions,
            debug_marks,
            marks_dict,
            decl_index: OnceLock::new(),
        }
    }

    #[must_use]
    pub fn without_debug_info() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn find_source_loc(&self, hash: &str, offset: u16) -> Option<SourceLocation> {
        let marks = self.marks_dict.get(hash)?;
        let target_offset = i32::from(offset);

        let mut approx_loc = None;
        let mut exact_loc = None;

        for &(mark_offset, mark_id) in marks {
            let Some(loc) = self.source_location_for_mark(mark_id as usize) else {
                continue;
            };

            if mark_offset < target_offset {
                approx_loc = Some(loc);
                continue;
            }

            if mark_offset == target_offset {
                exact_loc = Some(loc);
                continue;
            }

            break;
        }

        exact_loc.or(approx_loc)
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
    pub fn find_message_name_by_opcode(&self, opcode: u32) -> Option<&str> {
        self.declarations.iter().find_map(|declaration| {
            let Declaration::Struct(struct_decl) = declaration else {
                return None;
            };

            if struct_decl
                .type_params
                .as_ref()
                .is_some_and(|params| !params.is_empty())
            {
                return None;
            }

            let matches_opcode = struct_decl.prefix.as_ref().is_some_and(|prefix| {
                prefix.prefix_len == 32 && prefix.prefix_num == u64::from(opcode)
            });
            matches_opcode.then_some(struct_decl.name.as_str())
        })
    }

    #[must_use]
    pub fn get_struct(&self, name: &str) -> &AbiStruct {
        let idx = *self
            .declaration_index()
            .structs
            .get(name)
            .unwrap_or_else(|| panic!("struct `{name}` not found"));
        match &self.declarations[idx] {
            Declaration::Struct(s) => s,
            _ => unreachable!("declaration index points to non-struct"),
        }
    }

    #[must_use]
    pub fn get_alias(&self, name: &str) -> &AbiAlias {
        let idx = *self
            .declaration_index()
            .aliases
            .get(name)
            .unwrap_or_else(|| panic!("alias `{name}` not found"));
        match &self.declarations[idx] {
            Declaration::Alias(a) => a,
            _ => unreachable!("declaration index points to non-alias"),
        }
    }

    #[must_use]
    pub fn get_enum(&self, name: &str) -> &AbiEnum {
        let idx = *self
            .declaration_index()
            .enums
            .get(name)
            .unwrap_or_else(|| panic!("enum `{name}` not found"));
        match &self.declarations[idx] {
            Declaration::Enum(e) => e,
            _ => unreachable!("declaration index points to non-enum"),
        }
    }

    #[must_use]
    pub fn struct_fields_of(&self, ty_idx: TyIdx) -> Option<Vec<FieldInfo>> {
        let Ty::StructRef { struct_name, .. } = self.ty_by_idx(ty_idx)? else {
            return None;
        };
        if let Some(inst) = self
            .struct_instantiations
            .iter()
            .find(|inst| inst.ty_idx == ty_idx)
        {
            let fields = &self.get_struct(struct_name).fields;
            if fields.len() != inst.monomorphic_fields_ty_idx.len() {
                return None;
            }
            return Some(
                fields
                    .iter()
                    .zip(&inst.monomorphic_fields_ty_idx)
                    .map(|(field, &field_ty_idx)| FieldInfo {
                        ty_idx: field_ty_idx,
                        ..field.clone()
                    })
                    .collect(),
            );
        }
        Some(self.get_struct(struct_name).fields.clone())
    }

    #[must_use]
    pub fn alias_target_of(&self, ty_idx: TyIdx) -> Option<TyIdx> {
        let Ty::AliasRef { alias_name, .. } = self.ty_by_idx(ty_idx)? else {
            return None;
        };
        self.alias_instantiations
            .iter()
            .find(|inst| inst.ty_idx == ty_idx)
            .map(|inst| inst.monomorphic_target_ty_idx)
            .or_else(|| Some(self.get_alias(alias_name).target_ty_idx))
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
    pub fn ty_by_idx(&self, ty_idx: TyIdx) -> Option<&Ty> {
        self.unique_types.get(ty_idx)
    }

    /// Collect all source lines that have a stoppable debug mark (LOC,
    /// inlined `ENTER_FUN`, or `LEAVE_FUN`) for a given file, sorted and deduped.
    #[must_use]
    pub fn stoppable_lines_for_file(&self, file_id: usize) -> Vec<usize> {
        let mut lines: Vec<usize> = self
            .debug_marks
            .iter()
            .filter_map(|mark| match mark {
                DebugMark::Loc { range, .. } | DebugMark::LeaveFun { range, .. }
                    if range.file_id() == file_id =>
                {
                    Some(range.start_line())
                }
                DebugMark::EnterFun {
                    range,
                    is_inlined: true,
                    ..
                } if range.file_id() == file_id => Some(range.start_line()),
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
    pub fn has_debug_marks(&self) -> bool {
        !self.marks_dict.is_empty()
    }

    #[must_use]
    pub const fn debug_marks_dict(&self) -> &DebugMarksDict {
        &self.marks_dict
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

    fn declaration_index(&self) -> &DeclarationIndex {
        self.decl_index.get_or_init(|| {
            let mut index = DeclarationIndex::default();
            for (idx, decl) in self.declarations.iter().enumerate() {
                match decl {
                    Declaration::Struct(s) => {
                        index.structs.insert(s.name.clone(), idx);
                    }
                    Declaration::Alias(a) => {
                        index.aliases.insert(a.name.clone(), idx);
                    }
                    Declaration::Enum(e) => {
                        index.enums.insert(e.name.clone(), idx);
                    }
                }
            }
            index
        })
    }

    fn source_location_for_mark(&self, mark_id: usize) -> Option<SourceLocation> {
        let (DebugMark::EnterFun {
            is_inlined: true,
            range,
            ..
        }
        | DebugMark::Loc { range, .. }
        | DebugMark::LeaveFun { range, .. }) = self.get_debug_mark(mark_id)
        else {
            return None;
        };

        let file_id = range.file_id();
        let file = self
            .resolve_file_full_path(file_id)
            .unwrap_or_else(|| self.resolve_file_name(file_id))
            .to_owned();
        if file.is_empty() || file.starts_with("@stdlib/") {
            return None;
        }

        Some(SourceLocation {
            file,
            line: range.start_line() as i64,
            column: range.start_col() as i64,
            end_line: range.end_line() as i64,
            end_column: range.end_col() as i64,
            length: 0,
        })
    }
}

impl TyResolver for SourceMap {
    fn ty_by_idx(&self, ty_idx: TyIdx) -> Option<&Ty> {
        SourceMap::ty_by_idx(self, ty_idx)
    }

    fn struct_field_ty_indices(&self, ty_idx: TyIdx) -> Option<Vec<TyIdx>> {
        self.struct_fields_of(ty_idx)
            .map(|fields| fields.into_iter().map(|field| field.ty_idx).collect())
    }

    fn alias_target_ty_idx(&self, ty_idx: TyIdx) -> Option<TyIdx> {
        self.alias_target_of(ty_idx)
    }
}

#[derive(Clone, Debug, Default)]
struct DeclarationIndex {
    structs: HashMap<String, usize>,
    aliases: HashMap<String, usize>,
    enums: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Source location: [file_id, start_line, start_col, end_line, end_col]
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SrcRange(pub Vec<usize>);

impl SrcRange {
    #[must_use]
    pub fn is_undefined(&self) -> bool {
        self.start_line() == 0
    }

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
    pub ty_idx: TyIdx,
    pub ident_loc: SrcRange,
    #[serde(default)]
    pub type_params: Option<Vec<String>>,
    #[serde(default)]
    pub prefix: Option<PrefixInfo>,
    pub fields: Vec<FieldInfo>,
    #[serde(default)]
    pub custom_pack_unpack: Option<ABICustomPackUnpack>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AbiAlias {
    pub name: String,
    pub ty_idx: TyIdx,
    pub ident_loc: SrcRange,
    pub target_ty_idx: TyIdx,
    #[serde(default)]
    pub type_params: Option<Vec<String>>,
    #[serde(default)]
    pub custom_pack_unpack: Option<ABICustomPackUnpack>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AbiEnum {
    pub name: String,
    pub ty_idx: TyIdx,
    pub ident_loc: SrcRange,
    pub encoded_as_ty_idx: TyIdx,
    pub members: Vec<EnumMemberInfo>,
    #[serde(default)]
    pub custom_pack_unpack: Option<ABICustomPackUnpack>,
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
    pub prefix_num: u64,
    pub prefix_len: i32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty_idx: TyIdx,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EnumMemberInfo {
    pub name: String,
    pub value: BigintAsString,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FunctionInfo {
    pub f_idx: usize,
    pub name: String,
    pub return_ty_idx: TyIdx,
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
        ty_idx: TyIdx,
        ir_slots: Vec<usize>,
        #[serde(default)]
        ir_lazy_slice: Option<usize>,
    },
    #[serde(rename = "scope_start")]
    ScopeStart { mark_id: usize, range: SrcRange },
    #[serde(rename = "scope_end")]
    ScopeEnd { mark_id: usize },
    #[serde(rename = "smart_cast")]
    SmartCast {
        mark_id: usize,
        var_name: String,
        ty_idx: TyIdx,
        ir_slots: Vec<usize>,
    },
    #[serde(rename = "set_glob")]
    SetGlob {
        mark_id: usize,
        glob_name: String,
        ty_idx: TyIdx,
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
