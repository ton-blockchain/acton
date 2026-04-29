use super::{GITHUB_SOURCE_BASE, generated_notice_from_path};
use anyhow::Result;
use path_absolutize::Absolutize;
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use tolk_syntax::{
    AstNode, BaseFunction, Constant as AstConstant, Enum as AstEnum, HasName, SourceFile,
    Struct as AstStruct, TopLevel, TypeAlias as AstTypeAlias, parse,
};

pub(super) fn generate_stdlib_docs(
    lib_dir: &Path,
    tolk_stdlib_dir: &Path,
    stdlib_out_dir: &Path,
    tolk_stdlib_out_dir: &Path,
) -> Result<()> {
    let legacy_nested_tolk_dir = stdlib_out_dir.join("tolk_stdlib");
    if legacy_nested_tolk_dir.exists() {
        fs::remove_dir_all(&legacy_nested_tolk_dir)?;
    }

    let mut docs = collect_docs(lib_dir, stdlib_out_dir, SourceKind::Acton)?;
    docs.extend(collect_docs(
        tolk_stdlib_dir,
        tolk_stdlib_out_dir,
        SourceKind::Tolk,
    )?);

    write_stdlib_index(stdlib_out_dir)?;
    write_tolk_stdlib_index(tolk_stdlib_out_dir)?;

    let symbol_map = build_symbol_map(&docs);
    let link_regex = Regex::new(r"\[([a-zA-Z0-9_.]+)]")?;

    for doc in &docs {
        write_doc_page(doc, &symbol_map, &link_regex)?;
    }

    Ok(())
}

fn write_stdlib_index(stdlib_out_dir: &Path) -> Result<()> {
    fs::create_dir_all(stdlib_out_dir)?;
    let index_article = stdlib_out_dir.join("overview.mdx");
    fs::write(
        &index_article,
        render_generated_index_page(
            "Overview",
            "All available functions, structs, constants, and other entities available in Acton standard library",
            "Acton provides a collection of functions for writing scripts and tests in Tolk.\n\nThe Tolk stdlib is documented in [Tolk standard library](/docs/tolk_standard_library/overview).\n",
            Path::new(super::ACTON_STDLIB_SRC),
        ),
    )?;
    let meta_file = stdlib_out_dir.join("meta.json");
    let _ = fs::write(
        &meta_file,
        "{\n  \"title\": \"Acton standard library\",\n  \"icon\": \"FileCode\",\n  \"pages\": [\n    \"overview\",\n    \"...\"\n  ]\n}\n",
    );
    Ok(())
}

fn write_tolk_stdlib_index(tolk_stdlib_out_dir: &Path) -> Result<()> {
    fs::create_dir_all(tolk_stdlib_out_dir)?;
    let index_article = tolk_stdlib_out_dir.join("overview.mdx");
    fs::write(
        &index_article,
        render_generated_index_page(
            "Overview",
            "Bundled Tolk stdlib modules used by Acton standard library",
            "This section contains documentation of Tolk standard library.\n",
            Path::new(super::TOLK_STDLIB_SRC),
        ),
    )?;
    let meta_file = tolk_stdlib_out_dir.join("meta.json");
    let _ = fs::write(
        &meta_file,
        "{\n  \"title\": \"Tolk standard library\",\n  \"icon\": \"FileCode\",\n  \"pages\": [\n    \"overview\",\n    \"...\"\n  ]\n}\n",
    );
    Ok(())
}

