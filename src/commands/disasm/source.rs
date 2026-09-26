//! Preserve selected Tolk functions in an isolated compilation for inspection.

use anyhow::{Context, Result, anyhow, bail};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tasm_core::decompile::Disassembler;
use tolk_compiler::compiler::CompilerResultSuccess;
use tolk_compiler::{Compiler, CompilerResult};
use tolk_syntax::{
    AstNode, BaseFunction, FuncBody, HasAnnotations, HasName, TryFromNode, parse_tolk_int_literal,
};
use tycho_types::boc::Boc;

pub(super) struct SelectedFunction {
    pub name: String,
    pub method_id: u64,
}

pub(super) struct SourceCompilation {
    pub output: CompilerResultSuccess,
    pub functions: Vec<SelectedFunction>,
    pub column_shifts: SourceColumnShifts,
}

/// Annotation overrides preserve source lines. Only inserted annotation columns need translating.
#[derive(Default)]
pub(super) struct SourceColumnShifts(HashMap<String, Vec<(i64, i64, i64)>>);

impl SourceColumnShifts {
    pub(super) fn original_column(&self, file: &str, line: i64, column: i64) -> i64 {
        let mut original = column;
        let mut preceding = 0;
        for &(shift_line, start, length) in self.0.get(file).into_iter().flatten() {
            if shift_line == line {
                original -= (column - start - preceding).clamp(0, length);
                preceding += length;
            }
        }
        original
    }
}

