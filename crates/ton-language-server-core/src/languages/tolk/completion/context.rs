use crate::completion::identifier_prefix;
use crate::{DocumentSnapshot, Position, Range};
use tolk_syntax::{AstNode, HasName, TryFromNode};
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
    pub(super) fn new(document: &DocumentSnapshot, position: Position) -> anyhow::Result<Self> {
        let (prefix, replacement_range) = identifier_prefix(document, position);
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position)
            .min(document.text().len());
        let (left, right) = document.text().split_at(offset);
        let rewritten = format!("{left}{DUMMY_IDENTIFIER}{right}");
        let source_file = tolk_syntax::parse(&rewritten)?;
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
