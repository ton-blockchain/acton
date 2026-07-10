use fift_syntax::{AstNode, DefinitionKind, TopLevel};
use tree_sitter::Node;

pub(super) struct FiftReference<'tree> {
    source_file: &'tree fift_syntax::SourceFile,
    identifier: Node<'tree>,
}

impl<'tree> FiftReference<'tree> {
    pub(super) fn new(
        node: Node<'tree>,
        source_file: &'tree fift_syntax::SourceFile,
    ) -> Option<Self> {
        Some(Self {
            source_file,
            identifier: reference_identifier(node)?,
        })
    }

    pub(super) fn resolve(&self) -> Option<Node<'tree>> {
        let name = self
            .identifier
            .utf8_text(self.source_file.source.as_bytes())
            .ok()?
            .trim();
        if name.is_empty() {
            return None;
        }

        self.source_file.top_levels().find_map(|top_level| {
            let TopLevel::Definition(definition) = top_level else {
                return None;
            };
            let kind = definition.kind()?;
            let candidate = kind.name()?;
            (candidate.text(self.source_file.source.as_ref()).trim() == name)
                .then(|| candidate.syntax())
        })
    }
}

pub(super) fn reference_identifier(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            "identifier" => return Some(node),
            "proc_call" | "instruction" => {
                let identifier = node.named_child(0)?;
                if identifier.kind() == "identifier" {
                    return Some(identifier);
                }
            }
            _ => {}
        }

        node = node.parent()?;
    }
}

pub(super) fn is_definition_name(parent: Node<'_>, node: Node<'_>) -> bool {
    parent.child_by_field_name("name") == Some(node)
        && matches!(
            parent.kind(),
            "proc_definition"
                | "proc_inline_definition"
                | "proc_ref_definition"
                | "method_definition"
                | "proc_declaration"
                | "method_declaration"
                | "declaration"
        )
}

pub(super) fn definition_parent(node: Node<'_>) -> Option<Node<'_>> {
    let parent = node.parent()?;
    matches!(
        DefinitionKind::from(parent),
        DefinitionKind::ProcDefinition(_)
            | DefinitionKind::ProcInlineDefinition(_)
            | DefinitionKind::ProcRefDefinition(_)
            | DefinitionKind::MethodDefinition(_)
    )
    .then_some(parent)
}