fn collect_docs(
    source_dir: &Path,
    output_root: &Path,
    source_kind: SourceKind,
) -> Result<Vec<FileDoc>> {
    if !source_dir.exists() {
        anyhow::bail!("Directory '{}' not found", source_dir.display());
    }

    let mut files: Vec<_> = walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "tolk")
        })
        .collect();

    files.sort_by_key(|e| e.path().to_string_lossy().to_string());

    let mut docs = Vec::new();

    for entry in files {
        let path = entry.path();
        let path_string = path.to_string_lossy();
        if path_string.contains("tests") || path_string.ends_with(".test.tolk") {
            continue;
        }

        let content = fs::read_to_string(path)?;
        let relative_path = path.strip_prefix(source_dir)?;
        if should_skip_stdlib_doc(relative_path) {
            continue;
        }
        let file_stem = relative_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let source_file = parse(&content)?;
        let symbols = extract_symbols(&source_file, &content);
        let symbols: Vec<_> = symbols.into_iter().filter(|s| !skip_symbol(s)).collect();
        let file_header = extract_file_header_doc(&content);

        if !symbols.is_empty() || file_header.is_some() {
            docs.push(FileDoc {
                source_path: path.to_path_buf(),
                output_stem_path: output_root.join(relative_path.with_extension("")),
                docs_url: build_docs_url(source_kind.docs_url_root(), relative_path),
                title: file_stem.clone(),
                description: format!("{}.tolk {}", file_stem, source_kind.description_suffix()),
                symbols,
                file_header,
            });
        }
    }

    Ok(docs)
}

fn should_skip_stdlib_doc(relative_path: &Path) -> bool {
    relative_path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.starts_with('_') || name == "impl.tolk"
    })
}

fn build_symbol_map(docs: &[FileDoc]) -> HashMap<String, Vec<LinkTarget>> {
    let mut symbol_map: HashMap<String, Vec<LinkTarget>> = HashMap::new();
    for doc in docs {
        for symbol in &doc.symbols {
            let link_target = LinkTarget {
                path: doc.output_stem_path.clone(),
                url: doc.docs_url.clone(),
                anchor: symbol.name.clone(),
            };
            insert_link_target(&mut symbol_map, symbol.name.clone(), link_target.clone());
            for alias in &symbol.link_aliases {
                insert_link_target(&mut symbol_map, alias.clone(), link_target.clone());
            }
        }
    }
    symbol_map
}

fn write_doc_page(
    doc: &FileDoc,
    symbol_map: &HashMap<String, Vec<LinkTarget>>,
    link_regex: &Regex,
) -> Result<()> {
    let current_file_stem_path = &doc.output_stem_path;
    let mut output_path = current_file_stem_path.clone();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    output_path.set_extension("mdx");

    let mut mdx_content = String::new();
    mdx_content.push_str("---\n");
    let _ = writeln!(mdx_content, "title: \"{}\"", doc.title);
    let _ = writeln!(mdx_content, "description: \"{}\"", doc.description);
    mdx_content.push_str("---\n\n");
    mdx_content.push_str("import { SourceCodeLink } from '@/components/SourceCodeLink';\n\n");
    mdx_content.push_str(&generated_notice_from_path(&doc.source_path));

    if let Some(header) = &doc.file_header {
        mdx_content.push_str(&sanitize_mdx_text(header));
        mdx_content.push_str("\n\n");
    }

    if !doc.symbols.is_empty() {
        mdx_content.push_str("## Definitions\n\n");
    }

    for symbol in &doc.symbols {
        let _ = writeln!(mdx_content, "## `{}`\n", symbol.name);

        let source_url = format!(
            "{GITHUB_SOURCE_BASE}/{}#L{}",
            doc.source_path.to_string_lossy(),
            symbol.start_line + 1
        );

        mdx_content.push_str("```tolk\n");
        mdx_content.push_str(&symbol.signature);
        mdx_content.push_str("\n```\n\n");

        if let Some(doc_text) = symbol.doc.as_ref() {
            if Some(doc_text) == doc.file_header.as_ref() {
                // skip if the symbol doc is exactly the same as the file header
            } else {
                let processed_doc =
                    link_regex.replace_all(doc_text, |caps: &regex::Captures<'_>| {
                        let name = &caps[1];
                        if let Some(link_target) =
                            resolve_link_target(symbol_map, name, current_file_stem_path)
                        {
                            let target_path = &link_target.path;
                            if target_path == current_file_stem_path {
                                format!(
                                    "[{}](#{})",
                                    name,
                                    normalize_symbol_link(&link_target.anchor)
                                )
                            } else {
                                format!(
                                    "[{}]({}#{})",
                                    name,
                                    link_target.url,
                                    normalize_symbol_link(&link_target.anchor)
                                )
                            }
                        } else {
                            let full_path = match doc.source_path.absolutize() {
                                Ok(path) => path.to_path_buf(),
                                Err(_) => doc.source_path.clone(),
                            };

                            eprintln!(
                                "Warning: Symbol '{name}' in '{}:{}' not found in documentation",
                                full_path.to_string_lossy(),
                                symbol.start_line
                            );
                            name.to_string()
                        }
                    });
                mdx_content.push_str(&sanitize_mdx_text(&processed_doc));
                mdx_content.push_str("\n\n");
            }
        }

        let _ = writeln!(mdx_content, "<SourceCodeLink href=\"{source_url}\" />\n");
    }

    fs::write(output_path, mdx_content)?;
    Ok(())
}

