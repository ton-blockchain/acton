use fift_syntax::{AstNode, DefinitionKind, TopLevel};
use ton_syntax::ast::PreorderTraverse;
use tree_sitter::Node;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct FiftReference<'tree> {
    source_file: &'tree fift_syntax::SourceFile,
    identifier: Node<'tree>,
}

#[allow(dead_code)]
impl<'tree> FiftReference<'tree> {
    pub fn new(node: Node<'tree>, source_file: &'tree fift_syntax::SourceFile) -> Option<Self> {
        let identifier = find_reference_identifier(node)?;
        Some(Self {
            source_file,
            identifier,
        })
    }

    pub fn identifier(&self) -> Node<'tree> {
        self.identifier
    }

    pub fn name(&self) -> Option<&'tree str> {
        let source = self.source_file.source.as_ref();
        self.identifier
            .utf8_text(source.as_bytes())
            .ok()
            .map(str::trim)
    }

    pub fn resolve(&self) -> Option<Node<'tree>> {
        let target_name = self.name()?;
        if target_name.is_empty() {
            return None;
        }

        let source = self.source_file.source.as_ref();
        for top_level in self.source_file.top_levels() {
            let TopLevel::Definition(definition) = top_level else {
                continue;
            };

            let Some(kind) = definition.kind() else {
                continue;
            };
            let Some(name) = kind.name() else {
                continue;
            };
            if name.text(source).trim() != target_name {
                continue;
            }

            let definition_node = match kind {
                DefinitionKind::ProcDefinition(node) => node.syntax(),
                DefinitionKind::ProcInlineDefinition(node) => node.syntax(),
                DefinitionKind::ProcRefDefinition(node) => node.syntax(),
                DefinitionKind::MethodDefinition(node) => node.syntax(),
                DefinitionKind::Unmapped(_) => continue,
            };
            return Some(definition_node);
        }

        None
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct FiftReferent<'tree> {
    node: Node<'tree>,
    source_file: &'tree fift_syntax::SourceFile,
    resolved: Option<Node<'tree>>,
}

#[allow(dead_code)]
impl<'tree> FiftReferent<'tree> {
    pub fn new(node: Node<'tree>, source_file: &'tree fift_syntax::SourceFile) -> Self {
        let resolved =
            FiftReference::new(node, source_file).and_then(|reference| reference.resolve());
        Self {
            node,
            source_file,
            resolved,
        }
    }

    pub fn resolved(&self) -> Option<Node<'tree>> {
        self.resolved
    }

    pub fn find_references(&self, include_definition: bool) -> Vec<Node<'tree>> {
        let Some(resolved_definition) = self.resolved else {
            return Vec::new();
        };

        let source = self.source_file.source.as_ref();
        let definition_name_node = resolved_definition
            .child_by_field_name("name")
            .unwrap_or(resolved_definition);
        let Ok(word) = definition_name_node.utf8_text(source.as_bytes()) else {
            return Vec::new();
        };
        let word = word.trim();
        if word.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        if include_definition {
            result.push(resolved_definition);
        }

        for node in PreorderTraverse::new(self.source_file.root_node().walk()) {
            if !node.is_named() {
                continue;
            }

            if node.kind() != "identifier" {
                continue;
            }

            let Ok(text) = node.utf8_text(source.as_bytes()) else {
                continue;
            };
            if text.trim() != word {
                continue;
            }

            let Some(parent) = node.parent() else {
                continue;
            };

            if is_definition_name(parent, node) {
                continue;
            }

            let Some(definition) = FiftReference::new(node, self.source_file)
                .and_then(|reference| reference.resolve())
            else {
                continue;
            };

            if definition == resolved_definition {
                result.push(node);
            }
        }

        result
    }
}

#[allow(dead_code)]
fn find_reference_identifier(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            "identifier" => return Some(node),
            "proc_call" => {
                let ident = node.named_child(0)?;
                if ident.kind() == "identifier" {
                    return Some(ident);
                }
            }
            "instruction" => {
                let ident = node.named_child(0)?;
                if ident.kind() == "identifier" {
                    return Some(ident);
                }
            }
            _ => {}
        }

        node = node.parent()?;
    }
}

fn is_definition_name(parent: Node<'_>, node: Node<'_>) -> bool {
    if parent.child_by_field_name("name") != Some(node) {
        return false;
    }

    matches!(
        parent.kind(),
        "proc_definition"
            | "proc_inline_definition"
            | "proc_ref_definition"
            | "method_definition"
            | "declaration"
    )
}
