use crate::AstNodeBytesKind;
use crate::ast::expressions::{
    AsCast, Assign, Bin, BoolLit, Call, CallArgument, DotAccess, DotAccessField, Expr, Ident,
    InstanceArg, Instantiation, IsType, Lambda, LambdaParameter, Lazy, Match, MatchArm,
    MatchArmBody, MatchBody, MatchPattern, NotNull, NullLit, NumberLit, NumericIndex, ObjectLit,
    Paren, SetAssign, StringLit, Tensor, TensorVars, Ternary, Tuple, TupleVars, Unary, Underscore,
    VarDecl, VarDeclLhs, VarDeclPattern,
};
use crate::ast::node::SourceFile;
use crate::ast::statements::{
    Assert, Block, Break, CatchClause, Continue, DoWhile, ExprStmt, If, IfAlt, MatchStmt, Repeat,
    Return, Stmt, Throw, TryCatch, While,
};
use crate::ast::top_level::{
    Annotation, AnnotationArgs, AnnotationList, AsmBody, Constant, EmptyStmt, Enum, EnumBody,
    EnumMember, Func, FuncBody, GetMethod, GlobalVar, Import, Method, MethodReceiver, Parameter,
    Struct, StructBody, StructField, TolkRequiredVersion, TopLevel, TypeAlias,
    TypeAliasUnderlyingType, TypeParameter, TypeParameters,
};
use crate::ast::traits::{FunctionLike, HasAnnotations, HasGenericParams, HasName};
use crate::ast::types::{
    FunCallableType, InstantiationTList, NullableType, ParenthesizedType, TensorType, TupleType,
    Type, TypeIdent, TypeInstantiatedTs, UnionType,
};
use tree_sitter::Node;