fn render_generated_index_page(
    title: &str,
    description: &str,
    body: &str,
    source_path: &Path,
) -> String {
    let generated_notice = generated_notice_from_path(source_path);
    format!("---\ntitle: {title:?}\ndescription: {description:?}\n---\n\n{generated_notice}{body}")
}

fn normalize_symbol_link(link: &str) -> String {
    link.replace('\\', "/")
        .replace('.', "")
        .to_ascii_lowercase()
}

fn skip_symbol(s: &SymbolInfo) -> bool {
    s.name == "ffi"
        || s.name == "impl"
        || s.name == "expect_impl"
        || s.name == "impl_msg"
        || s.name.starts_with("ffi.")
        || s.name.starts_with("impl.")
        || s.name.starts_with("expect_impl.")
        || s.name.starts_with("impl_msg.")
        || s.name.starts_with("never.")
        || s.name.starts_with("unknown.")
        || s.name.contains("__")
}

enum SymbolKind {
    Function,
    Struct,
    Enum,
    TypeAlias,
    Constant,
}

struct SymbolInfo {
    #[allow(dead_code)]
    kind: SymbolKind,
    name: String,
    signature: String,
    doc: Option<String>,
    start_line: usize,
    link_aliases: Vec<String>,
}

struct FileDoc {
    source_path: PathBuf,
    output_stem_path: PathBuf,
    docs_url: String,
    title: String,
    description: String,
    symbols: Vec<SymbolInfo>,
    file_header: Option<String>,
}

enum SourceKind {
    Acton,
    Tolk,
}

impl SourceKind {
    const fn description_suffix(&self) -> &'static str {
        match self {
            Self::Acton => "standard library file",
            Self::Tolk => "Tolk standard library file",
        }
    }

    const fn docs_url_root(&self) -> &'static str {
        match self {
            Self::Acton => "/docs/standard_library",
            Self::Tolk => "/docs/tolk_standard_library",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LinkTarget {
    path: PathBuf,
    url: String,
    anchor: String,
}

fn build_docs_url(root: &str, relative_path: &Path) -> String {
    let path = relative_path.with_extension("");
    let path = path.to_string_lossy().replace('\\', "/");
    format!("{root}/{path}")
}

fn extract_symbols(source_file: &SourceFile, source: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    for top_level in source_file.top_levels() {
        match top_level {
            TopLevel::Func(func) => {
                if let Some(symbol) = parse_function(BaseFunction::Function(func), source) {
                    symbols.push(symbol);
                }
            }
            TopLevel::Method(method) => {
                if let Some(symbol) =
                    parse_function(BaseFunction::MethodDeclaration(method), source)
                {
                    symbols.push(symbol);
                }
            }
            TopLevel::GetMethod(get_method) => {
                if let Some(symbol) =
                    parse_function(BaseFunction::GetMethodDeclaration(get_method), source)
                {
                    symbols.push(symbol);
                }
            }
            TopLevel::TypeAlias(type_alias) => {
                if let Some(symbol) = parse_type_alias(type_alias, source) {
                    symbols.push(symbol);
                }
            }
            TopLevel::Struct(struct_decl) => {
                if let Some(symbol) = parse_struct(struct_decl, source) {
                    symbols.push(symbol);
                }
            }
            TopLevel::Enum(enum_decl) => {
                if let Some(symbol) = parse_enum(enum_decl, source) {
                    symbols.push(symbol);
                }
            }
            TopLevel::Constant(const_decl) => {
                if let Some(symbol) = parse_constant(const_decl, source) {
                    symbols.push(symbol);
                }
            }
            _ => {}
        }
    }

    symbols
}

fn parse_type_alias(type_alias: AstTypeAlias<'_>, source: &str) -> Option<SymbolInfo> {
    let name = extract_name(&type_alias, source)?;
    let signature = type_alias.text(source);

    let doc = extract_doc_comment(type_alias.syntax().start_byte(), source);
    let start_line = type_alias.syntax().start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::TypeAlias,
        name,
        signature: signature.to_owned(),
        doc,
        start_line,
        link_aliases: Vec::new(),
    })
}

