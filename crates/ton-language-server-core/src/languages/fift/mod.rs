use crate::language::{
    FeatureSet, FoldingRangeRequest, LanguagePlugin, ParseRequest, ParsedDocument,
};
use crate::logging;
use crate::{FoldingRange, LanguageId};
use anyhow::Context;
use std::any::Any;
use tree_sitter::{Node, Tree};

pub const LANGUAGE_ID: &str = "fift";

#[derive(Clone, Copy, Debug, Default)]
pub struct FiftLanguage;

impl FiftLanguage {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LanguagePlugin for FiftLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["fif", "fift"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet {
            folding_ranges: true,
            ..FeatureSet::default()
        }
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        tracing::debug!(
            target: logging::FIFT_TARGET,
            operation = "fift.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            text_len = request.document.text().len(),
            "parsing Fift document"
        );
        let parse_started_at = request.profiler.start();
        let source_file =
            match fift_syntax::parse_with_old_tree(request.document.text(), request.old_tree) {
                Ok(source_file) => source_file,
                Err(error) => {
                    tracing::debug!(
                        target: logging::FIFT_TARGET,
                        operation = "fift.parse",
                        uri = request.document.uri().as_str(),
                        version = request.document.version(),
                        incremental = request.old_tree.is_some(),
                        error = %error,
                        "Fift parse failed"
                    );
                    return Err(error);
                }
            };
        request.profiler.finish("fift.parse", parse_started_at);
        tracing::debug!(
            target: logging::FIFT_TARGET,
            operation = "fift.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            has_error = source_file.tree.root_node().has_error(),
            "parsed Fift document"
        );

        Ok(Box::new(FiftParsedDocument { source_file }))
    }

    fn folding_ranges(
        &self,
        request: FoldingRangeRequest<'_>,
    ) -> anyhow::Result<Vec<FoldingRange>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<FiftParsedDocument>()
            .context("Fift parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let mut ranges = Vec::new();
        collect_ranges(parsed.source_file.tree.root_node(), &mut ranges);
        ranges.sort_by_key(|range| {
            (
                range.start_line,
                range.start_character,
                range.end_line,
                range.end_character,
            )
        });
        ranges.dedup();
        request
            .context
            .profiler
            .finish("fift.folding_ranges", started_at);
        tracing::debug!(
            target: logging::FIFT_TARGET,
            operation = "fift.folding_ranges",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            result_count = ranges.len(),
            "resolved Fift folding ranges"
        );

        Ok(ranges)
    }
}

#[derive(Debug)]
pub struct FiftParsedDocument {
    source_file: fift_syntax::SourceFile,
}

impl ParsedDocument for FiftParsedDocument {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}

fn collect_ranges(node: Node<'_>, ranges: &mut Vec<FoldingRange>) {
    match node.kind() {
        "program" => push_body_folding(node, "PROGRAM{", "END>c", ranges),
        "proc_definition" => push_body_folding(node, "PROC:<{", "}>", ranges),
        "proc_inline_definition" => push_body_folding(node, "PROCINLINE:<{", "}>", ranges),
        "proc_ref_definition" => push_body_folding(node, "PROCREF:<{", "}>", ranges),
        "method_definition" => push_body_folding(node, "METHOD:<{", "}>", ranges),
        "instruction_block" => push_body_folding(node, "<{", "}>", ranges),
        "if_statement" => {
            push_body_folding(node, "IF:<{", "}>", ranges);
            push_body_folding(node, "ELSE<{", "}>", ranges);
        }
        "ifjmp_statement" => push_body_folding(node, "IFJMP:<{", "}>", ranges),
        "while_statement" => {
            push_body_folding(node, "WHILE:<{", "}>DO<{", ranges);
            push_body_folding(node, "}>DO<{", "}>", ranges);
        }
        "repeat_statement" => push_body_folding(node, "REPEAT:<{", "}>", ranges),
        "until_statement" => push_body_folding(node, "UNTIL:<{", "}>", ranges),
        _ => {}
    }

    for child_index in 0..node.named_child_count() {
        let Ok(child_index) = u32::try_from(child_index) else {
            break;
        };
        let Some(child) = node.named_child(child_index) else {
            continue;
        };
        collect_ranges(child, ranges);
    }
}

fn push_body_folding(
    node: Node<'_>,
    open_token_kind: &str,
    close_token_kind: &str,
    ranges: &mut Vec<FoldingRange>,
) {
    let mut open_token = None;
    let mut close_token = None;

    for child_index in 0..node.child_count() {
        let Ok(child_index) = u32::try_from(child_index) else {
            break;
        };
        let Some(child) = node.child(child_index) else {
            continue;
        };
        if open_token.is_some() && child.kind() == close_token_kind {
            close_token = Some(child);
            break;
        }
        if child.kind() == open_token_kind {
            open_token = Some(child);
            close_token = None;
        }
    }

    let (Some(open_token), Some(close_token)) = (open_token, close_token) else {
        return;
    };
    let Ok(start_line) = u32::try_from(open_token.end_position().row) else {
        return;
    };
    let Some(end_row) = close_token.start_position().row.checked_sub(1) else {
        return;
    };
    let Ok(end_line) = u32::try_from(end_row) else {
        return;
    };
    if end_line <= start_line {
        return;
    }

    ranges.push(FoldingRange::line_range(start_line, end_line));
}
