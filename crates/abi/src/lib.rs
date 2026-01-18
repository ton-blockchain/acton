pub mod abi_serde;

use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

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
    content: String,
    tree: tree_sitter::Tree,
}

pub fn get_file_dependencies(file_path: &str, include_itself: bool) -> anyhow::Result<Vec<String>> {
    if file_path.ends_with(".boc") {
        if include_itself {
            return Ok(vec![file_path.to_owned()]);
        }

        return Ok(vec![]);
    }

    let content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => anyhow::bail!("Failed to read file '{file_path}': {e}"),
    };

    let tree = match tolk_syntax::parse(&content) {
        Ok(tree) => tree,
        Err(e) => anyhow::bail!("Failed to parse file '{file_path}': {e:?}"),
    };

    let root_node = tree.root_node();
    let files = collect_imported_files(&root_node, &content, file_path);

    let mut dependencies: Vec<String> = files
        .into_iter()
        .map(|file_info| file_info.path)
        .filter(|path| path != file_path)
        .collect();

    if include_itself {
        dependencies.push(file_path.to_string());
    }

    Ok(dependencies)
}

#[must_use]
pub fn contract_abi(content: &str, file_path: &str) -> ContractAbi {
    let contract_name = get_contract_name_from_file_path(file_path);

    let Ok(tree) = tolk_syntax::parse(content) else {
        return ContractAbi::default();
    };
    let root_node = tree.root_node();

    let files = collect_imported_files(&root_node, content, file_path);

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
pub fn extract_handled_messages(content: &str, file_path: &str) -> Vec<String> {
    let Ok(tree) = tolk_syntax::parse(content) else {
        return Vec::new();
    };

    let root_node = tree.root_node();
    let files = collect_imported_files(&root_node, content, file_path);

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
    root_node: &tree_sitter::Node<'_>,
    content: &str,
    file_path: &str,
) -> Vec<FileInfo> {
    let mut files = Vec::new();
    let mut processed = HashSet::new();

    let Ok(parsed_file) = tolk_syntax::parse(content) else {
        return vec![];
    };
    files.push(FileInfo {
        path: file_path.to_string(),
        content: content.to_string(),
        tree: parsed_file.tree,
    });
    processed.insert(file_path.to_string());

    collect_imported_files_recursive(root_node, content, file_path, &mut files, &mut processed);

    files
}

fn collect_imported_files_recursive(
    node: &tree_sitter::Node<'_>,
    content: &str,
    file_path: &str,
    files: &mut Vec<FileInfo>,
    processed: &mut HashSet<String>,
) {
    let mut cursor = node.walk();
    for child in node
        .children(&mut cursor)
        .filter(|child| child.kind() == "import_directive")
    {
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };

        let import_path_text = path_node
            .utf8_text(content.as_bytes())
            .unwrap_or("")
            .to_string();

        let import_path = import_path_text.trim_matches('"');

        let resolved_path = resolve_import_path(file_path, import_path);
        let Some(resolved) = resolved_path else {
            continue;
        };

        if processed.contains(&resolved) {
            // recursive dependency, already processed
            continue;
        }

        let Ok(import_content) = fs::read_to_string(&resolved) else {
            continue;
        };

        if let Ok(parsed_file) = tolk_syntax::parse(&import_content) {
            let root_node = parsed_file.root_node();

            collect_imported_files_recursive(
                &root_node,
                &import_content,
                &resolved,
                files,
                processed,
            );

            files.push(FileInfo {
                path: resolved.clone(),
                content: import_content,
                tree: parsed_file.tree,
            });
            processed.insert(resolved);
        }
    }
}

fn resolve_import_path(base_file: &str, import_path: &str) -> Option<String> {
    let import_path = add_tolk_extension_if_needed(import_path.to_string());

    let base_path = Path::new(base_file).parent()?;

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(code, "test.tolk");

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

        let abi = contract_abi(main_content, "main.tolk");

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
}