fn parse_struct(struct_decl: AstStruct<'_>, source: &str) -> Option<SymbolInfo> {
    let name = extract_name(&struct_decl, source)?;

    let signature = struct_decl.text(source);
    let link_aliases = parse_struct_field_aliases(struct_decl, source, &name);

    let doc = extract_doc_comment(struct_decl.syntax().start_byte(), source);
    let start_line = struct_decl.syntax().start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Struct,
        name,
        signature: signature.to_owned(),
        doc,
        start_line,
        link_aliases,
    })
}

fn parse_enum(enum_decl: AstEnum<'_>, source: &str) -> Option<SymbolInfo> {
    let name = extract_name(&enum_decl, source)?;
    let signature = enum_decl.text(source);
    let link_aliases = parse_enum_member_aliases(enum_decl, source, &name);

    let doc = extract_doc_comment(enum_decl.syntax().start_byte(), source);
    let start_line = enum_decl.syntax().start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Enum,
        name,
        signature: signature.to_owned(),
        doc,
        start_line,
        link_aliases,
    })
}

fn parse_constant(constant: AstConstant<'_>, source: &str) -> Option<SymbolInfo> {
    let name = extract_name(&constant, source)?;
    let full_text = constant.text(source);
    let doc = extract_doc_comment(constant.syntax().start_byte(), source);
    let start_line = constant.syntax().start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Constant,
        name,
        signature: full_text.to_string(),
        doc,
        start_line,
        link_aliases: Vec::new(),
    })
}

fn parse_function(function: BaseFunction<'_>, source: &str) -> Option<SymbolInfo> {
    let mut name = extract_name(&function, source)?;
    if let BaseFunction::MethodDeclaration(method) = function
        && let Some(receiver_type) = method.receiver_type()
    {
        name = format!("{}.{}", receiver_type.text(source), name);
    }

    let function_syntax = function.syntax();
    let full_text = function_syntax.utf8_text(source.as_bytes()).ok()?;
    let link_aliases = parse_parameter_aliases(function, source, &name);

    let signature = if let Some(body) = function.body() {
        let cut_idx = body.syntax().start_byte() - function_syntax.start_byte();
        full_text[..cut_idx].trim().to_string()
    } else {
        full_text.to_string()
    };

    let doc = extract_doc_comment(function_syntax.start_byte(), source);
    let start_line = function_syntax.start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Function,
        link_aliases,
        name,
        signature,
        doc,
        start_line,
    })
}

fn parse_parameter_aliases(
    function: BaseFunction<'_>,
    source: &str,
    owner_name: &str,
) -> Vec<String> {
    let mut aliases = Vec::new();

    for parameter in function.parameters() {
        let Some(parameter_name) = extract_name(&parameter, source) else {
            continue;
        };
        if parameter_name.is_empty() || parameter_name == "self" {
            continue;
        }

        aliases.push(parameter_name.clone());
        aliases.push(format!("{owner_name}.{parameter_name}"));
    }

    aliases
}

