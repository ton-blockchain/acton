use crate::expressions::*;
use crate::node::*;
use crate::statements::*;
use crate::top_level::*;
use crate::types::*;
use tree_sitter::Node;

pub trait Walker<'tree> {
    type Result;

    fn visit_source_file(&mut self, source_file: &'tree SourceFile) -> Self::Result {
        self.walk_source_file(source_file)
    }

    fn visit_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        self.walk_top_level(top_level)
    }

    fn visit_statement(&mut self, statement: &Statement<'tree>) -> Self::Result {
        self.walk_statement(statement)
    }

    fn visit_expression(&mut self, expression: &Expression<'tree>) -> Self::Result {
        self.walk_expression(expression)
    }

    fn visit_type(&mut self, typ: &Type<'tree>) -> Self::Result {
        self.walk_type(typ)
    }

    fn walk_source_file(&mut self, source_file: &'tree SourceFile) -> Self::Result {
        for top_level in &source_file.top_levels() {
            self.visit_top_level(top_level);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        match top_level {
            TopLevel::TolkRequiredVersion(node) => self.walk_tolk_required_version(node),
            TopLevel::Import(node) => self.walk_import(node),
            TopLevel::GlobalVarDeclaration(node) => self.walk_global_var_declaration(node),
            TopLevel::ConstantDeclaration(node) => self.walk_constant_declaration(node),
            TopLevel::TypeAliasDeclaration(node) => self.walk_type_alias_declaration(node),
            TopLevel::StructDeclaration(node) => self.walk_struct_declaration(node),
            TopLevel::EnumDeclaration(node) => self.walk_enum_declaration(node),
            TopLevel::Function(node) => self.walk_function(node),
            TopLevel::MethodDeclaration(node) => self.walk_method_declaration(node),
            TopLevel::GetMethodDeclaration(node) => self.walk_get_method_declaration(node),
            TopLevel::EmptyStatement(node) => self.walk_empty_statement(node),
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_statement(&mut self, statement: &Statement<'tree>) -> Self::Result {
        match statement {
            Statement::BlockStatement(node) => self.walk_block_statement(node),
            Statement::IfStatement(node) => self.walk_if_statement(node),
            Statement::WhileStatement(node) => self.walk_while_statement(node),
            Statement::RepeatStatement(node) => self.walk_repeat_statement(node),
            Statement::TryCatchStatement(node) => self.walk_try_catch_statement(node),
            Statement::ReturnStatement(node) => self.walk_return_statement(node),
            Statement::LocalVarsDeclaration(node) => self.walk_local_vars_declaration(node),
            Statement::DoWhileStatement(node) => self.walk_do_while_statement(node),
            Statement::BreakStatement(node) => self.walk_break_statement(node),
            Statement::ContinueStatement(node) => self.walk_continue_statement(node),
            Statement::ThrowStatement(node) => self.walk_throw_statement(node),
            Statement::AssertStatement(node) => self.walk_assert_statement(node),
            Statement::MatchStatement(node) => self.walk_match_statement(node),
            Statement::EmptyStatement(node) => self.walk_empty_statement(node),
            Statement::ExpressionStatement(node) => self.walk_expression_statement(node),
            Statement::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_expression(&mut self, expression: &Expression<'tree>) -> Self::Result {
        match expression {
            Expression::Assignment(node) => self.walk_assignment(node),
            Expression::SetAssignment(node) => self.walk_set_assignment(node),
            Expression::TernaryOperator(node) => self.walk_ternary_operator(node),
            Expression::BinaryOperator(node) => self.walk_binary_operator(node),
            Expression::UnaryOperator(node) => self.walk_unary_operator(node),
            Expression::LazyExpression(node) => self.walk_lazy_expression(node),
            Expression::CastAsOperator(node) => self.walk_cast_as_operator(node),
            Expression::IsTypeOperator(node) => self.walk_is_type_operator(node),
            Expression::NotNullOperator(node) => self.walk_not_null_operator(node),
            Expression::DotAccess(node) => self.walk_dot_access(node),
            Expression::FunctionCall(node) => self.walk_function_call(node),
            Expression::GenericInstantiation(node) => self.walk_generic_instantiation(node),
            Expression::ParenthesizedExpression(node) => self.walk_parenthesized_expression(node),
            Expression::MatchExpression(node) => self.walk_match_expression(node),
            Expression::ObjectLiteral(node) => self.walk_object_literal(node),
            Expression::TensorExpression(node) => self.walk_tensor_expression(node),
            Expression::TypedTuple(node) => self.walk_typed_tuple(node),
            Expression::LambdaExpression(node) => self.walk_lambda_expression(node),
            Expression::NumberLiteral(node) => self.walk_number_literal(node),
            Expression::StringLiteral(node) => self.walk_string_literal(node),
            Expression::BooleanLiteral(node) => self.walk_boolean_literal(node),
            Expression::NullLiteral(node) => self.walk_null_literal(node),
            Expression::Underscore(node) => self.walk_underscore(node),
            Expression::Ident(node) => self.walk_ident(node),
            Expression::NumericIndex(node) => self.walk_numeric_index(node),
            Expression::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_type(&mut self, typ: &Type<'tree>) -> Self::Result {
        match typ {
            Type::TypeIdentifier(node) => self.walk_type_identifier(node),
            Type::TypeInstantiatedTs(node) => self.walk_type_instantiated_ts(node),
            Type::TensorType(node) => self.walk_tensor_type(node),
            Type::TupleType(node) => self.walk_tuple_type(node),
            Type::ParenthesizedType(node) => self.walk_parenthesized_type(node),
            Type::FunCallableType(node) => self.walk_fun_callable_type(node),
            Type::NullableType(node) => self.walk_nullable_type(node),
            Type::UnionType(node) => self.walk_union_type(node),
            Type::NullLiteral(node) => self.walk_null_literal_type(node),
            Type::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_tolk_required_version(&mut self, _node: &TolkRequiredVersion<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_import(&mut self, _node: &Import<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_global_var_declaration(&mut self, node: &GlobalVarDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_constant_declaration(&mut self, node: &ConstantDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(value) = node.value() {
            self.visit_expression(&value);
        }
        self.default_result()
    }

    fn walk_type_alias_declaration(&mut self, node: &TypeAliasDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(underlying_type) = node.underlying_type() {
            self.walk_type_alias_underlying_type(&underlying_type);
        }
        self.default_result()
    }

    fn walk_struct_declaration(&mut self, node: &StructDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(body) = node.body() {
            self.walk_struct_body(&body);
        }
        self.default_result()
    }

    fn walk_enum_declaration(&mut self, node: &EnumDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(backed_type) = node.backed_type() {
            self.visit_type(&backed_type);
        }
        if let Some(body) = node.body() {
            self.walk_enum_body(&body);
        }
        self.default_result()
    }

    fn walk_function(&mut self, node: &Function<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_method_declaration(&mut self, node: &MethodDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(receiver) = node.receiver() {
            self.walk_method_receiver(&receiver);
        }
        if let Some(type_params) = node.type_parameters() {
            self.walk_type_parameters(&type_params);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters("", false) {
            self.walk_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_get_method_declaration(&mut self, node: &GetMethodDeclaration<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_empty_statement(&mut self, _node: &EmptyStatement<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_block_statement(&mut self, node: &BlockStatement<'tree>) -> Self::Result {
        for stmt in node.statements() {
            self.visit_statement(&stmt);
        }
        self.default_result()
    }

    fn walk_if_statement(&mut self, node: &IfStatement<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expression(&condition);
        }
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        if let Some(alternative) = node.alternative() {
            match alternative {
                IfStatementAlternative::IfStatement(if_stmt) => {
                    self.walk_if_statement(&if_stmt);
                }
                IfStatementAlternative::BlockStatement(block) => {
                    self.walk_block_statement(&block);
                }
            }
        }
        self.default_result()
    }

    fn walk_while_statement(&mut self, node: &WhileStatement<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expression(&condition);
        }
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        self.default_result()
    }

    fn walk_repeat_statement(&mut self, node: &RepeatStatement<'tree>) -> Self::Result {
        if let Some(count) = node.count() {
            self.visit_expression(&count);
        }
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        self.default_result()
    }

    fn walk_try_catch_statement(&mut self, node: &TryCatchStatement<'tree>) -> Self::Result {
        if let Some(try_body) = node.body() {
            self.walk_block_statement(&try_body);
        }
        if let Some(catch) = node.catch() {
            self.walk_catch_clause(&catch);
        }
        self.default_result()
    }

    fn walk_return_statement(&mut self, node: &ReturnStatement<'tree>) -> Self::Result {
        if let Some(body) = node.expr() {
            self.visit_expression(&body);
        }
        self.default_result()
    }

    fn walk_local_vars_declaration(&mut self, node: &LocalVarsDeclaration<'tree>) -> Self::Result {
        if let Some(lhs) = node.lhs() {
            self.walk_var_declaration_lhs(&lhs);
        }
        if let Some(assigned_val) = node.assigned_val() {
            self.visit_expression(&assigned_val);
        }
        self.default_result()
    }

    fn walk_do_while_statement(&mut self, node: &DoWhileStatement<'tree>) -> Self::Result {
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        if let Some(condition) = node.condition() {
            self.visit_expression(&condition);
        }
        self.default_result()
    }

    fn walk_break_statement(&mut self, _node: &BreakStatement<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_continue_statement(&mut self, _node: &ContinueStatement<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_throw_statement(&mut self, node: &ThrowStatement<'tree>) -> Self::Result {
        if let Some(expression) = node.expression() {
            self.visit_expression(&expression);
        }
        self.default_result()
    }

    fn walk_assert_statement(&mut self, node: &AssertStatement<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expression(&condition);
        }
        if let Some(expression) = node.expression() {
            self.visit_expression(&expression);
        }
        self.default_result()
    }

    fn walk_match_statement(&mut self, node: &MatchStatement<'tree>) -> Self::Result {
        self.walk_match_expression(&MatchExpression(node.0))
    }

    fn walk_expression_statement(&mut self, node: &ExpressionStatement<'tree>) -> Self::Result {
        if let Some(expr) = node.expression() {
            self.visit_expression(&expr);
        }
        self.default_result()
    }

    fn walk_assignment(&mut self, node: &Assignment<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expression(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expression(&right);
        }
        self.default_result()
    }

    fn walk_set_assignment(&mut self, node: &SetAssignment<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expression(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expression(&right);
        }
        self.default_result()
    }

    fn walk_ternary_operator(&mut self, node: &TernaryOperator<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expression(&condition);
        }
        if let Some(consequence) = node.consequence() {
            self.visit_expression(&consequence);
        }
        if let Some(alternative) = node.alternative() {
            self.visit_expression(&alternative);
        }
        self.default_result()
    }

    fn walk_binary_operator(&mut self, node: &BinaryOperator<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expression(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expression(&right);
        }
        self.default_result()
    }

    fn walk_unary_operator(&mut self, node: &UnaryOperator<'tree>) -> Self::Result {
        if let Some(argument) = node.argument() {
            self.visit_expression(&argument);
        }
        self.default_result()
    }

    fn walk_lazy_expression(&mut self, node: &LazyExpression<'tree>) -> Self::Result {
        if let Some(argument) = node.expr() {
            self.visit_expression(&argument);
        }
        self.default_result()
    }

    fn walk_cast_as_operator(&mut self, node: &CastAsOperator<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expression(&expr);
        }
        if let Some(casted_to) = node.casted_to() {
            self.visit_type(&casted_to);
        }
        self.default_result()
    }

    fn walk_is_type_operator(&mut self, node: &IsTypeOperator<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expression(&expr);
        }
        if let Some(rhs_type) = node.rhs_type() {
            self.visit_type(&rhs_type);
        }
        self.default_result()
    }

    fn walk_not_null_operator(&mut self, node: &NotNullOperator<'tree>) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_expression(&inner);
        }
        self.default_result()
    }

    fn walk_dot_access(&mut self, node: &DotAccess<'tree>) -> Self::Result {
        if let Some(obj) = node.obj() {
            self.visit_expression(&obj);
        }
        self.default_result()
    }

    fn walk_function_call(&mut self, node: &FunctionCall<'tree>) -> Self::Result {
        if let Some(callee) = node.callee() {
            self.visit_expression(&callee);
        }
        for arg in node.arguments() {
            self.walk_call_argument(&arg);
        }
        self.default_result()
    }

    fn walk_generic_instantiation(&mut self, node: &GenericInstantiation<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expression(&expr);
        }
        if let Some(instantiation_ts) = node.instantiation_ts() {
            self.walk_instantiation_t_list(&instantiation_ts);
        }
        self.default_result()
    }

    fn walk_parenthesized_expression(
        &mut self,
        node: &ParenthesizedExpression<'tree>,
    ) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_expression(&inner);
        }
        self.default_result()
    }

    fn walk_match_expression(&mut self, node: &MatchExpression<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            match expr {
                MatchExpr::Expression(ref e) => self.visit_expression(e),
                MatchExpr::LocalVarsDeclaration(ref lvd) => self.walk_local_vars_declaration(lvd),
            };
        }
        if let Some(body) = node.body() {
            self.walk_match_body(&body);
        }
        self.default_result()
    }

    fn walk_object_literal(&mut self, node: &ObjectLiteral<'tree>) -> Self::Result {
        if let Some(object_type) = node.typ() {
            self.visit_type(&object_type);
        }
        for arg in node.arguments() {
            self.walk_instance_argument(&arg);
        }
        self.default_result()
    }

    fn walk_tensor_expression(&mut self, node: &TensorExpression<'tree>) -> Self::Result {
        for element in node.elements() {
            self.walk_expression(&element);
        }
        self.default_result()
    }

    fn walk_typed_tuple(&mut self, node: &TypedTuple<'tree>) -> Self::Result {
        for element in node.elements() {
            self.walk_expression(&element);
        }
        self.default_result()
    }

    fn walk_lambda_expression(&mut self, node: &LambdaExpression<'tree>) -> Self::Result {
        for param in node.parameters() {
            self.walk_lambda_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        self.default_result()
    }

    fn walk_lambda_parameter(&mut self, node: &LambdaParameter<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_number_literal(&mut self, _node: &NumberLiteral<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_string_literal(&mut self, _node: &StringLiteral<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_boolean_literal(&mut self, _node: &BooleanLiteral<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_null_literal(&mut self, _node: &NullLiteral<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_underscore(&mut self, _node: &Underscore<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_ident(&mut self, _node: &Ident<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_numeric_index(&mut self, _node: &NumericIndex<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_type_identifier(&mut self, _node: &TypeIdentifier<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_type_instantiated_ts(&mut self, node: &TypeInstantiatedTs<'tree>) -> Self::Result {
        if let Some(arguments) = node.arguments() {
            self.walk_instantiation_t_list(&arguments);
        }
        self.default_result()
    }

    fn walk_tensor_type(&mut self, node: &TensorType<'tree>) -> Self::Result {
        for element_type in node.element_types() {
            self.visit_type(&element_type);
        }
        self.default_result()
    }

    fn walk_tuple_type(&mut self, node: &TupleType<'tree>) -> Self::Result {
        for element_type in node.element_types() {
            self.visit_type(&element_type);
        }
        self.default_result()
    }

    fn walk_parenthesized_type(&mut self, node: &ParenthesizedType<'tree>) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_type(&inner);
        }
        self.default_result()
    }

    fn walk_fun_callable_type(&mut self, node: &FunCallableType<'tree>) -> Self::Result {
        if let Some(param_types) = node.param_types() {
            self.visit_type(&param_types);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        self.default_result()
    }

    fn walk_nullable_type(&mut self, node: &NullableType<'tree>) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_type(&inner);
        }
        self.default_result()
    }

    fn walk_union_type(&mut self, node: &UnionType<'tree>) -> Self::Result {
        if let Some(lhs) = node.lhs() {
            self.visit_type(&lhs);
        }
        if let Some(rhs) = node.rhs() {
            self.visit_type(&rhs);
        }
        self.default_result()
    }

    fn walk_null_literal_type(&mut self, node: &NullLiteral<'tree>) -> Self::Result {
        self.walk_null_literal(node)
    }

    fn walk_annotation_list(&mut self, node: &AnnotationList<'tree>) -> Self::Result {
        for annotation in node.annotations() {
            self.walk_annotation(&annotation);
        }
        self.default_result()
    }

    fn walk_annotation(&mut self, node: &Annotation<'tree>) -> Self::Result {
        if let Some(arguments) = node.arguments() {
            self.walk_annotation_arguments(&arguments);
        }
        self.default_result()
    }

    fn walk_annotation_arguments(&mut self, node: &AnnotationArguments<'tree>) -> Self::Result {
        for arg in node.arguments() {
            self.visit_expression(&arg);
        }
        self.default_result()
    }

    fn walk_type_parameters(&mut self, node: &TypeParameters<'tree>) -> Self::Result {
        for param in node.parameters() {
            self.walk_type_parameter(&param);
        }
        self.default_result()
    }

    fn walk_type_parameter(&mut self, node: &TypeParameter<'tree>) -> Self::Result {
        if let Some(default) = node.default() {
            self.visit_type(&default);
        }
        self.default_result()
    }

    fn walk_parameter(&mut self, node: &Parameter<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(default) = node.default() {
            self.visit_expression(&default);
        }
        self.default_result()
    }

    fn walk_function_body(&mut self, body: &FunctionBody<'tree>) -> Self::Result {
        match body {
            FunctionBody::BlockStatement(block) => self.walk_block_statement(block),
            FunctionBody::AsmBody(asm) => self.walk_asm_body(asm),
            FunctionBody::BuiltinSpecifier(_) => self.default_result(),
            FunctionBody::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_type_alias_underlying_type(
        &mut self,
        node: &TypeAliasUnderlyingType<'tree>,
    ) -> Self::Result {
        match node {
            TypeAliasUnderlyingType::Type(typ) => self.visit_type(typ),
            TypeAliasUnderlyingType::BuiltinSpecifier(_) => self.default_result(),
        }
    }

    fn walk_struct_body(&mut self, node: &StructBody<'tree>) -> Self::Result {
        for field in node.fields() {
            self.walk_struct_field_declaration(&field);
        }
        self.default_result()
    }

    fn walk_struct_field_declaration(
        &mut self,
        node: &StructFieldDeclaration<'tree>,
    ) -> Self::Result {
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(default) = node.default() {
            self.visit_expression(&default);
        }
        self.default_result()
    }

    fn walk_enum_body(&mut self, node: &EnumBody<'tree>) -> Self::Result {
        for member in node.members() {
            self.walk_enum_member_declaration(&member);
        }
        self.default_result()
    }

    fn walk_enum_member_declaration(
        &mut self,
        node: &EnumMemberDeclaration<'tree>,
    ) -> Self::Result {
        if let Some(default) = node.default() {
            self.visit_expression(&default);
        }
        self.default_result()
    }

    fn walk_method_receiver(&mut self, node: &MethodReceiver<'tree>) -> Self::Result {
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_catch_clause(&mut self, node: &CatchClause<'tree>) -> Self::Result {
        if let Some(body) = node.body() {
            self.walk_block_statement(&body);
        }
        self.default_result()
    }

    fn walk_var_declaration_lhs(&mut self, lhs: &VarDeclarationLhs<'tree>) -> Self::Result {
        match lhs {
            VarDeclarationLhs::TupleVarsDeclaration(tuple) => {
                self.walk_tuple_vars_declaration(tuple)
            }
            VarDeclarationLhs::TensorVarsDeclaration(tensor) => {
                self.walk_tensor_vars_declaration(tensor)
            }
            VarDeclarationLhs::VarDeclaration(var) => self.walk_var_declaration(var),
        }
    }

    fn walk_tuple_vars_declaration(&mut self, node: &TupleVarsDeclaration<'tree>) -> Self::Result {
        for var in node.vars() {
            self.walk_var_declaration_lhs(&var);
        }
        self.default_result()
    }

    fn walk_tensor_vars_declaration(
        &mut self,
        node: &TensorVarsDeclaration<'tree>,
    ) -> Self::Result {
        for var in node.vars() {
            self.walk_var_declaration_lhs(&var);
        }
        self.default_result()
    }

    fn walk_var_declaration(&mut self, node: &VarDeclaration<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_match_body(&mut self, node: &MatchBody<'tree>) -> Self::Result {
        for arm in node.arms() {
            self.walk_match_arm(&arm);
        }
        self.default_result()
    }

    fn walk_match_arm(&mut self, node: &MatchArm<'tree>) -> Self::Result {
        match node.pattern() {
            MatchPattern::Type(pattern) => self.walk_type(&pattern),
            MatchPattern::Expression(pattern) => self.walk_expression(&pattern),
            _ => self.default_result(),
        };

        if let Some(body) = node.body() {
            match body {
                MatchArmBody::BlockStatement(ref block) => self.walk_block_statement(block),
                MatchArmBody::ReturnStatement(ref ret) => self.walk_return_statement(ret),
                MatchArmBody::ThrowStatement(ref throw) => self.walk_throw_statement(throw),
                MatchArmBody::Expression(ref expr) => self.visit_expression(expr),
            };
        }
        self.default_result()
    }

    fn walk_call_argument(&mut self, node: &CallArgument<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expression(&expr);
        }
        self.default_result()
    }

    fn walk_instantiation_t_list(&mut self, node: &InstantiationTList<'tree>) -> Self::Result {
        for typ in node.types() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_instance_argument(&mut self, node: &InstanceArgument<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.visit_expression(&value);
        }
        self.default_result()
    }

    fn walk_asm_body(&mut self, _node: &AsmBody<'tree>) -> Self::Result {
        self.default_result()
    }

    fn default_result(&self) -> Self::Result;
}

pub fn walk_ast<'tree, W: Walker<'tree>>(
    visitor: &mut W,
    source_file: &'tree SourceFile,
) -> W::Result {
    visitor.visit_source_file(source_file)
}

pub fn parent_of_type<'a>(node: &'a Node<'a>, target_kind: &str) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == target_kind {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}
