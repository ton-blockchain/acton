use crate::ast::expressions::{
    Argument, BinLit, BocLit, Code, ControlRegister, DataLit, DataLiteral, Dictionary,
    DictionaryEntry, Expr, HexLit, Ident, IntegerLit, StackElement, StringLit,
};
use crate::ast::node::SourceFile;
use crate::ast::top_level::{
    DefaultExotic, EmbedSlice, Exotic, ExoticLib, ExoticLibrary, ExplicitRef, Instruction, TopLevel,
};

pub trait Walker<'tree> {
    type Result;

    fn default_result(&self) -> Self::Result;

    fn visit_source_file(&mut self, source_file: &'tree SourceFile) -> Self::Result {
        self.walk_source_file(source_file)
    }

    fn visit_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        self.walk_top_level(top_level)
    }

    fn visit_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        self.walk_expr(expr)
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        for top_level in file.top_levels() {
            self.visit_top_level(&top_level);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        match top_level {
            TopLevel::Instruction(node) => self.walk_instruction(node),
            TopLevel::ExplicitRef(node) => self.walk_explicit_ref(node),
            TopLevel::EmbedSlice(node) => self.walk_embed_slice(node),
            TopLevel::Exotic(node) => self.walk_exotic(node),
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_instruction(&mut self, node: &Instruction<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for arg in node.args() {
            self.walk_argument(&arg);
        }
        self.default_result()
    }

    fn walk_explicit_ref(&mut self, node: &ExplicitRef<'tree>) -> Self::Result {
        if let Some(code) = node.code() {
            self.walk_code(&code);
        }
        self.default_result()
    }

    fn walk_embed_slice(&mut self, node: &EmbedSlice<'tree>) -> Self::Result {
        if let Some(data) = node.data() {
            self.walk_data_literal(&data);
        }
        self.default_result()
    }

    fn walk_exotic(&mut self, node: &Exotic<'tree>) -> Self::Result {
        if let Some(lib) = node.lib() {
            self.walk_exotic_lib(&lib);
        }
        self.default_result()
    }

    fn walk_exotic_lib(&mut self, lib: &ExoticLib<'tree>) -> Self::Result {
        match lib {
            ExoticLib::ExoticLibrary(node) => self.walk_exotic_library(node),
            ExoticLib::DefaultExotic(node) => self.walk_default_exotic(node),
            ExoticLib::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_exotic_library(&mut self, node: &ExoticLibrary<'tree>) -> Self::Result {
        if let Some(data) = node.data() {
            self.walk_data_literal(&data);
        }
        self.default_result()
    }

    fn walk_default_exotic(&mut self, node: &DefaultExotic<'tree>) -> Self::Result {
        if let Some(data) = node.data() {
            self.walk_data_literal(&data);
        }
        self.default_result()
    }

    fn walk_argument(&mut self, arg: &Argument<'tree>) -> Self::Result {
        if let Some(expr) = arg.expr() {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        match expr {
            Expr::IntegerLit(node) => self.walk_integer_lit(node),
            Expr::DataLiteral(node) => self.walk_data_literal(node),
            Expr::Code(node) => self.walk_code(node),
            Expr::Dictionary(node) => self.walk_dictionary(node),
            Expr::StackElement(node) => self.walk_stack_element(node),
            Expr::ControlRegister(node) => self.walk_control_register(node),
            Expr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_data_literal(&mut self, node: &DataLiteral<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_data_lit(&value);
        }
        self.default_result()
    }

    fn walk_data_lit(&mut self, value: &DataLit<'tree>) -> Self::Result {
        match value {
            DataLit::Hex(node) => self.walk_hex_lit(node),
            DataLit::Bin(node) => self.walk_bin_lit(node),
            DataLit::Boc(node) => self.walk_boc_lit(node),
            DataLit::String(node) => self.walk_string_lit(node),
            DataLit::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_code(&mut self, node: &Code<'tree>) -> Self::Result {
        if let Some(instructions) = node.instructions() {
            for top_level in instructions.items() {
                self.visit_top_level(&top_level);
            }
        }
        self.default_result()
    }

    fn walk_dictionary(&mut self, node: &Dictionary<'tree>) -> Self::Result {
        for entry in node.entries() {
            self.walk_dictionary_entry(&entry);
        }
        self.default_result()
    }

    fn walk_dictionary_entry(&mut self, node: &DictionaryEntry<'tree>) -> Self::Result {
        if let Some(id) = node.id() {
            self.walk_integer_lit(&id);
        }
        if let Some(code) = node.code() {
            self.walk_code(&code);
        }
        self.default_result()
    }

    fn walk_ident(&mut self, _node: &Ident<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_integer_lit(&mut self, _node: &IntegerLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_hex_lit(&mut self, _node: &HexLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_bin_lit(&mut self, _node: &BinLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_boc_lit(&mut self, _node: &BocLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_string_lit(&mut self, _node: &StringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_stack_element(&mut self, _node: &StackElement<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_control_register(&mut self, _node: &ControlRegister<'tree>) -> Self::Result {
        self.default_result()
    }
}

/// Finds the nearest parent node matching the provided tree-sitter kind.
pub fn find_parent_by_kind<'a>(
    node: &'a tree_sitter::Node<'a>,
    target_kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == target_kind {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}
