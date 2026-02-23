use crate::commands::check::pos;
use acton_config::config::ActonConfig;
use std::path::Path;
use std::time::Instant;
use tolk_linter::Rule;
use tolk_linter::diagnostic::{Annotation, Diagnostic, Severity};
use tolk_resolver::{FileDb, Span};
use tolkc::Compiler;
use tree_sitter::Point;

pub(super) fn check_with_compiler(
    root: &Path,
    file_db: &FileDb,
    acton_config: &ActonConfig,
    all_diagnostics: &mut Vec<Diagnostic>,
) -> anyhow::Result<bool> {
    let now = Instant::now();

    let compiler = Compiler::new(2).with_mappings(&acton_config.mappings);
    let compiler_errors = compiler.check(root)?;
    log::debug!(
        "Run compiler check took {:?}, found {} errors in {}",
        now.elapsed(),
        compiler_errors.len(),
        root.display()
    );

    let has_compiler_errors = compiler_errors.is_empty();

    for compiler_error in compiler_errors {
        let file_info = match file_db.process(Path::new(&compiler_error.range.file_name)) {
            Ok(file_id) => file_id,
            Err(error) => {
                log::warn!("Cannot process file for compiler error {error}");
                continue;
            }
        };

        let file_source = file_info.source().source.clone();

        let start_byte = pos::byte_offset_from_point(
            &Point {
                row: compiler_error.range.start_line_no - 1,
                column: compiler_error.range.start_char_no - 1,
            },
            &file_source,
        );
        let end_byte = pos::byte_offset_from_point(
            &Point {
                row: compiler_error.range.end_line_no - 1,
                column: compiler_error.range.end_char_no - 1,
            },
            &file_source,
        );

        let diagnostic = Diagnostic {
            file_id: file_info.id(),
            severity: Severity::Error,
            code: Some("C001".to_owned()),
            rule: Rule::CompilerError,
            name: "compiler-error",
            message: compiler_error.message.clone(),
            annotations: vec![Annotation {
                span: Span {
                    start: start_byte as u32,
                    end: end_byte as u32,
                },
                message: None,
                is_primary: true,
                tags: vec![],
            }],
            fixes: vec![],
            help: None,
        };
        all_diagnostics.push(diagnostic);
    }
    Ok(has_compiler_errors)
}