fn parse_struct_field_aliases(
    struct_decl: AstStruct<'_>,
    source: &str,
    owner_name: &str,
) -> Vec<String> {
    let mut aliases = Vec::new();

    let Some(body) = struct_decl.body() else {
        return aliases;
    };

    for field in body.fields() {
        let Some(field_name) = extract_name(&field, source) else {
            continue;
        };
        if field_name.is_empty() {
            continue;
        }
        aliases.push(format!("{owner_name}.{field_name}"));
    }

    aliases
}

fn parse_enum_member_aliases(
    enum_decl: AstEnum<'_>,
    source: &str,
    owner_name: &str,
) -> Vec<String> {
    let mut aliases = Vec::new();

    let Some(body) = enum_decl.body() else {
        return aliases;
    };

    for member in body.members() {
        let Some(member_name) = extract_name(&member, source) else {
            continue;
        };
        if member_name.is_empty() {
            continue;
        }
        aliases.push(format!("{owner_name}.{member_name}"));
    }

    aliases
}

fn insert_link_target(
    symbol_map: &mut HashMap<String, Vec<LinkTarget>>,
    name: String,
    target: LinkTarget,
) {
    let entry = symbol_map.entry(name).or_default();
    if !entry.iter().any(|existing| existing == &target) {
        entry.push(target);
    }
}

fn resolve_link_target<'a>(
    symbol_map: &'a HashMap<String, Vec<LinkTarget>>,
    name: &str,
    current_file_stem_path: &Path,
) -> Option<&'a LinkTarget> {
    let targets = symbol_map.get(name)?;
    if targets.len() == 1 {
        return targets.first();
    }

    targets
        .iter()
        .find(|target| target.path == current_file_stem_path)
}

fn extract_name<'tree, N>(node: &N, source: &'tree str) -> Option<String>
where
    N: HasName<'tree>,
{
    let name = node.name()?;
    Some(name.text(source).trim_matches('`').to_string())
}

fn extract_doc_comment(start_byte: usize, source: &str) -> Option<String> {
    let prefix = &source[..start_byte];

    let mut lines = Vec::new();

    for line in prefix.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !lines.is_empty() {
                break;
            }
        } else if trimmed.starts_with("///") {
            let content = trimmed.trim_start_matches("///");
            let content = if let Some(stripped) = content.strip_prefix(' ') {
                stripped.to_string()
            } else {
                content.to_string()
            };

            lines.push(content);
        } else {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
    }
}

fn extract_file_header_doc(source: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut has_content = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("///") {
            let content = trimmed.trim_start_matches("///");
            let content = if let Some(stripped) = content.strip_prefix(' ') {
                stripped
            } else {
                content
            };
            lines.push(content);
            has_content = true;
        } else if trimmed.is_empty() {
            break;
        } else {
            // Found a non-comment, non-empty line. Stop.
            break;
        }
    }

    // Trim trailing empty lines from the buffer
    while let Some(last) = lines.last() {
        if last.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    if has_content && !lines.is_empty() {
        Some(lines.join("\n"))
    } else {
        None
    }
}

fn sanitize_mdx_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_fenced_code_block = false;

    for segment in text.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let ends_with_newline = segment.ends_with('\n');

        if line.trim_start().starts_with("```") {
            in_fenced_code_block = !in_fenced_code_block;
            result.push_str(line);
        } else if in_fenced_code_block {
            result.push_str(line);
        } else {
            result.push_str(&escape_mdx_text_line(line));
        }

        if ends_with_newline {
            result.push('\n');
        }
    }

    result
}

fn escape_mdx_text_line(line: &str) -> String {
    let mut escaped = String::with_capacity(line.len());
    let mut in_inline_code = false;

    for ch in line.chars() {
        if ch == '`' {
            in_inline_code = !in_inline_code;
            escaped.push(ch);
            continue;
        }

        if !in_inline_code && ch == '<' {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}
