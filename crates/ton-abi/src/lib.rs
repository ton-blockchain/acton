pub mod abi_serde;
pub mod compiler_abi_serde;

use num_bigint::BigInt;
use path_absolutize::Absolutize;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolk_syntax::SourceFile;

fn resolve_mapped_path<'a>(
    import_path: &'a str,
    mappings: &Option<BTreeMap<String, String>>,
) -> Cow<'a, str> {
    if import_path.starts_with("@stdlib/") || import_path.starts_with("@fiftlib/") {
        return Cow::Borrowed(import_path);
    }

    if let Some(rest) = import_path.strip_prefix('@') {
        let (prefix_without_at, path) = rest.split_once('/').unwrap_or((rest, ""));
        let prefix_with_at = &import_path[..=prefix_without_at.len()];

        if let Some(mappings) = mappings {
            // Try both with and without @ prefix in the mappings keys
            let mapping = mappings
                .get(prefix_without_at)
                .or_else(|| mappings.get(prefix_with_at));

            if let Some(mapping) = mapping {
                let mapped_path =
                    add_tolk_extension_if_needed_to_path(Path::new(mapping).join(path));
                let Ok(abs_path) = mapped_path.absolutize() else {
                    return Cow::Borrowed(import_path);
                };
                return Cow::Owned(abs_path.to_string_lossy().to_string());
            }
        }
    }

    Cow::Borrowed(import_path)
}

