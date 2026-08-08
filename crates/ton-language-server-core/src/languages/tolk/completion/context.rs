use super::contract::{ContractFieldValueKind, contract_field};
use crate::completion::identifier_prefix;
use crate::{DocumentSnapshot, Position, Range};
use tolk_syntax::{AstNode, Block, ContractField, ExprStmt, HasName, TryFromNode};
use tree_sitter::Node;

pub(super) const DUMMY_IDENTIFIER: &str = "DummyIdentifier";

#[derive(Debug)]
pub(super) struct TolkCompletionContext {
    source_file: tolk_syntax::SourceFile,
    pub(super) offset: usize,
    pub(super) prefix: String,
    pub(super) replacement_range: Range,
    pub(super) after_dot: bool,
    pub(super) before_paren: bool,
    pub(super) before_semicolon: bool,
}

impl TolkCompletionContext {
    pub(super) fn new(
        document: &DocumentSnapshot,
        parsed: &tolk_syntax::SourceFile,
        position: Position,
    ) -> anyhow::Result<Self> {
        let (prefix, replacement_range) = identifier_prefix(document, position);
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position)
            .min(document.text().len());
        let insertion_position = document
            .text_index()
            .offset_to_position(document.text(), offset);
        let insertion_range = Range::new(insertion_position, insertion_position);
        let mut rewritten = document.text().to_owned();
        let input_edit =
            document
                .text_index()
                .apply_edit(&mut rewritten, insertion_range, DUMMY_IDENTIFIER)?;
        let mut old_tree = parsed.tree.clone();
        old_tree.edit(&input_edit);
        let source_file = tolk_syntax::parse_with_old_tree(&rewritten, Some(&old_tree))?;
        let (prefix, replacement_range) = if let Some(backtick_start) = document.text().as_bytes()
            [..offset]
            .iter()
            .rposition(|byte| *byte == b'`')
            && backtick_start + 1 < offset
            && !document.text()[backtick_start + 1..offset].contains('\n')
        {
            let backtick_end = document.text().as_bytes()[offset..]
                .iter()
                .position(|byte| *byte == b'`')
                .map_or(offset, |relative| offset + relative + 1);
            (
                document.text()[backtick_start + 1..offset].to_owned(),
                document.text_index().range_for_offsets(
                    document.text(),
                    backtick_start,
                    backtick_end,
                ),
            )
        } else {
            (prefix.to_owned(), replacement_range)
        };
        let prefix_start = document
            .text_index()
            .position_to_offset(document.text(), replacement_range.start);
        let after_dot = document.text().as_bytes()[..prefix_start]
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .is_some_and(|index| document.text().as_bytes()[index] == b'.');
        let replacement_end = document
            .text_index()
            .position_to_offset(document.text(), replacement_range.end);
        let suffix = &document.text()[replacement_end..];
        let next = suffix.trim_start();