pub trait Walker<'tree> {
    type Result;

    fn visit_source_file(&mut self, source_file: &'tree SourceFile) -> Self::Result {
        self.walk_source_file(source_file)
    }

    fn visit_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        self.walk_top_level(top_level)
    }

    fn visit_stmt(&mut self, stmt: &Stmt<'tree>) -> Self::Result {
        self.walk_stmt(stmt)
    }

    fn visit_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        self.walk_expr(expr)
    }

    fn visit_type(&mut self, typ: &Type<'tree>) -> Self::Result {
        self.walk_type(typ)
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        for top_level in file.top_levels() {
            self.visit_top_level(&top_level);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, top_level: &TopLevel<'tree>) -> Self::Result {
        match top_level {
            TopLevel::TolkRequiredVersion(node) => self.walk_tolk_required_version(node),
            TopLevel::Import(node) => self.walk_import(node),
            TopLevel::GlobalVar(node) => self.walk_global_var(node),
            TopLevel::Constant(node) => self.walk_constant(node),
            TopLevel::TypeAlias(node) => self.walk_type_alias(node),
            TopLevel::Struct(node) => self.walk_struct(node),
            TopLevel::Enum(node) => self.walk_enum(node),
            TopLevel::Func(node) => self.walk_func(node),
            TopLevel::Method(node) => self.walk_method(node),
            TopLevel::GetMethod(node) => self.walk_get_method(node),
            TopLevel::EmptyStmt(node) => self.walk_empty_stmt(node),
            TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt<'tree>) -> Self::Result {
        match stmt {
            Stmt::Block(node) => self.walk_block(node),
            Stmt::If(node) => self.walk_if(node),
            Stmt::While(node) => self.walk_while(node),
            Stmt::Repeat(node) => self.walk_repeat(node),
            Stmt::TryCatch(node) => self.walk_try_catch(node),
            Stmt::Return(node) => self.walk_return(node),
            Stmt::DoWhile(node) => self.walk_do_while(node),
            Stmt::Break(node) => self.walk_break(node),
            Stmt::Continue(node) => self.walk_continue(node),
            Stmt::Throw(node) => self.walk_throw(node),
            Stmt::Assert(node) => self.walk_assert(node),
            Stmt::Match(node) => self.walk_match_stmt(node),
            Stmt::EmptyStmt(node) => self.walk_empty_stmt(node),
            Stmt::ExprStmt(node) => self.walk_expr_stmt(node),
            Stmt::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        match expr {
            Expr::VarDeclLhs(node) => self.walk_var_decl_lhs(node),
            Expr::Assign(node) => self.walk_assign(node),
            Expr::SetAssign(node) => self.walk_set_assign(node),
            Expr::Ternary(node) => self.walk_ternary(node),
            Expr::Bin(node) => self.walk_binary(node),
            Expr::Unary(node) => self.walk_unary(node),
            Expr::Lazy(node) => self.walk_lazy(node),
            Expr::AsCast(node) => self.walk_as_cast(node),
            Expr::IsType(node) => self.walk_is_type(node),
            Expr::NotNull(node) => self.walk_not_null(node),
            Expr::DotAccess(node) => self.walk_dot_access(node),
            Expr::Call(node) => self.walk_call(node),
            Expr::Instantiation(node) => self.walk_instantiation(node),
            Expr::Paren(node) => self.walk_paren(node),
            Expr::Match(node) => self.walk_match(node),
            Expr::ObjectLit(node) => self.walk_object_lit(node),
            Expr::Tensor(node) => self.walk_tensor(node),
            Expr::Tuple(node) => self.walk_tuple(node),
            Expr::Lambda(node) => self.walk_lambda(node),
            Expr::NumberLit(node) => self.walk_number_lit(node),
            Expr::StringLit(node) => self.walk_string_lit(node),
            Expr::BoolLit(node) => self.walk_boolean_lit(node),
            Expr::NullLit(node) => self.walk_null_lit(node),
            Expr::Ident(node) => self.walk_ident(node),
            Expr::Underscore(node) => self.walk_underscore(node),
            Expr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_type(&mut self, typ: &Type<'tree>) -> Self::Result {
        match typ {
            Type::TypeIdent(node) => self.walk_type_ident(node),
            Type::TypeInstantiatedTs(node) => self.walk_type_instantiated_ts(node),
            Type::TensorType(node) => self.walk_tensor_type(node),
            Type::TupleType(node) => self.walk_tuple_type(node),
            Type::ParenthesizedType(node) => self.walk_parenthesized_type(node),
            Type::FunCallableType(node) => self.walk_fun_callable_type(node),
            Type::NullableType(node) => self.walk_nullable_type(node),
            Type::UnionType(node) => self.walk_union_type(node),
            Type::NullLit(node) => self.walk_null_lit_type(node),
            Type::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_tolk_required_version(&mut self, node: &TolkRequiredVersion<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.walk_string_lit(&value);
        }
        self.default_result()
    }

    fn walk_import(&mut self, node: &Import<'tree>) -> Self::Result {
        if let Some(path) = node.path() {
            self.walk_string_lit(&path);
        }
        self.default_result()
    }

    fn walk_global_var(&mut self, node: &GlobalVar<'tree>) -> Self::Result {
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

    fn walk_constant(&mut self, node: &Constant<'tree>) -> Self::Result {
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
            self.visit_expr(&value);
        }
        self.default_result()
    }

    fn walk_type_alias(&mut self, node: &TypeAlias<'tree>) -> Self::Result {
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

    fn walk_struct(&mut self, node: &Struct<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(pack_prefix) = node.pack_prefix() {
            self.walk_number_lit(&pack_prefix);
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

    fn walk_enum(&mut self, node: &Enum<'tree>) -> Self::Result {
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

    fn walk_func(&mut self, node: &Func<'tree>) -> Self::Result {
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
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_method(&mut self, node: &Method<'tree>) -> Self::Result {
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
        for param in node.parameters() {
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_get_method(&mut self, node: &GetMethod<'tree>) -> Self::Result {
        if let Some(annotations) = node.annotations() {
            self.walk_annotation_list(&annotations);
        }
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        for param in node.parameters() {
            self.walk_parameter(&param, false);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_empty_stmt(&mut self, _node: &EmptyStmt<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_block(&mut self, node: &Block<'tree>) -> Self::Result {
        for stmt in node.stmts() {
            self.visit_stmt(&stmt);
        }
        self.default_result()
    }

    fn walk_if(&mut self, node: &If<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body);
        }
        if let Some(alternative) = node.alternative() {
            match alternative {
                IfAlt::If(if_stmt) => {
                    self.walk_if(&if_stmt);
                }
                IfAlt::Block(block) => {
                    self.walk_block(&block);
                }
            }
        }
        self.default_result()
    }

    fn walk_while(&mut self, node: &While<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body);
        }
        self.default_result()
    }

    fn walk_repeat(&mut self, node: &Repeat<'tree>) -> Self::Result {
        if let Some(count) = node.count() {
            self.visit_expr(&count);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body);
        }
        self.default_result()
    }

    fn walk_try_catch(&mut self, node: &TryCatch<'tree>) -> Self::Result {
        if let Some(try_body) = node.body() {
            self.walk_block(&try_body);
        }
        if let Some(catch) = node.catch() {
            self.walk_catch_clause(&catch);
        }
        self.default_result()
    }

    fn walk_return(&mut self, node: &Return<'tree>) -> Self::Result {
        if let Some(body) = node.expr() {
            self.visit_expr(&body);
        }
        self.default_result()
    }

    fn walk_do_while(&mut self, node: &DoWhile<'tree>) -> Self::Result {
        if let Some(body) = node.body() {
            self.walk_block(&body);
        }
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        self.default_result()
    }

    fn walk_break(&mut self, _node: &Break<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_continue(&mut self, _node: &Continue<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_throw(&mut self, node: &Throw<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_assert(&mut self, node: &Assert<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_match_stmt(&mut self, node: &MatchStmt<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.walk_match(&expr);
        }
        self.default_result()
    }

    fn walk_expr_stmt(&mut self, node: &ExprStmt<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_assign(&mut self, node: &Assign<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expr(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expr(&right);
        }
        self.default_result()
    }

    fn walk_set_assign(&mut self, node: &SetAssign<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expr(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expr(&right);
        }
        self.default_result()
    }

    fn walk_ternary(&mut self, node: &Ternary<'tree>) -> Self::Result {
        if let Some(condition) = node.condition() {
            self.visit_expr(&condition);
        }
        if let Some(consequence) = node.consequence() {
            self.visit_expr(&consequence);
        }
        if let Some(alternative) = node.alternative() {
            self.visit_expr(&alternative);
        }
        self.default_result()
    }

    fn walk_binary(&mut self, node: &Bin<'tree>) -> Self::Result {
        if let Some(left) = node.left() {
            self.visit_expr(&left);
        }
        if let Some(right) = node.right() {
            self.visit_expr(&right);
        }
        self.default_result()
    }

    fn walk_unary(&mut self, node: &Unary<'tree>) -> Self::Result {
        if let Some(argument) = node.argument() {
            self.visit_expr(&argument);
        }
        self.default_result()
    }

    fn walk_lazy(&mut self, node: &Lazy<'tree>) -> Self::Result {
        if let Some(argument) = node.expr() {
            self.visit_expr(&argument);
        }
        self.default_result()
    }

    fn walk_as_cast(&mut self, node: &AsCast<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        if let Some(casted_to) = node.casted_to() {
            self.visit_type(&casted_to);
        }
        self.default_result()
    }

    fn walk_is_type(&mut self, node: &IsType<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        if let Some(rhs_type) = node.rhs_type() {
            self.visit_type(&rhs_type);
        }
        self.default_result()
    }

    fn walk_not_null(&mut self, node: &NotNull<'tree>) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_expr(&inner);
        }
        self.default_result()
    }

    fn walk_dot_access(&mut self, node: &DotAccess<'tree>) -> Self::Result {
        if let Some(obj) = node.obj() {
            self.visit_expr(&obj);
        }
        if let Some(field) = node.field() {
            match field {
                DotAccessField::Ident(ident) => {
                    self.walk_ident(&ident);
                }
                DotAccessField::NumericIndex(index) => {
                    self.walk_numeric_index(&index);
                }
            }
        }
        self.default_result()
    }

    fn walk_call(&mut self, node: &Call<'tree>) -> Self::Result {
        if let Some(callee) = node.callee() {
            self.visit_expr(&callee);
        }
        for arg in node.arguments() {
            self.walk_call_argument(&arg);
        }
        self.default_result()
    }

    fn walk_instantiation(&mut self, node: &Instantiation<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        if let Some(instantiation_ts) = node.instantiation_ts() {
            self.walk_instantiation_t_list(&instantiation_ts);
        }
        self.default_result()
    }

    fn walk_paren(&mut self, node: &Paren<'tree>) -> Self::Result {
        if let Some(inner) = node.inner() {
            self.visit_expr(&inner);
        }
        self.default_result()
    }

    fn walk_match(&mut self, node: &Match<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        if let Some(body) = node.body() {
            self.walk_match_body(&body);
        }
        self.default_result()
    }

    fn walk_object_lit(&mut self, node: &ObjectLit<'tree>) -> Self::Result {
        if let Some(object_type) = node.typ() {
            self.visit_type(&object_type);
        }
        for arg in node.arguments() {
            self.walk_instance_arg(&arg);
        }
        self.default_result()
    }

    fn walk_tensor(&mut self, node: &Tensor<'tree>) -> Self::Result {
        for element in node.elements() {
            self.walk_expr(&element);
        }
        self.default_result()
    }

    fn walk_tuple(&mut self, node: &Tuple<'tree>) -> Self::Result {
        for element in node.elements() {
            self.walk_expr(&element);
        }
        self.default_result()
    }

    fn walk_lambda(&mut self, node: &Lambda<'tree>) -> Self::Result {
        for param in node.parameters() {
            self.walk_lambda_parameter(&param);
        }
        if let Some(return_type) = node.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(body) = node.body() {
            self.walk_block(&body);
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

    fn walk_number_lit(&mut self, _node: &NumberLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_string_lit(&mut self, _node: &StringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_boolean_lit(&mut self, _node: &BoolLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_null_lit(&mut self, _node: &NullLit<'tree>) -> Self::Result {
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

    fn walk_type_ident(&mut self, _node: &TypeIdent<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_type_instantiated_ts(&mut self, node: &TypeInstantiatedTs<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_type_ident(&name);
        }
        if let Some(arguments) = node.arguments() {
            self.walk_instantiation_t_list(&arguments);
        }
        self.default_result()
    }

    fn walk_tensor_type(&mut self, node: &TensorType<'tree>) -> Self::Result {
        for element in node.elements() {
            self.visit_type(&element);
        }
        self.default_result()
    }

    fn walk_tuple_type(&mut self, node: &TupleType<'tree>) -> Self::Result {
        for element in node.elements() {
            self.visit_type(&element);
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

    fn walk_null_lit_type(&mut self, node: &NullLit<'tree>) -> Self::Result {
        self.walk_null_lit(node)
    }

    fn walk_annotation_list(&mut self, node: &AnnotationList<'tree>) -> Self::Result {
        for annotation in node.annotations() {
            self.walk_annotation(&annotation);
        }
        self.default_result()
    }

    fn walk_annotation(&mut self, node: &Annotation<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(args) = node.args() {
            self.walk_annotation_args(&args);
        }
        self.default_result()
    }

    fn walk_annotation_args(&mut self, node: &AnnotationArgs<'tree>) -> Self::Result {
        for arg in node.args() {
            self.visit_expr(&arg);
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
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(default) = node.default() {
            self.visit_type(&default);
        }
        self.default_result()
    }

    fn walk_parameter(&mut self, node: &Parameter<'tree>, _in_common: bool) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
        self.default_result()
    }

    fn walk_function_body(&mut self, body: &FuncBody<'tree>) -> Self::Result {
        match body {
            FuncBody::Block(block) => self.walk_block(block),
            FuncBody::AsmBody(asm) => self.walk_asm_body(asm),
            FuncBody::BuiltinSpecifier(_) | FuncBody::Unmapped(_) => self.default_result(),
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
            self.walk_struct_field(&field);
        }
        self.default_result()
    }

    fn walk_struct_field(&mut self, node: &StructField<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(typ) = node.typ() {
            self.visit_type(&typ);
        }
        if let Some(default) = node.default() {
            self.visit_expr(&default);
        }
        self.default_result()
    }

    fn walk_enum_body(&mut self, node: &EnumBody<'tree>) -> Self::Result {
        for member in node.members() {
            self.walk_enum_member(&member);
        }
        self.default_result()
    }

    fn walk_enum_member(&mut self, node: &EnumMember<'tree>) -> Self::Result {
        if let Some(name) = node.name() {
            self.walk_ident(&name);
        }
        if let Some(default) = node.default() {
            self.visit_expr(&default);
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
            self.walk_block(&body);
        }
        self.default_result()
    }

    fn walk_var_decl_lhs(&mut self, node: &VarDeclLhs<'tree>) -> Self::Result {
        if let Some(pattern) = node.pattern() {
            self.walk_var_decl_pattern(&pattern);
        }
        self.default_result()
    }

    fn walk_var_decl_pattern(&mut self, pattern: &VarDeclPattern<'tree>) -> Self::Result {
        match pattern {
            VarDeclPattern::TupleVars(tuple) => self.walk_tuple_vars(tuple),
            VarDeclPattern::TensorVars(tensor) => self.walk_tensor_vars(tensor),
            VarDeclPattern::VarDecl(var) => self.walk_var_declaration(var),
        }
    }

    fn walk_tuple_vars(&mut self, node: &TupleVars<'tree>) -> Self::Result {
        for var in node.vars() {
            self.walk_var_decl_pattern(&var);
        }
        self.default_result()
    }

    fn walk_tensor_vars(&mut self, node: &TensorVars<'tree>) -> Self::Result {
        for var in node.vars() {
            self.walk_var_decl_pattern(&var);
        }
        self.default_result()
    }

    fn walk_var_declaration(&mut self, node: &VarDecl<'tree>) -> Self::Result {
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
            MatchPattern::Expr(expr) => self.walk_expr(&expr),
            MatchPattern::Else => self.default_result(),
        };

        if let Some(body) = node.body() {
            match body {
                MatchArmBody::Block(ref block) => self.walk_block(block),
                MatchArmBody::Return(ref ret) => self.walk_return(ret),
                MatchArmBody::Throw(ref throw) => self.walk_throw(throw),
                MatchArmBody::Expr(ref expr) => self.visit_expr(expr),
            };
        }
        self.default_result()
    }

    fn walk_call_argument(&mut self, node: &CallArgument<'tree>) -> Self::Result {
        if let Some(expr) = node.expr() {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_instantiation_t_list(&mut self, node: &InstantiationTList<'tree>) -> Self::Result {
        for typ in node.types() {
            self.visit_type(&typ);
        }
        self.default_result()
    }

    fn walk_instance_arg(&mut self, node: &InstanceArg<'tree>) -> Self::Result {
        if let Some(value) = node.value() {
            self.visit_expr(&value);
        }
        self.default_result()
    }

    fn walk_asm_body(&mut self, node: &AsmBody<'tree>) -> Self::Result {
        for param in node.params() {
            self.walk_ident(&param);
        }
        for num in node.return_values() {
            self.walk_number_lit(&num);
        }
        for string in node.instructions() {
            self.walk_string_lit(&string);
        }
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

#[must_use]
pub fn find_parent_by_kind<'a>(node: &'a Node<'a>, target_kind: &str) -> Option<Node<'a>> {
    let target_kind = target_kind.as_bytes();
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind_bytes() == target_kind {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}
