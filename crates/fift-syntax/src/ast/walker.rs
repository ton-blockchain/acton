use crate::ast::expressions::{
    HexLit, Ident, IfJmpStatement, IfStatement, Instruction, InstructionBlock, InstructionExpr,
    NegativeIdent, NumberLit, ProcCall, RepeatStatement, SliceLit, StackIndex, StackOp, StackRef,
    StringLit, UntilStatement, WhileStatement,
};
use crate::ast::node::SourceFile;
use crate::ast::top_level::{
    Declaration, DeclarationKind, Definition, DefinitionKind, GlobalVar, IncludeDirective,
    MethodDeclaration, MethodDefinition, ProcDeclaration, ProcDefinition, ProcInlineDefinition,
    ProcRefDefinition, Program, TopLevel,
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

    fn visit_instruction_expr(&mut self, expr: &InstructionExpr<'tree>) -> Self::Result {
        self.walk_instruction_expr(expr)
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        if let Some(include) = file.include_directive() {
            self.walk_include_directive(&include);
        }
        if let Some(program) = file.program() {
            self.walk_program(&program);
        }
        self.default_result()
    }

    fn walk_include_directive(&mut self, _node: &IncludeDirective<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_program(&mut self, node: &Program<'tree>) -> Self::Result {
        for top_level in node.items() {
            self.visit_top_level(&top_level);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        match top_level {
            TopLevel::Declaration(node) => self.walk_declaration(node),
            TopLevel::Definition(node) => self.walk_definition(node),
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_declaration(&mut self, node: &Declaration<'tree>) -> Self::Result {
        if let Some(kind) = node.kind() {
            self.walk_declaration_kind(&kind);
        }
        self.default_result()
    }

    fn walk_declaration_kind(&mut self, kind: &DeclarationKind<'tree>) -> Self::Result {
        match kind {
            DeclarationKind::ProcDeclaration(node) => self.walk_proc_declaration(node),
            DeclarationKind::MethodDeclaration(node) => self.walk_method_declaration(node),
            DeclarationKind::GlobalVar(node) => self.walk_global_var(node),
            DeclarationKind::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_proc_declaration(&mut self, node: &ProcDeclaration<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        self.default_result()
    }

    fn walk_method_declaration(&mut self, node: &MethodDeclaration<'tree>) -> Self::Result {
        if let Some(id) = node.id() {
            self.walk_number_lit(&id);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        self.default_result()
    }

    fn walk_global_var(&mut self, node: &GlobalVar<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        self.default_result()
    }

    fn walk_definition(&mut self, node: &Definition<'tree>) -> Self::Result {
        if let Some(kind) = node.kind() {
            self.walk_definition_kind(&kind);
        }
        self.default_result()
    }

    fn walk_definition_kind(&mut self, kind: &DefinitionKind<'tree>) -> Self::Result {
        match kind {
            DefinitionKind::ProcDefinition(node) => self.walk_proc_definition(node),
            DefinitionKind::ProcInlineDefinition(node) => self.walk_proc_inline_definition(node),
            DefinitionKind::ProcRefDefinition(node) => self.walk_proc_ref_definition(node),
            DefinitionKind::MethodDefinition(node) => self.walk_method_definition(node),
            DefinitionKind::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_proc_definition(&mut self, node: &ProcDefinition<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_proc_inline_definition(&mut self, node: &ProcInlineDefinition<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_proc_ref_definition(&mut self, node: &ProcRefDefinition<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_method_definition(&mut self, node: &MethodDefinition<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_instruction(&mut self, node: &Instruction<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.visit_instruction_expr(&value);
        }
        self.default_result()
    }

    fn walk_instruction_expr(&mut self, expr: &InstructionExpr<'tree>) -> Self::Result {
        match expr {
            InstructionExpr::Identifier(node) => self.walk_ident(node),
            InstructionExpr::NegativeIdentifier(node) => self.walk_negative_ident(node),
            InstructionExpr::Number(node) => self.walk_number_lit(node),
            InstructionExpr::String(node) => self.walk_string_lit(node),
            InstructionExpr::IfStatement(node) => self.walk_if_statement(node),
            InstructionExpr::IfJmpStatement(node) => self.walk_if_jmp_statement(node),
            InstructionExpr::WhileStatement(node) => self.walk_while_statement(node),
            InstructionExpr::RepeatStatement(node) => self.walk_repeat_statement(node),
            InstructionExpr::UntilStatement(node) => self.walk_until_statement(node),
            InstructionExpr::ProcCall(node) => self.walk_proc_call(node),
            InstructionExpr::SliceLiteral(node) => self.walk_slice_lit(node),
            InstructionExpr::HexLiteral(node) => self.walk_hex_lit(node),
            InstructionExpr::StackRef(node) => self.walk_stack_ref(node),
            InstructionExpr::StackOp(node) => self.walk_stack_op(node),
            InstructionExpr::InstructionBlock(node) => self.walk_instruction_block(node),
            InstructionExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_negative_ident(&mut self, node: &NegativeIdent<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_ident(&value);
        }
        self.default_result()
    }

    fn walk_if_statement(&mut self, node: &IfStatement<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_if_jmp_statement(&mut self, node: &IfJmpStatement<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_while_statement(&mut self, node: &WhileStatement<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_repeat_statement(&mut self, node: &RepeatStatement<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_until_statement(&mut self, node: &UntilStatement<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_instruction_block(&mut self, node: &InstructionBlock<'tree>) -> Self::Result {
        for instruction in node.instructions() {
            self.walk_instruction(&instruction);
        }
        self.default_result()
    }

    fn walk_proc_call(&mut self, node: &ProcCall<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        self.default_result()
    }

    fn walk_stack_op(&mut self, node: &StackOp<'tree>) -> Self::Result {
        for index in node.stack_indices() {
            self.walk_stack_index(&index);
        }
        for reference in node.stack_refs() {
            self.walk_stack_ref(&reference);
        }
        if let Some(operation) = node.operation() {
            self.walk_ident(&operation);
        }
        self.default_result()
    }

    fn walk_ident(&mut self, _node: &Ident<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_number_lit(&mut self, _node: &NumberLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_string_lit(&mut self, _node: &StringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_slice_lit(&mut self, _node: &SliceLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_hex_lit(&mut self, _node: &HexLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_stack_ref(&mut self, _node: &StackRef<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_stack_index(&mut self, _node: &StackIndex<'tree>) -> Self::Result {
        self.default_result()
    }
}

/// Finds the nearest parent node matching the provided tree-sitter kind.
#[must_use]
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
