use crate::ast::expressions::{
    ArrayElementType, ArrayMultiplier, ArrayType, BinaryExpression, BinaryNumberLit,
    BinaryRightExpr, BitSizeExpr, BitSizeValue, BuiltinExpr, BuiltinField, BuiltinOneArg,
    BuiltinZeroArgs, CellRefExpr, CellRefInner, CellRefInnerValue, CellRefTarget, CombinatorExpr,
    Comment, CompareExpr, CondDotAndQuestionExpr, CondDotted, CondDottedValue, CondExpr,
    CondQuestionExpr, CondTypeExpr, CurlyExpression, HexLit, Identifier, NegateExpr, NumberLit,
    ParensCellRef, ParensCompareExpr, ParensCondDotted, ParensExpr, ParensTypeExpr, RefExpr,
    RefExprValue, RefInner, RefInnerValue, SimpleExpr, TypeExpr, TypeIdentifier, TypeParameter,
};
use crate::ast::node::SourceFile;
use crate::ast::top_level::{
    Combinator, Constructor, ConstructorTag, Declaration, Field, FieldAnonRef, FieldAnonymous,
    FieldAnonymousKind, FieldBuiltin, FieldCurlyExpr, FieldExpr, FieldKind, FieldNamed,
    FieldNamedAnonRef, Program, TopLevel,
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

    fn visit_simple_expr(&mut self, expr: &SimpleExpr<'tree>) -> Self::Result {
        self.walk_simple_expr(expr)
    }

    fn visit_cond_expr(&mut self, expr: &CondExpr<'tree>) -> Self::Result {
        self.walk_cond_expr(expr)
    }

    fn visit_type_expr(&mut self, expr: &TypeExpr<'tree>) -> Self::Result {
        self.walk_type_expr(expr)
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        if let Some(program) = file.program() {
            self.walk_program(&program);
        }
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
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_declaration(&mut self, node: &Declaration<'tree>) -> Self::Result {
        if let Some(constructor) = node.constructor() {
            self.walk_constructor(&constructor);
        }

        for field in node.fields() {
            self.walk_field(&field);
        }

        if let Some(combinator) = node.combinator() {
            self.walk_combinator(&combinator);
        }

        self.default_result()
    }

    fn walk_constructor(&mut self, node: &Constructor<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_identifier(&name);
        }

        if let Some(tag) = node.tag() {
            self.walk_constructor_tag(&tag);
        }

        self.default_result()
    }

    fn walk_constructor_tag(&mut self, tag: &ConstructorTag<'tree>) -> Self::Result {
        match tag {
            ConstructorTag::Identifier(node) => self.walk_identifier(node),
            ConstructorTag::BinaryNumber(node) => self.walk_binary_number_lit(node),
            ConstructorTag::Hex(node) => self.walk_hex_lit(node),
            ConstructorTag::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_combinator(&mut self, node: &Combinator<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_type_identifier(&name);
        }

        for param in node.params() {
            self.walk_type_parameter(&param);
        }

        self.default_result()
    }

    fn walk_type_parameter(&mut self, node: &TypeParameter<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_simple_expr(&expr);
        }
        self.default_result()
    }

    fn walk_field(&mut self, node: &Field<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_field_kind(&value);
        }
        self.default_result()
    }

    fn walk_field_kind(&mut self, kind: &FieldKind<'tree>) -> Self::Result {
        match kind {
            FieldKind::FieldBuiltin(node) => self.walk_field_builtin(node),
            FieldKind::FieldCurlyExpr(node) => self.walk_field_curly_expr(node),
            FieldKind::FieldAnonymous(node) => self.walk_field_anonymous(node),
            FieldKind::FieldNamed(node) => self.walk_field_named(node),
            FieldKind::FieldExpr(node) => self.walk_field_expr(node),
            FieldKind::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_field_builtin(&mut self, node: &FieldBuiltin<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_identifier(&name);
        }

        if let Some(field) = node.field() {
            self.walk_builtin_field(&field);
        }

        self.default_result()
    }

    fn walk_field_curly_expr(&mut self, node: &FieldCurlyExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_curly_expression(&expr);
        }
        self.default_result()
    }

    fn walk_field_anonymous(&mut self, node: &FieldAnonymous<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_field_anonymous_kind(&value);
        }
        self.default_result()
    }

    fn walk_field_anonymous_kind(&mut self, kind: &FieldAnonymousKind<'tree>) -> Self::Result {
        match kind {
            FieldAnonymousKind::FieldAnonRef(node) => self.walk_field_anon_ref(node),
            FieldAnonymousKind::FieldNamedAnonRef(node) => self.walk_field_named_anon_ref(node),
            FieldAnonymousKind::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_field_anon_ref(&mut self, node: &FieldAnonRef<'tree>) -> Self::Result {
        for field in node.fields() {
            self.walk_field(&field);
        }
        self.default_result()
    }

    fn walk_field_named_anon_ref(&mut self, node: &FieldNamedAnonRef<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_identifier(&name);
        }
        if let Some(anon_ref) = node.anon_ref() {
            self.walk_field_anon_ref(&anon_ref);
        }
        self.default_result()
    }

    fn walk_field_named(&mut self, node: &FieldNamed<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_identifier(&name);
        }
        if let Some(expr) = node.expr() {
            self.visit_cond_expr(&expr);
        }
        self.default_result()
    }

    fn walk_field_expr(&mut self, node: &FieldExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_cond_expr(&expr);
        }
        self.default_result()
    }

    fn walk_simple_expr(&mut self, expr: &SimpleExpr<'tree>) -> Self::Result {
        match expr {
            SimpleExpr::NegateExpr(node) => self.walk_negate_expr(node),
            SimpleExpr::BinaryExpression(node) => self.walk_binary_expression(node),
            SimpleExpr::RefExpr(node) => self.walk_ref_expr(node),
            SimpleExpr::ParensExpr(node) => self.walk_parens_expr(node),
            SimpleExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_negate_expr(&mut self, node: &NegateExpr<'tree>) -> Self::Result {
        if let Some(operand) = node.operand() {
            self.visit_simple_expr(&operand);
        }
        self.default_result()
    }

    fn walk_binary_expression(&mut self, node: &BinaryExpression<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_simple_expr(&left);
        }

        if let Some(right) = node.right() {
            self.walk_binary_right_expr(&right);
        }

        self.default_result()
    }

    fn walk_binary_right_expr(&mut self, right: &BinaryRightExpr<'tree>) -> Self::Result {
        match right {
            BinaryRightExpr::SimpleExpr(node) => self.walk_simple_expr(node),
            BinaryRightExpr::BitSizeExpr(node) => self.walk_bit_size_expr(node),
            BinaryRightExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_ref_expr(&mut self, node: &RefExpr<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_ref_expr_value(&value);
        }
        self.default_result()
    }

    fn walk_ref_expr_value(&mut self, value: &RefExprValue<'tree>) -> Self::Result {
        match value {
            RefExprValue::RefInner(node) => self.walk_ref_inner(node),
            RefExprValue::ParensExpr(node) => self.walk_parens_expr(node),
            RefExprValue::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_ref_inner(&mut self, node: &RefInner<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_ref_inner_value(&value);
        }
        self.default_result()
    }

    fn walk_ref_inner_value(&mut self, value: &RefInnerValue<'tree>) -> Self::Result {
        match value {
            RefInnerValue::TypeIdentifier(node) => self.walk_type_identifier(node),
            RefInnerValue::Number(node) => self.walk_number_lit(node),
            RefInnerValue::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_parens_expr(&mut self, node: &ParensExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_simple_expr(&expr);
        }
        self.default_result()
    }

    fn walk_cond_expr(&mut self, expr: &CondExpr<'tree>) -> Self::Result {
        match expr {
            CondExpr::CondDotAndQuestionExpr(node) => self.walk_cond_dot_and_question_expr(node),
            CondExpr::CondQuestionExpr(node) => self.walk_cond_question_expr(node),
            CondExpr::CondTypeExpr(node) => self.walk_cond_type_expr(node),
            CondExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_cond_dot_and_question_expr(
        &mut self,
        node: &CondDotAndQuestionExpr<'tree>,
    ) -> Self::Result {
        if let Some(dotted) = node.dotted() {
            self.walk_cond_dotted_value(&dotted);
        }

        if let Some(expr) = node.expr() {
            self.visit_type_expr(&expr);
        }

        self.default_result()
    }

    fn walk_cond_dotted_value(&mut self, value: &CondDottedValue<'tree>) -> Self::Result {
        match value {
            CondDottedValue::CondDotted(node) => self.walk_cond_dotted(node),
            CondDottedValue::ParensCondDotted(node) => self.walk_parens_cond_dotted(node),
            CondDottedValue::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_cond_dotted(&mut self, node: &CondDotted<'tree>) -> Self::Result {
        if let Some(base) = node.base() {
            self.visit_type_expr(&base);
        }

        if let Some(number) = node.number() {
            self.walk_number_lit(&number);
        }

        self.default_result()
    }

    fn walk_parens_cond_dotted(&mut self, node: &ParensCondDotted<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_cond_dotted(&expr);
        }
        self.default_result()
    }

    fn walk_cond_question_expr(&mut self, node: &CondQuestionExpr<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_type_expr(&left);
        }

        if let Some(right) = node.right() {
            self.visit_type_expr(&right);
        }

        self.default_result()
    }

    fn walk_cond_type_expr(&mut self, node: &CondTypeExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_type_expr(&expr);
        }
        self.default_result()
    }

    fn walk_type_expr(&mut self, expr: &TypeExpr<'tree>) -> Self::Result {
        match expr {
            TypeExpr::CellRefExpr(node) => self.walk_cell_ref_expr(node),
            TypeExpr::BuiltinExpr(node) => self.walk_builtin_expr(node),
            TypeExpr::CombinatorExpr(node) => self.walk_combinator_expr(node),
            TypeExpr::SimpleExpr(node) => self.walk_simple_expr(node),
            TypeExpr::ArrayType(node) => self.walk_array_type(node),
            TypeExpr::ArrayMultiplier(node) => self.walk_array_multiplier(node),
            TypeExpr::BitSizeExpr(node) => self.walk_bit_size_expr(node),
            TypeExpr::ParensTypeExpr(node) => self.walk_parens_type_expr(node),
            TypeExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_cell_ref_expr(&mut self, node: &CellRefExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_cell_ref_target(&expr);
        }
        self.default_result()
    }

    fn walk_cell_ref_target(&mut self, target: &CellRefTarget<'tree>) -> Self::Result {
        match target {
            CellRefTarget::CellRefInner(node) => self.walk_cell_ref_inner(node),
            CellRefTarget::ParensCellRef(node) => self.walk_parens_cell_ref(node),
            CellRefTarget::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_cell_ref_inner(&mut self, node: &CellRefInner<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_cell_ref_inner_value(&value);
        }
        self.default_result()
    }

    fn walk_cell_ref_inner_value(&mut self, value: &CellRefInnerValue<'tree>) -> Self::Result {
        match value {
            CellRefInnerValue::CombinatorExpr(node) => self.walk_combinator_expr(node),
            CellRefInnerValue::TypeIdentifier(node) => self.walk_type_identifier(node),
            CellRefInnerValue::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_parens_cell_ref(&mut self, node: &ParensCellRef<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_cell_ref_inner(&expr);
        }
        self.default_result()
    }

    fn walk_builtin_expr(&mut self, expr: &BuiltinExpr<'tree>) -> Self::Result {
        match expr {
            BuiltinExpr::BuiltinOneArg(node) => self.walk_builtin_one_arg(node),
            BuiltinExpr::BuiltinZeroArgs(node) => self.walk_builtin_zero_args(node),
            BuiltinExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_builtin_one_arg(&mut self, node: &BuiltinOneArg<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_ref_expr(&expr);
        }
        self.default_result()
    }

    fn walk_builtin_zero_args(&mut self, _node: &BuiltinZeroArgs<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_combinator_expr(&mut self, node: &CombinatorExpr<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_type_identifier(&name);
        }

        for param in node.params() {
            self.visit_type_expr(&param);
        }

        self.default_result()
    }

    fn walk_parens_type_expr(&mut self, node: &ParensTypeExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_type_expr(&expr);
        }
        self.default_result()
    }

    fn walk_array_type(&mut self, node: &ArrayType<'tree>) -> Self::Result {
        if let Some(element_type) = node.element_type() {
            self.walk_array_element_type(&element_type);
        }
        self.default_result()
    }

    fn walk_array_element_type(&mut self, node: &ArrayElementType<'tree>) -> Self::Result {
        match node {
            ArrayElementType::TypeIdentifier(node) => self.walk_type_identifier(node),
            ArrayElementType::TypeExpr(node) => self.walk_type_expr(node),
            ArrayElementType::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_array_multiplier(&mut self, node: &ArrayMultiplier<'tree>) -> Self::Result {
        if let Some(size) = node.size() {
            self.walk_simple_expr(&size);
        }

        if let Some(ty) = node.ty() {
            self.walk_array_type(&ty);
        }

        self.default_result()
    }

    fn walk_bit_size_expr(&mut self, node: &BitSizeExpr<'tree>) -> Self::Result {
        if let Some(size) = node.size() {
            self.walk_bit_size_value(&size);
        }
        self.default_result()
    }

    fn walk_bit_size_value(&mut self, value: &BitSizeValue<'tree>) -> Self::Result {
        match value {
            BitSizeValue::Number(node) => self.walk_number_lit(node),
            BitSizeValue::ParensExpr(node) => self.walk_parens_expr(node),
            BitSizeValue::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_curly_expression(&mut self, expr: &CurlyExpression<'tree>) -> Self::Result {
        match expr {
            CurlyExpression::CompareExpr(node) => self.walk_compare_expr(node),
            CurlyExpression::Identifier(node) => self.walk_identifier(node),
            CurlyExpression::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_compare_expr(&mut self, expr: &CompareExpr<'tree>) -> Self::Result {
        match expr {
            CompareExpr::BinaryExpression(node) => self.walk_binary_expression(node),
            CompareExpr::ParensCompareExpr(node) => self.walk_parens_compare_expr(node),
            CompareExpr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_parens_compare_expr(&mut self, node: &ParensCompareExpr<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_compare_expr(&expr);
        }
        self.default_result()
    }

    fn walk_identifier(&mut self, _node: &Identifier<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_type_identifier(&mut self, _node: &TypeIdentifier<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_number_lit(&mut self, _node: &NumberLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_binary_number_lit(&mut self, _node: &BinaryNumberLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_hex_lit(&mut self, _node: &HexLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_comment(&mut self, _node: &Comment<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_builtin_field(&mut self, _node: &BuiltinField<'tree>) -> Self::Result {
        self.default_result()
    }
}
