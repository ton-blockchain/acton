use super::psi::TlbPsiFile;
use crate::logging;
use crate::{Position, Range};
use ton_syntax::ast::PreorderTraverse;
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum TlbNamedItemKind {
    Declaration,
    NamedField,
    Parameter,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TlbNamedItem<'tree> {
    pub(super) kind: TlbNamedItemKind,
    pub(super) node: Node<'tree>,
    pub(super) owner: Option<Node<'tree>>,
}

impl<'tree> TlbNamedItem<'tree> {
    pub(super) fn name(self, source: &'tree str) -> Option<&'tree str> {
        self.node.utf8_text(source.as_bytes()).ok().map(str::trim)
    }

    pub(super) fn owner_name(self, source: &'tree str) -> Option<&'tree str> {
        self.owner?.utf8_text(source.as_bytes()).ok().map(str::trim)
    }
}

pub(super) fn resolve_variants_at<'tree>(
    source_file: &'tree tlb_syntax::SourceFile,
    node: Node<'tree>,
) -> Vec<TlbNamedItem<'tree>> {
    let Some(identifier) = find_reference_identifier(node) else {
        return Vec::new();
    };
    if find_parent_of_kind(identifier, "type_parameter").is_some() {
        return Vec::new();
    }

    let mut result = Vec::new();
    for top_level in source_file.top_levels() {
        let tlb_syntax::TopLevel::Declaration(declaration) = top_level else {
            continue;
        };
        if let Some(name_node) = declaration_name_node(declaration) {
            result.push(TlbNamedItem {
                kind: TlbNamedItemKind::Declaration,
                node: name_node,
                owner: None,
            });
        }
    }

    let Some(raw_declaration) = find_parent_of_kind(identifier, "declaration") else {
        return result;
    };
    let declaration = tlb_syntax::Declaration(raw_declaration);
    if let Some(combinator) = declaration.combinator() {
        let owner = combinator.name().map(|name| name.0);
        for parameter in combinator.params() {
            if let Some(node) = find_type_parameter_node(parameter.0) {
                result.push(TlbNamedItem {
                    kind: TlbNamedItemKind::Parameter,
                    node,
                    owner,
                });
            }
        }
    }
    for node in PreorderTraverse::new(declaration.0.walk()) {
        if node.kind() != "combinator_expr" {
            continue;
        }
        let combinator = tlb_syntax::CombinatorExpr(node);
        let owner = combinator.name().map(|name| name.0);
        for parameter in combinator.params() {
            if let Some(node) = find_type_parameter_node(parameter.syntax()) {
                result.push(TlbNamedItem {
                    kind: TlbNamedItemKind::Parameter,
                    node,
                    owner,
                });
            }
        }
    }
    for field in declaration.fields() {
        let Some(value) = field.value() else {
            continue;
        };
        let name = match value {
            tlb_syntax::FieldKind::FieldNamed(field) => field.name().map(|name| name.0),
            tlb_syntax::FieldKind::FieldBuiltin(field) => field.name().map(|name| name.0),
            tlb_syntax::FieldKind::FieldCurlyExpr(_)
            | tlb_syntax::FieldKind::FieldAnonymous(_)
            | tlb_syntax::FieldKind::FieldExpr(_)
            | tlb_syntax::FieldKind::Unmapped(_) => None,
        };
        if let Some(node) = name {
            result.push(TlbNamedItem {
                kind: TlbNamedItemKind::NamedField,
                node,
                owner: None,
            });
        }
    }
    result
}

pub(super) fn resolved_items_at<'tree>(
    source_file: &'tree tlb_syntax::SourceFile,
    node: Node<'tree>,
) -> Vec<TlbNamedItem<'tree>> {
    let Some(identifier) = find_reference_identifier(node) else {
        return Vec::new();
    };
    let Ok(name) = identifier.utf8_text(source_file.source.as_bytes()) else {
        return Vec::new();
    };
    let name = name.trim();

    resolve_variants_at(source_file, identifier)
        .into_iter()
        .filter(|item| item.name(source_file.source.as_ref()) == Some(name))
        .collect()
}

