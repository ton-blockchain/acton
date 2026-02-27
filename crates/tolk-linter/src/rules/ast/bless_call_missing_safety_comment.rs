use super::mode_literal_helpers::resolve_call_symbol;
use super::safety_comment_helpers::has_safety_comment_above;
use crate::rules::diagnostic::{Annotation, Diagnostic};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use tolk_macros::ViolationMetadata;
use tolk_resolver::AstNodeSpanExt;
use tolk_resolver::file_index::{FileId, Span, SymbolId};
use tolk_syntax::ast::expressions::Call;
use tolk_syntax::{FuncBody, FunctionLike, TopLevel};
use tolk_ty::InferenceResult;

/// ### What it does
/// Requires a `SAFETY` comment for calls to `transformSliceToContinuation`
/// and for calls to any `asm` function that uses `BLESS`.
///
/// ### Why is this bad?
/// `BLESS` creates continuations from raw code slices and can bypass
/// high-level control-flow assumptions. Call sites should document
/// why inputs are trusted and which invariants are required.
///
/// ### Example
/// ```tolk
/// fun convert(code: slice): continuation {
///     return transformSliceToContinuation(code);
/// }
/// ```
///
/// Use instead:
/// ```tolk
/// fun convert(code: slice): continuation {
///     // SAFETY: `code` is verified bytecode from trusted state.
///     return transformSliceToContinuation(code);
/// }
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct BlessCallMissingSafetyComment;

impl Violation for BlessCallMissingSafetyComment {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "converting slice to continuation requires safety comment".to_string()
    }
}

pub fn check_call(
    checker: &mut Checker,
    file_id: FileId,
    call: &Call,
    current_inference: Option<&InferenceResult>,
) -> Option<()> {
    let file = checker.file_db.get_by_id(file_id)?;
    let source = file.source().source.as_ref();

    let origin = find_bless_related_origin(checker, file_id, call, source, current_inference)?;

    if has_safety_comment_above(source, file.line_offsets(), call.span().start()) {
        return None;
    }

    let span = call
        .callee()
        .map_or_else(|| call.span(), |callee| callee.span());

    let mut annotations = vec![Annotation {
        span,
        message: Some("add `// SAFETY: ...` above this call".to_string()),
        is_primary: true,
        tags: vec![],
    }];

    if let Some(origin) = origin.as_ref()
        && origin.declaration_file_id == file_id
    {
        annotations.push(Annotation {
            span: origin.function_name_span,
            message: Some(format!(
                "callee resolves to `{}` which contains `BLESS`",
                origin.function_name
            )),
            is_primary: false,
            tags: vec![],
        });
        annotations.push(Annotation {
            span: origin.bless_span,
            message: Some("`BLESS` is used here".to_string()),
            is_primary: false,
            tags: vec![],
        });
    }

    let diagnostic = Diagnostic::warning_for(file_id, BlessCallMissingSafetyComment)
        .with_annotations(annotations)
        .with_help( "BLESS allows to execute ANY user-code; document why this is safe and what preconditions must hold");
    checker.emit_diagnostic(diagnostic);

    if let Some(origin) = origin.as_ref()
        && origin.declaration_file_id != file_id
    {
        emit_cross_file_help_diagnostic(checker, origin);
    }

    None
}

fn find_bless_related_origin(
    checker: &Checker,
    file_id: FileId,
    call: &Call,
    source: &str,
    current_inference: Option<&InferenceResult>,
) -> Option<Option<BlessOrigin>> {
    let is_transform_call_by_name = call
        .callee_identifier()
        .and_then(|callee| callee.utf8_text(source.as_bytes()).ok())
        .is_some_and(|name| name.trim_matches('`') == "transformSliceToContinuation");

    let Some(symbol_id) = resolve_call_symbol(checker, file_id, call, current_inference) else {
        return is_transform_call_by_name.then_some(None);
    };
    let Some(symbol) = checker.type_db.project_index.resolve_symbol(symbol_id) else {
        return is_transform_call_by_name.then_some(None);
    };

    let function_name = symbol.name.to_string();
    let function_name_span = symbol.name_span;

    if function_name == "transformSliceToContinuation" {
        // `transformSliceToContinuation` is always considered dangerous for call sites.
        return Some(symbol_bless_origin(
            checker,
            symbol_id,
            &function_name,
            function_name_span,
        ));
    }

    symbol_bless_origin(checker, symbol_id, &function_name, function_name_span).map(Some)
}

fn symbol_bless_origin(
    checker: &Checker,
    symbol_id: SymbolId,
    function_name: &str,
    function_name_span: Span,
) -> Option<BlessOrigin> {
    let file = checker.file_db.get_by_id(symbol_id.file_id)?;
    let decl = file.find_syntax_declaration(symbol_id)?;
    let source = file.source().source.as_ref();

    let bless_span = match decl {
        TopLevel::Func(func) => find_bless_instruction_span(func.body(), source),
        TopLevel::Method(method) => find_bless_instruction_span(method.body(), source),
        TopLevel::GetMethod(get_method) => find_bless_instruction_span(get_method.body(), source),
        _ => None,
    }?;

    Some(BlessOrigin {
        function_name: function_name.to_string(),
        function_name_span,
        bless_span,
        bless_line: offset_to_line_1based(file.line_offsets(), bless_span.start()),
        declaration_file_id: symbol_id.file_id,
    })
}

fn find_bless_instruction_span<'tree>(
    body: Option<FuncBody<'tree>>,
    source: &'tree str,
) -> Option<Span> {
    let Some(FuncBody::AsmBody(asm_body)) = body else {
        return None;
    };

    asm_body
        .instructions()
        .find(|instruction| contains_bless_opcode(instruction.content(source)))
        .map(|instruction| instruction.span())
}

fn contains_bless_opcode(text: &str) -> bool {
    text.contains("BLESS")
}

fn offset_to_line_1based(line_offsets: &[usize], offset: usize) -> usize {
    match line_offsets.binary_search(&offset) {
        Ok(line) => line + 1,
        Err(0) => 1,
        Err(next_line) => next_line,
    }
}

struct BlessOrigin {
    function_name: String,
    function_name_span: Span,
    bless_span: Span,
    bless_line: usize,
    declaration_file_id: FileId,
}

fn emit_cross_file_help_diagnostic(checker: &mut Checker, origin: &BlessOrigin) {
    let diagnostic = Diagnostic::help_for(
        origin.declaration_file_id,
        BlessCallMissingSafetyComment,
        "called function is declared here",
    )
    .with_annotations(vec![
        Annotation {
            span: origin.function_name_span,
            message: Some(format!(
                "function `{}` is declared here",
                origin.function_name
            )),
            is_primary: false,
            tags: vec![],
        },
        Annotation {
            span: origin.bless_span,
            message: Some(format!("`BLESS` is used here (line {})", origin.bless_line)),
            is_primary: true,
            tags: vec![],
        },
    ]);
    checker.emit_diagnostic(diagnostic);
}