const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pos {
    pub row: usize,
    pub column: usize,
    pub uri: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub type_info: TypeInfo,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BaseTypeInfo {
    Unserializable,
    Int { width: usize },
    UInt { width: usize },
    Coins,
    Bool,
    Address,
    AnyAddress,
    RemainingBitsAndRefs,
    Bits { width: usize },
    Bytes { width: usize },
    Cell { inner: Option<Box<TypeInfo>> },
    VarInt16,
    VarInt32,
    VarUInt16,
    VarUInt32,
    Nullable { inner: Box<TypeInfo> },
    Struct { name: String },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeInfo {
    pub base: BaseTypeInfo,
    pub human_readable: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeAbi {
    pub name: String,
    pub opcode: Option<u32>,
    pub opcode_width: Option<usize>,
    pub fields: Vec<Field>,
    pub pos: Pos,
}

impl TypeAbi {
    #[must_use]
    pub fn is_from_acton_lib(&self) -> bool {
        // TODO: remove lib/
        self.pos.uri.contains(".acton/") || self.pos.uri.contains("lib/")
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMethod {
    pub name: String,
    pub id: u32,
    pub pos: Pos,
    pub return_type: TypeInfo,
    pub parameters: Vec<Field>,
}

impl GetMethod {
    #[must_use]
    pub fn is_from_acton_lib(&self) -> bool {
        // TODO: remove lib/
        self.pos.uri.contains(".acton/") || self.pos.uri.contains("lib/")
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitCodeInfo {
    pub constant_name: String,
    pub value: i32,
    pub usage_positions: Vec<Pos>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPoint {
    pub pos: Option<Pos>,
}

#[derive(Debug, Clone, Eq, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractAbi {
    pub name: String,
    pub entry_point: Option<EntryPoint>,
    pub external_entry_point: Option<EntryPoint>,
    pub storage: Option<TypeAbi>,
    pub get_methods: Vec<GetMethod>,
    pub messages: Vec<TypeAbi>,
    pub types: Vec<TypeAbi>,
    pub exit_codes: Vec<ExitCodeInfo>,
}

impl ContractAbi {
    #[must_use]
    pub fn find_any_type(&self, name: &String) -> Option<TypeAbi> {
        self.types.iter().find(|typ| typ.name == *name).cloned()
    }

    #[must_use]
    pub fn find_type_by_opcode(&self, id: u32) -> Option<TypeAbi> {
        self.types
            .iter()
            .filter(|typ| !typ.is_from_acton_lib())
            .find(|typ| typ.opcode == Some(id))
            .cloned()
    }

    #[must_use]
    pub fn find_get_method_by_id(&self, id: &BigInt) -> Option<GetMethod> {
        self.get_methods
            .iter()
            .filter(|typ| !typ.is_from_acton_lib())
            .find(|typ| &BigInt::from(typ.id) == id)
            .cloned()
    }

    #[must_use]
    pub fn storages(&self) -> Vec<&TypeAbi> {
        self.types
            .iter()
            .filter(|t| t.name.ends_with("Storage"))
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Eq, PartialEq)]
struct AbiInfo {
    get_methods: Vec<GetMethod>,
    messages: Vec<TypeAbi>,
    types: Vec<TypeAbi>,
    storage: Option<TypeAbi>,
    entry_point: Option<EntryPoint>,
    external_entry_point: Option<EntryPoint>,
    exit_codes: Vec<ExitCodeInfo>,
}

#[derive(Debug)]
struct FileInfo {
    path: String,
    content: Arc<str>,
    tree: tree_sitter::Tree,
}

#[derive(Debug, Default)]
pub struct ContractAbiParseCache {
    files: FxHashMap<String, SourceFile>,
}

impl ContractAbiParseCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn get_file_dependencies(
    file_path: &str,
    include_itself: bool,
    mappings: &Option<BTreeMap<String, String>>,
) -> anyhow::Result<Vec<String>> {
    if file_path.ends_with(".boc") {
        if include_itself {
            return Ok(vec![file_path.to_owned()]);
        }

        return Ok(vec![]);
    }

    let mut dependencies = collect_file_dependency_paths_from_imports(file_path, mappings)?;

    if include_itself {
        dependencies.push(file_path.to_string());
    }

    Ok(dependencies)
}

fn collect_file_dependency_paths_from_imports(
    file_path: &str,
    mappings: &Option<BTreeMap<String, String>>,
) -> anyhow::Result<Vec<String>> {
    let mut processed = HashSet::with_capacity(4);
    collect_file_dependency_paths_from_imports_recursive(
        file_path.to_owned(),
        mappings,
        &mut processed,
    );
    processed.remove(file_path);

    let mut dependencies: Vec<String> = processed.into_iter().collect();
    dependencies.sort_unstable();
    Ok(dependencies)
}

fn collect_file_dependency_paths_from_imports_recursive(
    file_path: String,
    mappings: &Option<BTreeMap<String, String>>,
    processed: &mut HashSet<String>,
) {
    if processed.contains(file_path.as_str()) {
        return;
    }

    let base_path = match Path::new(&file_path).parent() {
        Some(path) => path.to_path_buf(),
        None => return,
    };

    let file = match fs::File::open(&file_path) {
        Ok(file) => file,
        Err(_) => return,
    };
    processed.insert(file_path);

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        let line_trimmed = line.trim_start();
        if line_trimmed.is_empty() {
            continue;
        }

        let Some(import_path) = parse_import_path_line(&line) else {
            if line_trimmed.starts_with("fun ") || line_trimmed.starts_with("struct ") {
                // start of definitions
                break;
            }
            continue;
        };

        let import_path = resolve_mapped_path(import_path, mappings);
        let Some(resolved_path) =
            resolve_import_path_from_base_path(&base_path, import_path.as_ref())
        else {
            continue;
        };

        collect_file_dependency_paths_from_imports_recursive(resolved_path, mappings, processed);
    }
}

#[cfg(test)]
fn collect_import_paths_only(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(parse_import_path_line)
        .map(ToString::to_string)
        .collect()
}

fn parse_import_path_line(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let rest = line.strip_prefix("import")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end_quote = rest.find('"')?;
    let import_path = &rest[..end_quote];
    (!import_path.is_empty()).then_some(import_path)
}

#[must_use]
pub fn contract_abi(
    content: Arc<str>,
    file_path: &str,
    mappings: &Option<BTreeMap<String, String>>,
) -> ContractAbi {
    let file = tolk_syntax::parse(&content);
    contract_abi_with_file(content, file_path, &file, mappings, None)
}

#[must_use]
pub fn contract_abi_with_file(
    content: Arc<str>,
    file_path: &str,
    file: &anyhow::Result<SourceFile>,
    mappings: &Option<BTreeMap<String, String>>,
    cache: Option<&mut ContractAbiParseCache>,
) -> ContractAbi {
    let contract_name = get_contract_name_from_file_path(file_path);

    let Ok(file) = file else {
        return ContractAbi::default();
    };

    let mut local_cache = ContractAbiParseCache::default();
    let cache = cache.unwrap_or(&mut local_cache);

    let files = collect_imported_files(file, content, file_path, mappings, cache);

    let mut abi_info = AbiInfo {
        get_methods: Vec::new(),
        messages: Vec::new(),
        types: Vec::new(),
        storage: None,
        entry_point: None,
        external_entry_point: None,
        exit_codes: Vec::new(),
    };

    for file_info in files {
        let file_abi = collect_abi_info(
            &file_info.tree.root_node(),
            &file_info.content,
            &file_info.path,
        );
        merge_abi_info(&mut abi_info, file_abi);
    }

    ContractAbi {
        name: contract_name,
        entry_point: abi_info.entry_point,
        external_entry_point: abi_info.external_entry_point,
        storage: abi_info.storage,
        get_methods: abi_info.get_methods,
        messages: abi_info.messages,
        types: abi_info.types,
        exit_codes: abi_info.exit_codes,
    }
}

#[must_use]
pub fn extract_handled_messages(
    content: Arc<str>,
    file_path: &str,
    mappings: &Option<BTreeMap<String, String>>,
) -> Vec<String> {
    let Ok(file) = tolk_syntax::parse(&content) else {
        return Vec::new();
    };

    let mut cache = ContractAbiParseCache::default();
    let files = collect_imported_files(&file, content, file_path, mappings, &mut cache);

    let mut handled_messages = Vec::new();

    for file_info in files {
        let messages = extract_messages_from_match(&file_info.tree.root_node(), &file_info.content);
        handled_messages.extend(messages);
    }

    handled_messages
}

fn extract_messages_from_match(node: &tree_sitter::Node<'_>, content: &str) -> Vec<String> {
    let mut messages = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declaration" {
            let Some(name_node) = child.child_by_field_name("name") else {
                continue;
            };

            let func_name = name_node
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();

            if func_name == "onInternalMessage"
                && let Some(body) = child.child_by_field_name("body")
            {
                messages.extend(find_match_patterns(&body, content));
            }
        }
    }

    messages
}

fn find_match_patterns(node: &tree_sitter::Node<'_>, content: &str) -> Vec<String> {
    let mut patterns = Vec::new();

    if node.kind() == "match_expression"
        && let Some(body_node) = node.child_by_field_name("body")
        && body_node.kind() == "match_body"
    {
        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "match_arm"
                && let Some(pattern_type_node) = child.child_by_field_name("pattern_type")
            {
                let pattern_text = pattern_type_node
                    .utf8_text(content.as_bytes())
                    .unwrap_or("")
                    .to_string();

                if !pattern_text.is_empty() {
                    patterns.push(pattern_text);
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        patterns.extend(find_match_patterns(&child, content));
    }

    patterns
}

fn collect_imported_files(
    file: &SourceFile,
    content: Arc<str>,
    file_path: &str,
    mappings: &Option<BTreeMap<String, String>>,
    cache: &mut ContractAbiParseCache,
) -> Vec<FileInfo> {
    let mut files = Vec::with_capacity(4);
    let mut processed = HashSet::with_capacity(4);

    files.push(FileInfo {
        path: file_path.to_string(),
        content: content.clone(),
        tree: file.tree.clone(),
    });
    processed.insert(file_path.to_string());
    cache
        .files
        .entry(file_path.to_string())
        .or_insert_with(|| file.clone());

    collect_imported_files_recursive(
        file,
        content,
        file_path,
        &mut files,
        &mut processed,
        mappings,
        cache,
    );

    files
}

fn collect_imported_files_recursive(
    file: &SourceFile,
    content: Arc<str>,
    file_path: &str,
    files: &mut Vec<FileInfo>,
    processed: &mut HashSet<String>,
    mappings: &Option<BTreeMap<String, String>>,
    cache: &mut ContractAbiParseCache,
) {
    for import in file.imports() {
        let Some(path_node) = import.path() else {
            continue;
        };

        let import_path1 = path_node.content(content.as_ref());
        let import_path = resolve_mapped_path(import_path1, mappings);

        let resolved_path = resolve_import_path(file_path, import_path.as_ref());
        let Some(resolved) = resolved_path else {
            continue;
        };

        if processed.contains(&resolved) {
            // recursive dependency, already processed
            continue;
        }

        let parsed_file = if let Some(cached) = cache.files.get(&resolved) {
            cached.clone()
        } else {
            let Ok(import_content) = fs::read_to_string(&resolved) else {
                continue;
            };
            let import_content: Arc<str> = import_content.into();

            let Ok(parsed_file) = tolk_syntax::parse(&import_content) else {
                continue;
            };
            cache.files.insert(resolved.clone(), parsed_file.clone());
            parsed_file
        };

        collect_imported_files_recursive(
            &parsed_file,
            parsed_file.source.clone(),
            &resolved,
            files,
            processed,
            mappings,
            cache,
        );

        files.push(FileInfo {
            path: resolved.clone(),
            content: parsed_file.source.clone(),
            tree: parsed_file.tree,
        });
        processed.insert(resolved);
    }
}

fn resolve_import_path(base_file: &str, import_path: &str) -> Option<String> {
    let base_path = Path::new(base_file).parent()?;
    resolve_import_path_from_base_path(base_path, import_path)
}

fn resolve_import_path_from_base_path(base_path: &Path, import_path: &str) -> Option<String> {
    if Path::new(import_path).is_absolute() {
        // path can be absolute after mappings
        return Some(import_path.to_owned());
    }

    let import_path = add_tolk_extension_if_needed(import_path.to_string());

    if import_path.starts_with("./") || import_path.starts_with("../") {
        let relative_path = base_path.join(import_path);
        return Some(relative_path.to_string_lossy().to_string());
    }

    let relative_path = base_path.join(&import_path);
    if relative_path.exists() {
        return Some(relative_path.to_string_lossy().to_string());
    }

    let with_ext = format!("{import_path}.tolk");
    let path_with_ext = base_path.join(with_ext);
    if path_with_ext.exists() {
        Some(path_with_ext.to_string_lossy().to_string())
    } else {
        None
    }
}

fn add_tolk_extension_if_needed(path: String) -> String {
    if path.ends_with(".tolk") {
        return path;
    }
    format!("{path}.tolk")
}

fn add_tolk_extension_if_needed_to_path(path: PathBuf) -> PathBuf {
    if path.extension() == Some(OsStr::new("tolk")) {
        return path;
    }
    path.with_extension("tolk")
}

fn merge_abi_info(target: &mut AbiInfo, source: AbiInfo) {
    target.get_methods.extend(source.get_methods);
    target.messages.extend(source.messages);
    target.types.extend(source.types);
    target.exit_codes.extend(source.exit_codes);

    if target.storage.is_none() && source.storage.is_some() {
        target.storage = source.storage;
    }
    if target.entry_point.is_none() && source.entry_point.is_some() {
        target.entry_point = source.entry_point;
    }
    if target.external_entry_point.is_none() && source.external_entry_point.is_some() {
        target.external_entry_point = source.external_entry_point;
    }
}

fn collect_abi_info(node: &tree_sitter::Node<'_>, content: &str, file_path: &str) -> AbiInfo {
    let mut info = AbiInfo {
        get_methods: Vec::new(),
        messages: Vec::new(),
        types: Vec::new(),
        storage: None,
        entry_point: None,
        external_entry_point: None,
        exit_codes: Vec::new(),
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_declaration" {
            let Some(name_node) = child.child_by_field_name("name") else {
                continue;
            };

            let func_name = name_node
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();

            if func_name == "onInternalMessage" {
                info.entry_point = Some(EntryPoint {
                    pos: Some(Pos {
                        row: name_node.start_position().row,
                        column: name_node.start_position().column,
                        uri: file_path.to_string(),
                    }),
                });
            } else if func_name == "onExternalMessage" {
                info.external_entry_point = Some(EntryPoint {
                    pos: Some(Pos {
                        row: name_node.start_position().row,
                        column: name_node.start_position().column,
                        uri: file_path.to_string(),
                    }),
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "get_method_declaration" {
            let Some(method) = extract_get_method(&child, content, file_path) else {
                continue;
            };
            info.get_methods.push(method);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "struct_declaration" {
            let Some(struct_abi) = extract_struct_abi(&child, content, file_path) else {
                continue;
            };

            if struct_abi.name == "Storage" {
                info.storage = Some(struct_abi.clone());
            }

            if struct_abi.opcode.is_some() {
                info.messages.push(struct_abi.clone());
            }
            info.types.push(struct_abi);
        }
    }

    info
}

fn extract_get_method(
    func_node: &tree_sitter::Node<'_>,
    content: &str,
    file_path: &str,
) -> Option<GetMethod> {
    let name_node = func_node.child_by_field_name("name")?;
    let func_name = name_node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .to_string();

    let explicit_id = get_explicit_method_id(func_node, content);
    let method_id = match explicit_id {
        Some(id) => id,
        None => (u32::from(CRC16.checksum(func_name.as_bytes())) & 0xFFFF) | 0x10000,
    };

    let pos = Pos {
        row: name_node.start_position().row,
        column: name_node.start_position().column,
        uri: file_path.to_string(),
    };

    let parameters = extract_parameters(func_node, content, file_path);

    let return_type = if let Some(return_type_node) = func_node.child_by_field_name("return_type") {
        extract_type_info(&return_type_node, content)
    } else {
        TypeInfo {
            base: BaseTypeInfo::Unserializable,
            human_readable: "void".to_string(),
        }
    };

    Some(GetMethod {
        name: func_name,
        id: method_id,
        pos,
        return_type,
        parameters,
    })
}

fn extract_struct_abi(
    struct_node: &tree_sitter::Node<'_>,
    content: &str,
    file_path: &str,
) -> Option<TypeAbi> {
    let name_node = struct_node.child_by_field_name("name")?;
    let struct_name = name_node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .to_string();

    let mut fields = Vec::new();

    if let Some(body_node) = struct_node.child_by_field_name("body") {
        let mut cursor = body_node.walk();
        for child in body_node
            .children(&mut cursor)
            .filter(|child| child.kind() == "struct_field_declaration")
        {
            if let Some(field) = extract_field(&child, content, file_path) {
                fields.push(field);
            }
        }
    }

    let mut opcode = None;
    let mut opcode_width = None;

    if let Some(prefix_node) = struct_node.child_by_field_name("pack_prefix") {
        let prefix_text = prefix_node
            .utf8_text(content.as_bytes())
            .unwrap_or("")
            .to_string();

        // Clean the number by removing underscores
        let clean_text = prefix_text.replace('_', "");

        let (prefix_val, radix) = if let Some(stripped) = clean_text.strip_prefix("0x") {
            (u32::from_str_radix(stripped, 16), 16)
        } else if let Some(stripped) = clean_text.strip_prefix("0b") {
            (u32::from_str_radix(stripped, 2), 2)
        } else {
            (clean_text.parse::<u32>(), 10)
        };

        if let Ok(val) = prefix_val {
            opcode = Some(val);
            opcode_width = match radix {
                16 => Some((clean_text.len() - 2) * 4),
                2 => Some(clean_text.len() - 2),
                _ => Some(format!("{val:b}").len()),
            };
        }
    }

    let pos = Pos {
        row: name_node.start_position().row,
        column: name_node.start_position().column,
        uri: file_path.to_string(),
    };

    Some(TypeAbi {
        name: struct_name,
        opcode,
        opcode_width,
        fields,
        pos,
    })
}

fn extract_field(
    field_node: &tree_sitter::Node<'_>,
    content: &str,
    _file_path: &str,
) -> Option<Field> {
    let name_node = field_node.child_by_field_name("name")?;
    let type_node = field_node.child_by_field_name("type")?;

    let field_name = name_node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .to_string();

    let type_info = extract_type_info(&type_node, content);

    Some(Field {
        name: field_name,
        type_info,
    })
}

fn extract_type_info(type_node: &tree_sitter::Node<'_>, content: &str) -> TypeInfo {
    let type_name = type_node
        .utf8_text(content.as_bytes())
        .unwrap_or("")
        .to_string();

    if type_node.kind() == "type_instantiatedTs"
        && let Some(name_node) = type_node.child_by_field_name("name")
    {
        let name = name_node
            .utf8_text(content.as_bytes())
            .unwrap_or("")
            .to_string();

        if name == "Cell"
            && let Some(args_node) = type_node.child_by_field_name("arguments")
            && let Some(inner_type_node) = args_node.child_by_field_name("types")
        {
            let inner_type_info = extract_type_info(&inner_type_node, content);
            return TypeInfo {
                base: BaseTypeInfo::Cell {
                    inner: Some(Box::new(inner_type_info)),
                },
                human_readable: type_name,
            };
        }

        if name == "map" {
            // for now treat map<K, V> as cell?
            return TypeInfo {
                base: BaseTypeInfo::Nullable {
                    inner: Box::new(TypeInfo {
                        base: BaseTypeInfo::Cell { inner: None },
                        human_readable: "dict".to_owned(),
                    }),
                },
                human_readable: type_name,
            };
        }
    }

    if type_node.kind() == "tensor_type"
        || type_node.kind() == "tuple_type"
        || type_node.kind() == "fun_callable_type"
        || type_node.kind() == "union_type"
        || type_node.kind() == "null_literal"
    {
        return TypeInfo {
            base: BaseTypeInfo::Unserializable,
            human_readable: type_name,
        };
    }

    if type_node.kind() == "nullable_type" {
        let inner_node = type_node.child_by_field_name("inner");
        if let Some(inner_node) = inner_node {
            let inner = extract_type_info(&inner_node, content);
            return TypeInfo {
                base: BaseTypeInfo::Nullable {
                    inner: Box::new(inner),
                },
                human_readable: type_name,
            };
        }
    }

    let base = match type_name.as_str() {
        "void" | "never" | "null" | "tuple" | "continuation" | "slice" | "builder" | "int" => {
            BaseTypeInfo::Unserializable
        }
        "coins" => BaseTypeInfo::Coins,
        "bool" => BaseTypeInfo::Bool,
        "cell" => BaseTypeInfo::Cell { inner: None },
        "address" => BaseTypeInfo::Address,
        "any_address" => BaseTypeInfo::AnyAddress,
        "dict" => BaseTypeInfo::Nullable {
            inner: Box::new(TypeInfo {
                base: BaseTypeInfo::Cell { inner: None },
                human_readable: "dict".to_owned(),
            }),
        },
        // TODO: real type alias resolving
        "RemainingBitsAndRefs" | "ForwardPayloadRemainder" => BaseTypeInfo::RemainingBitsAndRefs,
        _ if type_name.starts_with("int") && type_name.len() > 3 => {
            let width = type_name[3..].parse::<usize>().unwrap_or(0);
            BaseTypeInfo::Int { width }
        }
        _ if type_name.starts_with("uint") && type_name.len() > 4 => {
            let width = type_name[4..].parse::<usize>().unwrap_or(0);
            BaseTypeInfo::UInt { width }
        }
        _ if type_name.starts_with("varint") && type_name.len() > 6 => {
            let width = type_name[6..].parse::<usize>().unwrap_or(0);
            if width == 16 {
                BaseTypeInfo::VarInt16
            } else if width == 32 {
                BaseTypeInfo::VarInt32
            } else {
                BaseTypeInfo::Unserializable
            }
        }
        _ if type_name.starts_with("varuint") && type_name.len() > 7 => {
            let width = type_name[7..].parse::<usize>().unwrap_or(0);
            if width == 16 {
                BaseTypeInfo::VarUInt16
            } else if width == 32 {
                BaseTypeInfo::VarUInt32
            } else {
                BaseTypeInfo::Unserializable
            }
        }
        _ if type_name.starts_with("bits") && type_name.len() > 4 => {
            let width = type_name[4..].parse::<usize>().unwrap_or(0);
            BaseTypeInfo::Bits { width }
        }
        _ if type_name.starts_with("bytes") && type_name.len() > 5 => {
            let width = type_name[5..].parse::<usize>().unwrap_or(0);
            BaseTypeInfo::Bytes { width }
        }
        &_ => BaseTypeInfo::Unserializable,
    };

    TypeInfo {
        base,
        human_readable: type_name,
    }
}

fn extract_parameters(
    func_node: &tree_sitter::Node<'_>,
    content: &str,
    file_path: &str,
) -> Vec<Field> {
    let mut parameters = Vec::new();

    let Some(params_node) = func_node.child_by_field_name("parameters") else {
        return parameters;
    };

    let mut cursor = params_node.walk();
    for child in params_node
        .children(&mut cursor)
        .filter(|child| child.kind() == "parameter_declaration")
    {
        if let Some(field) = extract_field(&child, content, file_path) {
            parameters.push(field);
        }
    }

    parameters
}

fn get_explicit_method_id(func_node: &tree_sitter::Node<'_>, content: &str) -> Option<u32> {
    let annotations = func_node.child_by_field_name("annotations")?;
    let mut cursor = annotations.walk();

    for child in annotations
        .children(&mut cursor)
        .filter(|child| child.kind() == "annotation")
    {
        let Some(name_node) = child.child_by_field_name("name") else {
            continue;
        };

        let annotation_name = name_node
            .utf8_text(content.as_bytes())
            .unwrap_or("")
            .to_string();

        if annotation_name == "method_id" {
            let Some(args_node) = child.child_by_field_name("arguments") else {
                continue;
            };

            let mut args_cursor = args_node.walk();
            for arg in args_node.children(&mut args_cursor) {
                if arg.kind() == "number_literal" {
                    let value_text = arg.utf8_text(content.as_bytes()).unwrap_or("").to_string();

                    let id = if let Some(stripped) = value_text.strip_prefix("0x") {
                        u32::from_str_radix(stripped, 16).ok()
                    } else {
                        value_text.parse::<u32>().ok()
                    };

                    return id;
                }
            }
        }
    }

    None
}

fn get_contract_name_from_file_path(file_path: &str) -> String {
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");

    file_name.split('.').next().unwrap_or("Unknown").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("ton-abi-{prefix}-{}-{now}", std::process::id()))
    }

    #[test]
    fn test_contract_abi_basic() {
        let code = r"
struct Storage {
    balance: int;
}

get fun get_balance(): int {
    return 0;
}

fun onInternalMessage() {
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert_eq!(abi.name, "test");
        assert!(abi.entry_point.is_some());
        assert!(abi.storage.is_some());
        assert_eq!(abi.get_methods.len(), 1);
        assert_eq!(abi.get_methods[0].name, "get_balance");

        let expected_id = (u32::from(CRC16.checksum(b"get_balance")) & 0xFFFF) | 0x10000;
        assert_eq!(abi.get_methods[0].id, expected_id);
    }

    #[test]
    fn test_contract_abi_explicit_method_id() {
        let code = r"
@method_id(0x12345)
get fun custom_method(): int {
    return 42;
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert_eq!(abi.get_methods.len(), 1);
        assert_eq!(abi.get_methods[0].name, "custom_method");
        assert_eq!(abi.get_methods[0].id, 0x12345);
    }

    #[test]
    fn test_get_method_variants() {
        let code = r"
// Get method with parameters and return type
get fun get_balance(addr: address): int {
    return 0;
}

// Get method without parameters
get fun get_counter(): int {
    return 42;
}

// Get method without return type annotation
get fun ping() {
    return;
}

// Get method with just 'get' (no 'fun')
get simple_method(): int {
    return 1;
}

// Get method with method_id annotation
@method_id(0x10001)
get fun custom_id(): int {
    return 2;
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert_eq!(abi.get_methods.len(), 5);

        let names: Vec<&str> = abi.get_methods.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"get_balance"));
        assert!(names.contains(&"get_counter"));
        assert!(names.contains(&"ping"));
        assert!(names.contains(&"simple_method"));
        assert!(names.contains(&"custom_id"));

        let custom_id_method = abi
            .get_methods
            .iter()
            .find(|m| m.name == "custom_id")
            .unwrap();
        assert_eq!(custom_id_method.id, 0x10001);
    }

    #[test]
    fn test_struct_variants() {
        let code = r"
// Regular struct
struct User {
    id: int;
    name: string;
}

// Storage struct
struct Storage {
    counter: int;
    owner: address;
}

// Struct with hex pack prefix
struct (0xABCD) MessageData {
    data: cell;
}

// Struct with decimal pack prefix
struct (123) TokenInfo {
    amount: int;
    symbol: string;
}

// Struct with binary pack prefix
struct (0b1010) BinaryData {
    flag: bool;
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert_eq!(abi.types.len(), 5);
        assert_eq!(abi.messages.len(), 3); // Only structs with pack_prefix
        assert!(abi.storage.is_some());

        assert_eq!(abi.storage.as_ref().unwrap().name, "Storage");

        let message_names: Vec<&str> = abi.messages.iter().map(|m| m.name.as_str()).collect();
        assert!(message_names.contains(&"MessageData"));
        assert!(message_names.contains(&"TokenInfo"));
        assert!(message_names.contains(&"BinaryData"));

        let message_data = abi
            .messages
            .iter()
            .find(|m| m.name == "MessageData")
            .unwrap();
        assert_eq!(message_data.opcode, Some(0xABCD));

        let token_info = abi.messages.iter().find(|m| m.name == "TokenInfo").unwrap();
        assert_eq!(token_info.opcode, Some(123));

        let binary_data = abi
            .messages
            .iter()
            .find(|m| m.name == "BinaryData")
            .unwrap();
        assert_eq!(binary_data.opcode, Some(0b1010));
    }

    #[test]
    fn test_entry_points() {
        let code = r"
fun onInternalMessage() {
    // Internal message handler
}

fun onExternalMessage() {
    // External message handler
}

fun regular_function() {
    // Just a regular function
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert!(abi.entry_point.is_some());
        assert!(abi.external_entry_point.is_some());

        assert_eq!(
            abi.entry_point.as_ref().unwrap().pos.as_ref().unwrap().row,
            1
        );
        assert_eq!(
            abi.external_entry_point
                .as_ref()
                .unwrap()
                .pos
                .as_ref()
                .unwrap()
                .row,
            5
        );
    }

    #[test]
    fn test_method_id_formats() {
        let code = r"
// Decimal method ID
@method_id(65537)
get fun decimal_id(): int {
    return 1;
}

// Hex method ID
@method_id(0x10001)
get fun hex_id(): int {
    return 2;
}

// Large method ID
@method_id(0xFFFFFFFF)
get fun large_id(): int {
    return 3;
}
";

        let abi = contract_abi(code.into(), "test.tolk", &None);

        assert_eq!(abi.get_methods.len(), 3);

        let decimal_method = abi
            .get_methods
            .iter()
            .find(|m| m.name == "decimal_id")
            .unwrap();
        assert_eq!(decimal_method.id, 65537);

        let hex_method = abi.get_methods.iter().find(|m| m.name == "hex_id").unwrap();
        assert_eq!(hex_method.id, 0x10001);

        let large_method = abi
            .get_methods
            .iter()
            .find(|m| m.name == "large_id")
            .unwrap();
        assert_eq!(large_method.id, 0xFFFF_FFFF);
    }

    #[test]
    fn test_imports_support() {
        let import_content = r"
struct ImportedStruct {
    value: int;
}

get fun imported_method(): int {
    return 42;
}
";

        let import_path = "test_import.tolk";
        fs::write(import_path, import_content).unwrap();

        let main_content = r#"
import "test_import";

struct MainStruct {
    data: int;
}

get fun main_method(): int {
    return 100;
}
"#;

        let abi = contract_abi(main_content.into(), "main.tolk", &None);

        let _ = fs::remove_file(import_path);

        assert_eq!(abi.types.len(), 2); // MainStruct and ImportedStruct
        assert_eq!(abi.get_methods.len(), 2); // main_method and imported_method

        let type_names: Vec<&str> = abi.types.iter().map(|t| t.name.as_str()).collect();
        assert!(type_names.contains(&"MainStruct"));
        assert!(type_names.contains(&"ImportedStruct"));

        let method_names: Vec<&str> = abi.get_methods.iter().map(|m| m.name.as_str()).collect();
        assert!(method_names.contains(&"main_method"));
        assert!(method_names.contains(&"imported_method"));
    }

    #[test]
    fn test_crc16_consistency() {
        let test_name = "get_balance";
        let crc_value = u32::from(CRC16.checksum(test_name.as_bytes()));
        let method_id = (crc_value & 0xFFFF) | 0x10000;

        assert!(crc_value > 0);
        assert!(method_id >= 0x10000);
        assert_eq!(method_id & 0xFFFF, crc_value & 0xFFFF);
    }

    #[test]
    fn test_collect_import_paths_only_simple_lines() {
        let content = r#"
    import "a"
import "b/c"
"#;

        let imports = collect_import_paths_only(content);
        assert_eq!(imports, vec!["a".to_string(), "b/c".to_string()]);
    }

    #[test]
    fn test_get_file_dependencies_scans_only_imports_and_recurses() {
        let test_dir = unique_test_dir("deps");
        fs::create_dir_all(&test_dir).unwrap();

        let main = test_dir.join("main.tolk");
        let dep_a = test_dir.join("a.tolk");
        let dep_b = test_dir.join("b.tolk");

        fs::write(
            &main,
            r#"
import "a"
// import "ignored"
this is invalid syntax but should not affect import scanning
"#,
        )
        .unwrap();
        fs::write(&dep_a, "import \"./b\"\n").unwrap();
        fs::write(&dep_b, "let x = 1\n").unwrap();

        let main_str = main.to_string_lossy().to_string();
        let deps = get_file_dependencies(&main_str, true, &None).unwrap();

        let deps_canon: HashSet<String> = deps
            .iter()
            .map(|dep| {
                dunce::canonicalize(dep)
                    .unwrap_or_else(|_| PathBuf::from(dep))
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert!(
            deps_canon.contains(
                &dunce::canonicalize(&dep_a)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert!(
            deps_canon.contains(
                &dunce::canonicalize(&dep_b)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert_eq!(deps.last(), Some(&main_str));

        let _ = fs::remove_file(&main);
        let _ = fs::remove_file(&dep_a);
        let _ = fs::remove_file(&dep_b);
        let _ = fs::remove_dir_all(&test_dir);
    }
}