        Ok(Self {
            source_file,
            offset,
            prefix,
            replacement_range,
            after_dot,
            before_paren: next.starts_with('('),
            before_semicolon: next.starts_with(';'),
        })
    }

    pub(super) fn source(&self) -> &str {
        self.source_file.source.as_ref()
    }

    pub(super) const fn source_file(&self) -> &tolk_syntax::SourceFile {
        &self.source_file
    }

    pub(super) fn text(&self) -> &str {
        self.source()
    }

    pub(super) fn text_of<'tree, N>(&self, node: N) -> &str
    where
        N: AstNode<'tree>,
    {
        node.syntax()
            .utf8_text(self.text().as_bytes())
            .unwrap_or("")
    }

    pub(super) fn root(&self) -> Node<'_> {
        self.source_file.tree.root_node()
    }

    pub(super) fn cursor_node(&self) -> Option<Node<'_>> {
        let dummy_end = self.offset.checked_add(DUMMY_IDENTIFIER.len())?;
        let mut node = self
            .root()
            .descendant_for_byte_range(self.offset, dummy_end)?;
        loop {
            if matches!(
                node.kind(),
                "identifier" | "type_identifier" | "annotation_name" | "string_literal"
            ) {
                return Some(node);
            }
            node = node.parent()?;
        }
    }

    pub(super) fn is_type(&self) -> bool {
        if let Some(value_kind) = self.contract_field_value_kind() {
            return value_kind == ContractFieldValueKind::Type;
        }

        let Some(mut node) = self.cursor_node() else {
            return false;
        };
        loop {
            if node.kind() == "type_identifier" || node.kind().ends_with("_type") {
                return true;
            }
            if matches!(
                node.kind(),
                "expression_statement"
                    | "block_statement"
                    | "source_file"
                    | "function_call"
                    | "dot_access"
            ) {
                return false;
            }
            let Some(parent) = node.parent() else {
                return false;
            };
            node = parent;
        }
    }

    pub(super) fn inside_import(&self) -> bool {
        self.has_ancestor("import_directive")
    }

    pub(super) fn inside_string(&self) -> bool {
        self.cursor_node()
            .is_some_and(|node| node.kind() == "string_literal")
    }

    pub(super) fn is_annotation_name(&self) -> bool {
        self.has_ancestor("annotation")
    }

    pub(super) fn top_level(&self) -> bool {
        !self.has_any_ancestor(&[
            "block_statement",
            "match_expression",
            "match_body",
            "struct_body",
            "enum_body",
            "contract_body",
            "contract_declaration",
            "constant_declaration",
            "enum_declaration",
            "function_declaration",
            "get_method_declaration",
            "global_var_declaration",
            "import_directive",
            "method_declaration",
            "struct_declaration",
            "type_alias_declaration",
        ])
    }

    pub(super) fn struct_top_level(&self) -> bool {
        self.has_ancestor("struct_body") && !self.has_ancestor("block_statement")
    }

    pub(super) fn enum_top_level(&self) -> bool {
        self.has_ancestor("enum_body") && !self.has_ancestor("block_statement")
    }

    pub(super) fn contract_top_level(&self) -> bool {
        self.has_ancestor("contract_body") && !self.has_ancestor("block_statement")
    }

    pub(super) fn in_contract_field_value(&self) -> bool {
        self.contract_field_value().is_some()
    }

    pub(super) fn is_statement(&self) -> bool {
        let Some(mut node) = self.cursor_node() else {
            return false;
        };
        loop {
            if node.kind() == "expression_statement" {
                return has_ancestor(node, "block_statement");
            }
            if node.kind() == "block_statement" {
                return false;
            }
            let Some(parent) = node.parent() else {
                return false;
            };
            node = parent;
        }
    }

    pub(super) fn needs_semicolon_for_call(&self) -> bool {
        if self.before_semicolon || self.before_paren {
            return false;
        }

        self.cursor_node().is_some_and(needs_semicolon_for_call)
    }

    pub(super) fn expression(&self) -> bool {
        self.has_ancestor("block_statement")
            && !self.after_dot
            && !self.is_type()
            && !self.in_name_of_field_init()
            && !self.inside_import()
            && !self.expect_match_arm()
            && !self.is_declaration_name()
            && !self.is_annotation_name()
            && !self.inside_string()
    }

    pub(super) fn is_declaration_name(&self) -> bool {
        self.cursor_node()
            .is_some_and(tolk_syntax::is_declaration_name_node)
    }

    pub(super) fn is_function_name(&self) -> bool {
        self.cursor_node().is_some_and(|node| {
            node.parent().is_some_and(|parent| {
                matches!(parent.kind(), "function_declaration" | "method_declaration")
                    && tolk_syntax::is_declaration_name_node(node)
            })
        })
    }

    pub(super) fn expect_field_modifier(&self) -> bool {
        let Some(node) = self.cursor_node() else {
            return false;
        };
        if !self.struct_top_level() {
            return false;
        }
        node.parent().is_some_and(|parent| {
            (parent.kind() == "struct_field_declaration"
                && tolk_syntax::is_declaration_name_node(node))
                || matches!(parent.kind(), "ERROR" | "struct_body")
        })
    }

    pub(super) fn expect_match_arm(&self) -> bool {
        let Some(mut node) = self.cursor_node() else {
            return false;
        };
        loop {
            match node.kind() {
                "match_body" => return true,
                "match_arm" | "block_statement" => return false,
                _ => {}
            }
            let Some(parent) = node.parent() else {
                return false;
            };
            node = parent;
        }
    }

    pub(super) fn in_name_of_field_init(&self) -> bool {
        let Some(node) = self.cursor_node() else {
            return false;
        };
        node.parent().is_some_and(|parent| {
            tolk_syntax::InstanceArg::try_from_node(parent)
                .ok()
                .and_then(|argument| argument.name())
                .is_some_and(|name| name.syntax() == node)
        })
    }

    pub(super) fn in_field_init_value(&self) -> bool {
        let Some(node) = self.cursor_node() else {
            return false;
        };
        node.parent().is_some_and(|parent| {
            tolk_syntax::InstanceArg::try_from_node(parent)
                .ok()
                .and_then(|argument| argument.value())
                .is_some_and(|value| value.syntax() == node)
        })
    }

    pub(super) fn is_catch_variable(&self) -> bool {
        let Some(node) = self.cursor_node() else {
            return false;
        };
        node.parent()
            .is_some_and(|parent| parent.kind() == "catch_clause")
    }

    pub(super) fn in_multiline_struct_init(&self) -> bool {
        self.ancestor("object_literal")
            .is_some_and(|node| node.start_position().row != node.end_position().row)
    }

    pub(super) fn ancestor(&self, kind: &str) -> Option<Node<'_>> {
        let mut node = self.cursor_node()?;
        loop {
            if node.kind() == kind {
                return Some(node);
            }
            node = node.parent()?;
        }
    }

    pub(super) fn ancestor_as<'tree, T>(&'tree self) -> Option<T>
    where
        T: TryFromNode<'tree> + tolk_syntax::HasTreeSitterKind,
    {
        let node = self.ancestor(T::TREE_SITTER_KIND)?;
        T::try_from_node(node).ok()
    }

    pub(super) fn ancestor_base_function(&self) -> Option<tolk_syntax::BaseFunction<'_>> {
        let mut node = self.cursor_node()?;
        loop {
            if let Ok(function) = tolk_syntax::BaseFunction::try_from_node(node) {
                return Some(function);
            }
            node = node.parent()?;
        }
    }

    pub(super) fn parent_as<'tree, T>(&'tree self) -> Option<T>
    where
        T: TryFromNode<'tree>,
    {
        let node = self.cursor_node()?.parent()?;
        T::try_from_node(node).ok()
    }

    pub(super) fn string_literal(&self) -> Option<tolk_syntax::StringLit<'_>> {
        self.ancestor_as::<tolk_syntax::StringLit>()
    }

    pub(super) fn annotation_owner(&self) -> Option<tolk_syntax::AnnotatedDeclaration<'_>> {
        let mut node = self.cursor_node()?;

        loop {
            if let Ok(owner) = tolk_syntax::AnnotatedDeclaration::try_from_node(node) {
                return Some(owner);
            }

            node = node.parent()?;
        }
    }

    fn has_ancestor(&self, kind: &str) -> bool {
        self.ancestor(kind).is_some()
    }

    fn has_any_ancestor(&self, kinds: &[&str]) -> bool {
        let Some(mut node) = self.cursor_node() else {
            return false;
        };
        loop {
            if kinds.contains(&node.kind()) {
                return true;
            }
            let Some(parent) = node.parent() else {
                return false;
            };
            node = parent;
        }
    }

    fn contract_field_value_kind(&self) -> Option<ContractFieldValueKind> {
        let field = self.contract_field_value()?;
        let name = field.name()?;

        contract_field(self.text_of(name)).map(|field| field.value_kind)
    }

    fn contract_field_value(&self) -> Option<ContractField<'_>> {
        let cursor = self.cursor_node()?;
        let field = self.ancestor_as::<ContractField>()?;
        let value = field.value()?.syntax();

        (value.start_byte() <= cursor.start_byte() && cursor.end_byte() <= value.end_byte())
            .then_some(field)
    }
}

