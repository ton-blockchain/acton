use tlb_syntax::TopLevel;
use ton_syntax::ast::PreorderTraverse;
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TlbNamedItemKind {
    Declaration,
    NamedField,
    Parameter,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TlbNamedItem<'tree> {
    pub(crate) kind: TlbNamedItemKind,
    pub(crate) node: Node<'tree>,
    pub(crate) owner: Option<Node<'tree>>,
}

impl<'tree> TlbNamedItem<'tree> {
    pub(crate) fn name(&self, source: &'tree str) -> Option<&'tree str> {
        self.node.utf8_text(source.as_bytes()).ok().map(str::trim)
    }

    pub(crate) fn owner_name(&self, source: &'tree str) -> Option<&'tree str> {
        let owner = self.owner?;
        owner.utf8_text(source.as_bytes()).ok().map(str::trim)
    }
}

pub(crate) trait ScopeProcessor<'tree> {
    fn execute(&mut self, item: TlbNamedItem<'tree>) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TlbReference<'tree> {
    source_file: &'tree tlb_syntax::SourceFile,
    identifier: Node<'tree>,
}

impl<'tree> TlbReference<'tree> {
    pub(crate) fn new(
        node: Node<'tree>,
        source_file: &'tree tlb_syntax::SourceFile,
    ) -> Option<Self> {
        let identifier = find_reference_identifier(node)?;
        Some(Self {
            source_file,
            identifier,
        })
    }

    pub(crate) fn name(&self) -> Option<&'tree str> {
        let source = self.source_file.source.as_ref();
        self.identifier
            .utf8_text(source.as_bytes())
            .ok()
            .map(str::trim)
    }

    pub(crate) fn resolve(&self) -> Option<TlbNamedItem<'tree>> {
        self.multi_resolve().into_iter().next()
    }

    pub(crate) fn multi_resolve(&self) -> Vec<TlbNamedItem<'tree>> {
        let source = self.source_file.source.as_ref();
        let Some(search_name) = self.name() else {
            return Vec::new();
        };

        let mut collector = ResolveCollector {
            source,
            search_name,
            target: self.identifier,
            result: Vec::new(),
        };

        self.process_resolve_variants(&mut collector);
        collector.result
    }

    pub(crate) fn process_resolve_variants(
        &self,
        processor: &mut dyn ScopeProcessor<'tree>,
    ) -> bool {
        if let Some(parent) = self.identifier.parent()
            && parent.kind() == "combinator"
            && let Some(declaration_node) = find_parent_of_kind(parent, "declaration")
            && let Some(name_node) =
                declaration_name_node(tlb_syntax::Declaration(declaration_node))
        {
            return processor.execute(TlbNamedItem {
                kind: TlbNamedItemKind::Declaration,
                node: name_node,
                owner: None,
            });
        }

        if find_parent_of_kind(self.identifier, "type_parameter").is_some() {
            return true;
        }

        for top_level in self.source_file.top_levels() {
            let TopLevel::Declaration(declaration) = top_level else {
                continue;
            };

            let Some(name_node) = declaration_name_node(declaration) else {
                continue;
            };

            if !processor.execute(TlbNamedItem {
                kind: TlbNamedItemKind::Declaration,
                node: name_node,
                owner: None,
            }) {
                return false;
            }
        }

        self.process_block(processor)
    }

    fn process_block(&self, processor: &mut dyn ScopeProcessor<'tree>) -> bool {
        let Some(raw_declaration) = find_parent_of_kind(self.identifier, "declaration") else {
            return true;
        };
        let declaration = tlb_syntax::Declaration(raw_declaration);

        if let Some(combinator) = declaration.combinator() {
            let owner = combinator.name().map(|name| name.0);
            for parameter in combinator.params() {
                let Some(param_name_node) = find_type_parameter_node(parameter.0) else {
                    continue;
                };

                if !processor.execute(TlbNamedItem {
                    kind: TlbNamedItemKind::Parameter,
                    node: param_name_node,
                    owner,
                }) {
                    return false;
                }
            }
        }

        for node in PreorderTraverse::new(declaration.0.walk()) {
            if node.kind() != "combinator_expr" {
                continue;
            }

            let combinator_expr = tlb_syntax::CombinatorExpr(node);
            let owner = combinator_expr.name().map(|name| name.0);
            for parameter in combinator_expr.params() {
                let Some(param_name_node) = find_type_parameter_node(parameter.syntax()) else {
                    continue;
                };

                if !processor.execute(TlbNamedItem {
                    kind: TlbNamedItemKind::Parameter,
                    node: param_name_node,
                    owner,
                }) {
                    return false;
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

            let Some(name_node) = name_node else {
                continue;
            };

            if !processor.execute(TlbNamedItem {
                kind: TlbNamedItemKind::NamedField,
                node: name_node,
                owner: None,
            }) {
                return false;
            }
        }

        true
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TlbReferent<'tree> {
    source_file: &'tree tlb_syntax::SourceFile,
    resolved: Option<TlbNamedItem<'tree>>,
}

impl<'tree> TlbReferent<'tree> {
    pub(crate) fn new(node: Node<'tree>, source_file: &'tree tlb_syntax::SourceFile) -> Self {
        let resolved =
            TlbReference::new(node, source_file).and_then(|reference| reference.resolve());
        Self {
            source_file,
            resolved,
        }
    }

    pub(crate) fn resolved(&self) -> Option<TlbNamedItem<'tree>> {
        self.resolved
    }

    pub(crate) fn find_references(&self, include_definition: bool) -> Vec<Node<'tree>> {
        let Some(resolved) = self.resolved else {
            return Vec::new();
        };

        let source = self.source_file.source.as_ref();
        let Some(target_name) = resolved.name(source) else {
            return Vec::new();
        };
        if target_name.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        if include_definition {
            result.push(resolved.node);
        }

        for node in PreorderTraverse::new(self.source_file.root_node().walk()) {
            if !node.is_named() {
                continue;
            }
            if !matches!(node.kind(), "identifier" | "type_identifier") {
                continue;
            }

            let Ok(text) = node.utf8_text(source.as_bytes()) else {
                continue;
            };
            if text.trim() != target_name {
                continue;
            }

            let Some(parent) = node.parent() else {
                continue;
            };
            if parent.kind() == "combinator" {
                continue;
            }

            let Some(reference) = TlbReference::new(node, self.source_file) else {
                continue;
            };
            if reference
                .multi_resolve()
                .into_iter()
                .any(|item| matches_resolved(item, resolved, target_name, source))
            {
                result.push(node);
            }
        }

        result
    }
}

struct ResolveCollector<'tree> {
    source: &'tree str,
    search_name: &'tree str,
    target: Node<'tree>,
    result: Vec<TlbNamedItem<'tree>>,
}

impl<'tree> ScopeProcessor<'tree> for ResolveCollector<'tree> {
    fn execute(&mut self, item: TlbNamedItem<'tree>) -> bool {
        if item.node == self.target {
            return true;
        }

        let Some(name) = item.name(self.source) else {
            return true;
        };
        if name != self.search_name {
            return true;
        }

        self.result.push(item);
        true
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

fn matches_resolved(
    candidate: TlbNamedItem<'_>,
    resolved: TlbNamedItem<'_>,
    target_name: &str,
    source: &str,
) -> bool {
    if candidate.node.kind() != resolved.node.kind() {
        return false;
    }

    if candidate.node.start_position().row != resolved.node.start_position().row {
        return false;
    }

    let Some(identifier_text) = candidate.name(source) else {
        return false;
    };

    identifier_text == target_name || identifier_text == "self"
}
