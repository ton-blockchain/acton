use super::reference::{reference_identifier, resolved_items_at};
use super::{TlbParsedDocument, psi::TlbPsiFile};
use crate::{Location, Position};
use ton_syntax::ast::PreorderTraverse;

pub(super) fn references(
    document: &crate::DocumentSnapshot,
    parsed: &TlbParsedDocument,
    position: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let file = TlbPsiFile::new(document, parsed);
    let Some(node) = file.node_at(position) else {
        return Vec::new();
    };
    let Some(identifier) = reference_identifier(node) else {
        return Vec::new();
    };
    let targets = resolved_items_at(&parsed.source_file, identifier);
    if targets.is_empty() {
        return Vec::new();
    }

    let name = document.text_of(identifier).trim();
    if name.is_empty() {
        return Vec::new();
    }
    let mut locations = Vec::new();

    if include_declaration {
        locations.extend(
            targets
                .iter()
                .map(|target| location(document, file.range_of(target.node))),
        );
    }

    for candidate in PreorderTraverse::new(parsed.source_file.root_node().walk()) {
        if !matches!(candidate.kind(), "identifier" | "type_identifier") {
            continue;
        }
        if document.text_of(candidate).trim() != name {
            continue;
        }
        if targets.iter().any(|target| target.node == candidate) {
            continue;
        }

        let resolved = resolved_items_at(&parsed.source_file, candidate);
        if resolved
            .iter()
            .any(|item| targets.iter().any(|target| target.node == item.node))
        {
            locations.push(location(document, file.range_of(candidate)));
        }
    }

    locations
}

fn location(document: &crate::DocumentSnapshot, range: crate::Range) -> Location {
    Location::new(document.uri().clone(), range)
}