/// Compiles the original import graph, then retains requested functions through source overrides.
/// The ordinary build cache and source files are not modified by this inspection build.
pub(super) fn compile(
    path: &Path,
    names: &[String],
    disassembler: &Disassembler,
) -> Result<SourceCompilation> {
    let config = acton_config::config::ActonConfig::load().unwrap_or_default();
    let compiler = Compiler::new(2)
        .with_mappings(&config.mappings())
        .with_allow_no_entrypoint(true);
    let compiled = compile_entrypoint(&compiler, path, names.is_empty())?;
    if names.is_empty() {
        return Ok(SourceCompilation {
            output: compiled,
            functions: Vec::new(),
            column_shifts: SourceColumnShifts::default(),
        });
    }

    // Let the compiler resolve imports and mappings rather than rebuilding its import graph.
    let source_map = compiled
        .source_map
        .as_ref()
        .context("Tolk compilation did not return source metadata")?;
    let cell = Boc::decode_base64(&compiled.code_boc64)?;
    let code = disassembler.decompile_cell(&cell)?;
    let requested = names.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut method_ids = HashMap::new();
    let mut overrides = Vec::new();
    let mut column_shifts = SourceColumnShifts::default();
    // Get-method hashes occupy 0x10000..0x20000; explicit IDs may use this upper range too.
    let mut next_id = 0x3_ffff_u64;

    for file in source_map.files() {
        if file.file_name.starts_with('@') {
            continue;
        }
        let source = fs::read_to_string(&file.file_name)
            .with_context(|| format!("Cannot read {}", file.file_name))?;
        let parsed = tolk_syntax::parse(&source)?;
        let mut edits = Vec::new();
        for declaration in parsed.top_levels() {
            let Ok(function) = BaseFunction::try_from_node(declaration.syntax()) else {
                continue;
            };
            let Some(ident) = function.name() else {
                continue;
            };
            let name = ident.normalized_name(&source);
            let qualified_name = function.receiver_type().map_or_else(
                || name.to_owned(),
                |receiver| format!("{}.{name}", receiver.text(&source)),
            );
            if !requested.contains(qualified_name.as_str()) {
                continue;
            }
            if method_ids.contains_key(&qualified_name) {
                bail!("Function `{qualified_name}` is ambiguous in this entrypoint");
            }
            if function.type_parameters_node().is_some()
                || matches!(function, BaseFunction::MethodDeclaration(_))
                || function.parameters().any(|parameter| parameter.mutate())
                || !matches!(function.body(), Some(FuncBody::Block(_)))
                || name == "onBouncedMessage"
            {
                bail!(
                    "Cannot select `{qualified_name}`: named disassembly supports non-generic functions with a block body and no receiver or mutate parameters, get-methods, and external entrypoints"
                );
            }

            let mut explicit_id = None;
            if let Some(annotations) = function.annotations() {
                for annotation in annotations.annotations() {
                    let Some(annotation_name) = annotation.name() else {
                        continue;
                    };
                    match annotation_name.text(&source) {
                        "inline" | "inline_ref" | "noinline" => {
                            let range = annotation.syntax().byte_range();
                            let blank = source[range.clone()]
                                .bytes()
                                .map(|byte| if byte == b'\n' { '\n' } else { ' ' })
                                .collect();
                            edits.push((range, blank));
                        }
                        "method_id" => {
                            let value = annotation
                                .args()
                                .and_then(|args| args.args().next())
                                .context("Missing @method_id value")?;
                            let literal = parse_tolk_int_literal(value.text(&source))
                                .context("Invalid @method_id value")?;
                            explicit_id =
                                Some(u64::from_str_radix(literal.digits(), literal.radix())?);
                        }
                        _ => {}
                    }
                }
            }

            let existing_id = if matches!(function, BaseFunction::GetMethodDeclaration(_)) {
                Some(u64::try_from(
                    compiled
                        .abi
                        .as_ref()
                        .and_then(|abi| abi.get_methods.iter().find(|method| method.name == name))
                        .with_context(|| format!("Cannot resolve get-method ID for `{name}`"))?
                        .tvm_method_id,
                )?)
            } else {
                explicit_id.or(match name {
                    "main" | "onInternalMessage" => Some(0),
                    // The disassembler reads the signed 19-bit dictionary keys as unsigned.
                    "onExternalMessage" => Some(0x7_ffff),
                    "onRunTickTock" => Some(0x7_fffe),
                    "onSplitPrepare" => Some(0x7_fffd),
                    "onSplitInstall" => Some(0x7_fffc),
                    _ => None,
                })
            };
            let (method_id, annotation) = if let Some(method_id) = existing_id {
                (method_id, "@noinline ".to_owned())
            } else {
                while next_id >= 0x2_0000 && code.find_method(next_id).is_some() {
                    next_id -= 1;
                }
                if next_id < 0x2_0000 {
                    bail!("No free method IDs for named disassembly");
                }
                let method_id = next_id;
                next_id -= 1;
                (method_id, format!("@noinline @method_id({method_id}) "))
            };
            let start = function.syntax().start_byte();
            let line = source[..start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            let column = start - source[..start].rfind('\n').map_or(0, |offset| offset + 1) + 1;
            column_shifts
                .0
                .entry(file.file_name.clone())
                .or_default()
                .push((
                    i64::try_from(line)?,
                    i64::try_from(column)?,
                    i64::try_from(annotation.len())?,
                ));
            edits.push((start..start, annotation));
            method_ids.insert(qualified_name, method_id);
        }

        if !edits.is_empty() {
            // Apply removals before insertions at the same offset, from the end of the file.
            edits.sort_by(|(left, _), (right, _)| {
                right.start.cmp(&left.start).then(right.end.cmp(&left.end))
            });
            let mut modified = source;
            for (range, replacement) in edits {
                modified.replace_range(range, &replacement);
            }
            overrides.push((file.file_name.clone(), modified));
        }
    }

    let mut selected = Vec::new();
    for name in names {
        if selected
            .iter()
            .any(|function: &SelectedFunction| function.name == *name)
        {
            continue;
        }
        let method_id = *method_ids.get(name).ok_or_else(|| {
            anyhow!("Function `{name}` was not found in this entrypoint's sources")
        })?;
        selected.push(SelectedFunction {
            name: name.clone(),
            method_id,
        });
    }
    let compiled = compile_entrypoint(&compiler.with_source_overrides(overrides), path, true)?;
    Ok(SourceCompilation {
        output: compiled,
        functions: selected,
        column_shifts,
    })
}

fn compile_entrypoint(
    compiler: &Compiler,
    path: &Path,
    debug_marks: bool,
) -> Result<CompilerResultSuccess> {
    match compiler.compile(path, debug_marks) {
        CompilerResult::Success(result) => Ok(result),
        CompilerResult::Error(error) => bail!(error.message),
    }
}