pub(super) fn reference_identifier(node: Node<'_>) -> Option<Node<'_>> {
    find_reference_identifier(node)
}

pub(super) fn definition_ranges_at(psi_file: &TlbPsiFile<'_>, position: Position) -> Vec<Range> {
    let document = psi_file.document();
    tracing::trace!(
        target: logging::TLB_TARGET,
        operation = "tlb.reference.lookup",
        uri = document.uri().as_str(),
        version = document.version(),
        line = position.line,
        character = position.character,
        "looking up TL-B reference"
    );
    let Some(node) = psi_file.node_at(position) else {
        tracing::trace!(
            target: logging::TLB_TARGET,
            operation = "tlb.reference.lookup",
            uri = document.uri().as_str(),
            version = document.version(),
            line = position.line,
            character = position.character,
            found_node = false,
            "no TL-B node at position"
        );
        return Vec::new();
    };
    tracing::trace!(
        target: logging::TLB_TARGET,
        operation = "tlb.reference.lookup",
        uri = document.uri().as_str(),
        version = document.version(),
        line = position.line,
        character = position.character,
        found_node = true,
        node_kind = node.kind(),
        "found TL-B node at position"
    );
    let Some(reference) = TlbReference::new(psi_file, node) else {
        tracing::trace!(
            target: logging::TLB_TARGET,
            operation = "tlb.reference.lookup",
            uri = document.uri().as_str(),
            version = document.version(),
            line = position.line,
            character = position.character,
            node_kind = node.kind(),
            "TL-B node is not a reference"
        );
        return Vec::new();
    };
    reference.definition_ranges()
}

#[derive(Clone, Copy)]
struct TlbReference<'file, 'psi> {
    psi_file: &'file TlbPsiFile<'psi>,
    identifier: Node<'file>,
}

impl<'file, 'psi> TlbReference<'file, 'psi> {
    fn new(psi_file: &'file TlbPsiFile<'psi>, node: Node<'file>) -> Option<Self> {
        let identifier = find_reference_identifier(node)?;
        Some(Self {
            psi_file,
            identifier,
        })
    }

    fn name(&self) -> Option<&str> {
        self.identifier
            .utf8_text(self.psi_file.source().as_bytes())
            .ok()
            .map(str::trim)
    }

    fn definition_ranges(&self) -> Vec<Range> {
        if let Some(range) = self.declaration_name_definition_range() {
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.reference.resolve",
                kind = "declaration_name",
                result_count = 1,
                "resolved TL-B declaration name to itself"
            );
            return vec![range];
        }

        if find_parent_of_kind(self.identifier, "type_parameter").is_some() {
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.reference.resolve",
                kind = "type_parameter",
                result_count = 0,
                "skipped TL-B type parameter reference"
            );
            return Vec::new();
        }

        let Some(search_name) = self.name() else {
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.reference.resolve",
                result_count = 0,
                "TL-B reference has no text"
            );
            return Vec::new();
        };

        let mut ranges = Vec::new();
        for symbol in self.psi_file.declarations_named(search_name) {
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.reference.candidate",
                name = search_name,
                kind = "declaration",
                start_byte = symbol.start_byte,
                end_byte = symbol.end_byte,
                "found TL-B declaration candidate"
            );
            ranges.push(self.psi_file.range_of_symbol(symbol));
        }
        self.collect_block_ranges(search_name, &mut ranges);
        let ranges = dedup_ranges(ranges);
        tracing::trace!(
            target: logging::TLB_TARGET,
            operation = "tlb.reference.resolve",
            name = search_name,
            result_count = ranges.len(),
            "resolved TL-B reference"
        );
        ranges
    }

    fn declaration_name_definition_range(&self) -> Option<Range> {
        let parent = self.identifier.parent()?;
        if parent.kind() != "combinator" {
            return None;
        }
        let declaration_node = find_parent_of_kind(parent, "declaration")?;
        let name_node = declaration_name_node(tlb_syntax::Declaration(declaration_node))?;
        (name_node == self.identifier).then(|| self.psi_file.range_of(name_node))
    }

    fn collect_block_ranges(&self, search_name: &str, ranges: &mut Vec<Range>) {
        let Some(raw_declaration) = find_parent_of_kind(self.identifier, "declaration") else {
            return;
        };
        let declaration = tlb_syntax::Declaration(raw_declaration);

        if let Some(combinator) = declaration.combinator() {
            for parameter in combinator.params() {
                if let Some(param_name_node) = find_type_parameter_node(parameter.0) {
                    self.push_matching_node(search_name, param_name_node, ranges);
                }
            }
        }

        for node in PreorderTraverse::new(declaration.0.walk()) {
            if node.kind() != "combinator_expr" {
                continue;
            }

            let combinator_expr = tlb_syntax::CombinatorExpr(node);
            for parameter in combinator_expr.params() {
                if let Some(param_name_node) = find_type_parameter_node(parameter.syntax()) {
                    self.push_matching_node(search_name, param_name_node, ranges);
                }
            }
        }

        for field in declaration.fields() {
            let Some(value) = field.value() else {
                continue;
            };

            let name_node = match value {
                tlb_syntax::FieldKind::FieldNamed(field_named) => {
                    field_named.name().map(|name| name.0)
                }
                tlb_syntax::FieldKind::FieldBuiltin(field_builtin) => {
                    field_builtin.name().map(|name| name.0)
                }
                tlb_syntax::FieldKind::FieldCurlyExpr(_)
                | tlb_syntax::FieldKind::FieldAnonymous(_)
                | tlb_syntax::FieldKind::FieldExpr(_)
                | tlb_syntax::FieldKind::Unmapped(_) => None,
            };

            if let Some(name_node) = name_node {
                self.push_matching_node(search_name, name_node, ranges);
            }
        }
    }

    fn push_matching_node(&self, search_name: &str, node: Node<'_>, ranges: &mut Vec<Range>) {
        if node == self.identifier {
            return;
        }
        let Ok(name) = node.utf8_text(self.psi_file.source().as_bytes()) else {
            return;
        };
        if name.trim() == search_name {
            tracing::trace!(
                target: logging::TLB_TARGET,
                operation = "tlb.reference.candidate",
                name = search_name,
                kind = "local",
                start_byte = node.start_byte(),
                end_byte = node.end_byte(),
                "found TL-B local candidate"
            );
            ranges.push(self.psi_file.range_of(node));
        }
    }
}

fn declaration_name_node(declaration: tlb_syntax::Declaration<'_>) -> Option<Node<'_>> {
    declaration.combinator()?.name().map(|name| name.0)
}

fn find_reference_identifier(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            "identifier" | "type_identifier" => return Some(node),
            "field_named" | "field_builtin" | "constructor_" | "combinator" | "combinator_expr" => {
                if let Some(name) = node.child_by_field_name("name")
                    && matches!(name.kind(), "identifier" | "type_identifier")
                {
                    return Some(name);
                }
            }
            _ => {}
        }

        node = node.parent()?;
    }
}

fn find_parent_of_kind<'tree>(mut node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn find_type_parameter_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut result = None;
    for current in PreorderTraverse::new(node.walk()) {
        if current.kind() == "type_identifier" {
            result = Some(current);
        }
    }
    result
}

fn dedup_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
    ranges.sort_by_key(|range| {
        (
            range.start.line,
            range.start.character,
            range.end.line,
            range.end.character,
        )
    });
    ranges.dedup();
    ranges
}
