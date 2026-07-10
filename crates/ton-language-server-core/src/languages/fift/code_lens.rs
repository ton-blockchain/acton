use super::FiftParsedDocument;
use crate::{CodeLens, Command, DocumentSnapshot};
use fift_syntax::{AstNode, TopLevel};

pub(super) const OPEN_FILE_COMMAND: &str = "ton.openFile";

pub(super) fn code_lenses(
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
) -> Vec<CodeLens> {
    parsed
        .source_file
        .top_levels()
        .filter_map(|top_level| {
            let TopLevel::Definition(definition) = top_level else {
                return None;
            };
            let syntax = definition.syntax();
            let comment = syntax.prev_named_sibling()?;
            if comment.kind() != "comment" {
                return None;
            }

            let source_location = document.text_of(comment).trim().strip_prefix("// ")?.trim();
            let (path, line) = source_location.split_once(':')?;
            let line = line.parse::<u32>().ok()?;
            if path.is_empty() {
                return None;
            }

            let mut command = Command::new(
                format!("Go to Tolk sources ({source_location})"),
                OPEN_FILE_COMMAND,
            );
            command.arguments = vec![path.to_owned(), line.to_string()];

            Some(CodeLens::new(
                document.text_index().range_of_node(document.text(), syntax),
                Some(command),
            ))
        })
        .collect()
}