fn has_ancestor(mut node: Node<'_>, kind: &str) -> bool {
    loop {
        if node.kind() == kind {
            return true;
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

fn needs_semicolon_for_call(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if let Ok(access) = tolk_syntax::DotAccess::try_from_node(parent) {
        if access.obj().is_some_and(|object| object.syntax() == node) {
            return false;
        }

        return needs_semicolon_for_call(parent);
    }

    if ExprStmt::try_from_node(parent).is_ok() {
        return true;
    }

    if let Ok(declaration) = tolk_syntax::VarDeclLhs::try_from_node(parent) {
        return parent
            .parent()
            .is_some_and(|grandparent| Block::try_from_node(grandparent).is_ok())
            && declaration
                .assigned_value()
                .is_some_and(|value| value.syntax() == node);
    }

    if let Ok(assignment) = tolk_syntax::Assign::try_from_node(parent) {
        return parent
            .parent()
            .is_some_and(|grandparent| ExprStmt::try_from_node(grandparent).is_ok())
            && assignment
                .right()
                .is_some_and(|right| right.syntax() == node);
    }

    if let Ok(assignment) = tolk_syntax::SetAssign::try_from_node(parent) {
        return parent
            .parent()
            .is_some_and(|grandparent| ExprStmt::try_from_node(grandparent).is_ok())
            && assignment
                .right()
                .is_some_and(|right| right.syntax() == node);
    }

    false
}
