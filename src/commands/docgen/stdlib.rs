use super::GITHUB_SOURCE_BASE;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tolk_syntax::{
    AstNode, BaseFunction, Constant as AstConstant, Enum as AstEnum, HasName, SourceFile,
    Struct as AstStruct, TopLevel, TypeAlias as AstTypeAlias, parse,
};

pub(super) fn generate_stdlib_docs(lib_dir: &Path, out_dir: &Path) -> Result<()> {
    if !lib_dir.exists() {
        anyhow::bail!("Directory 'lib' not found");
    }

    fs::create_dir_all(out_dir)?;

    let index_article = out_dir.join("index.mdx");
    fs::write(
        &index_article,
        r#"---
title: "Standard library"
description: "Learn about available functions/struct/constants available in Acton standard library"
icon: FileCode
---

Acton provides a collection of functions for writing scripts and tests in Tolk.
"#,
    )?;

    let mut files: Vec<_> = walkdir::WalkDir::new(lib_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "tolk")
        })
        .collect();

    files.sort_by_key(|e| e.path().to_string_lossy().to_string());

    struct FileDoc {
        path: PathBuf,
        file_stem: String,
        symbols: Vec<SymbolInfo>,
        file_header: Option<String>,
    }

    let mut docs = Vec::new();
    let mut symbol_map: HashMap<String, Vec<LinkTarget>> = HashMap::new();

    for entry in files {
        let path = entry.path();
        let path_string = path.to_string_lossy();
        if path_string.contains("tests") || path_string.ends_with(".test.tolk") {
            continue;
        }

        let content = fs::read_to_string(path)?;
        let relative_path = path.strip_prefix(lib_dir)?;
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
            let target_rel_path = relative_path.with_extension("");
            for symbol in &symbols {
                let link_target = LinkTarget {
                    path: target_rel_path.clone(),
                    anchor: symbol.name.clone(),
                };
                insert_link_target(&mut symbol_map, symbol.name.clone(), link_target.clone());
                for alias in &symbol.link_aliases {
                    insert_link_target(&mut symbol_map, alias.clone(), link_target.clone());
                }
            }

            docs.push(FileDoc {
                path: path.to_path_buf(),
                file_stem,
                symbols,
                file_header,
            });
        }
    }

    let link_regex = Regex::new(r"\[([a-zA-Z0-9_.]+)]")?;

    for doc in docs {
        let relative_path = doc.path.strip_prefix(lib_dir)?;
        let current_file_stem_path = relative_path.with_extension("");

        let mut output_path = out_dir.to_path_buf();
        if let Some(parent) = relative_path.parent() {
            output_path.push(parent);
            fs::create_dir_all(&output_path)?;
        }
        output_path.push(format!("{}.mdx", doc.file_stem));

        let mut mdx_content = String::new();
        mdx_content.push_str("---\n");
        mdx_content.push_str(&format!("title: \"{}\"\n", doc.file_stem));
        mdx_content.push_str(&format!(
            "description: \"{}.tolk standard library file\"\n",
            doc.file_stem
        ));
        mdx_content.push_str("---\n\n");
        mdx_content.push_str("import { SourceCodeLink } from '@/components/SourceCodeLink';\n\n");

        if let Some(header) = &doc.file_header {
            mdx_content.push_str(header);
            mdx_content.push_str("\n\n");
        }

        if !doc.symbols.is_empty() {
            mdx_content.push_str("## Definitions\n\n");
        }

        for symbol in doc.symbols {
            mdx_content.push_str(&format!("## `{}`\n\n", symbol.name));

            let source_url = format!(
                "{GITHUB_SOURCE_BASE}/{}#L{}",
                doc.path.to_string_lossy(),
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
                                resolve_link_target(&symbol_map, name, &current_file_stem_path)
                            {
                                let target_path = &link_target.path;
                                if target_path == &current_file_stem_path {
                                    format!(
                                        "[{}](#{})",
                                        name,
                                        normalize_symbol_link(&link_target.anchor)
                                    )
                                } else {
                                    let relative_link_path =
                                        pathdiff::diff_paths(target_path, &current_file_stem_path)
                                            .unwrap_or_else(|| target_path.clone());

                                    let link = relative_link_path.to_string_lossy().to_string();
                                    format!(
                                        "[{}]({}/#{})",
                                        name,
                                        link,
                                        normalize_symbol_link(&link_target.anchor)
                                    )
                                }
                            } else {
                                eprintln!("Warning: Symbol '{name}' not found in documentation");
                                name.to_string()
                            }
                        });
                    mdx_content.push_str(&processed_doc);
                    mdx_content.push_str("\n\n");
                }
            }

            mdx_content.push_str(&format!("<SourceCodeLink href=\"{source_url}\" />\n\n"));
        }
        fs::write(output_path, mdx_content)?;
    }

    Ok(())
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct LinkTarget {
    path: PathBuf,
    anchor: String,
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
