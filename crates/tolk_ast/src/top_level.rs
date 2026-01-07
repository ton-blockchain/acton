use crate::expressions::{
    Expression, Ident, NumberLiteral, StringLiteral, StringLiteral as ExprStringLiteral,
};
use crate::node::{NodeFieldExt, RawNode};
use crate::statements::BlockStatement;
use crate::types::Type;
use crate::walker::parent_of_type;
use tree_sitter::Node;

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    TolkRequiredVersion(TolkRequiredVersion<'tree>),
    Import(Import<'tree>),
    GlobalVarDeclaration(GlobalVarDeclaration<'tree>),
    ConstantDeclaration(ConstantDeclaration<'tree>),
    TypeAliasDeclaration(TypeAliasDeclaration<'tree>),
    StructDeclaration(StructDeclaration<'tree>),
    EnumDeclaration(EnumDeclaration<'tree>),
    Function(Function<'tree>),
    MethodDeclaration(MethodDeclaration<'tree>),
    GetMethodDeclaration(GetMethodDeclaration<'tree>),
    EmptyStatement(EmptyStatement<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> TopLevel<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.raw_node().utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn name(&self, source: &'tree str) -> &'tree str {
        let Some(name_node) = self.raw_node().child_by_field_name("name") else {
            return "";
        };
        name_node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .trim_matches('`')
    }

    pub fn name_ident(&self) -> Option<Node<'_>> {
        let name_node = self.raw_node().child_by_field_name("name")?;
        Some(name_node)
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            TopLevel::TolkRequiredVersion(n) => n.0,
            TopLevel::Import(n) => n.0,
            TopLevel::GlobalVarDeclaration(n) => n.0,
            TopLevel::ConstantDeclaration(n) => n.0,
            TopLevel::TypeAliasDeclaration(n) => n.0,
            TopLevel::StructDeclaration(n) => n.0,
            TopLevel::EnumDeclaration(n) => n.0,
            TopLevel::Function(n) => n.0,
            TopLevel::MethodDeclaration(n) => n.0,
            TopLevel::GetMethodDeclaration(n) => n.0,
            TopLevel::EmptyStatement(n) => n.0,
            TopLevel::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for TopLevel<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "tolk_required_version" => TopLevel::TolkRequiredVersion(TolkRequiredVersion(node)),
            "import_directive" => TopLevel::Import(Import(node)),
            "global_var_declaration" => TopLevel::GlobalVarDeclaration(GlobalVarDeclaration(node)),
            "constant_declaration" => TopLevel::ConstantDeclaration(ConstantDeclaration(node)),
            "type_alias_declaration" => TopLevel::TypeAliasDeclaration(TypeAliasDeclaration(node)),
            "struct_declaration" => TopLevel::StructDeclaration(StructDeclaration(node)),
            "enum_declaration" => TopLevel::EnumDeclaration(EnumDeclaration(node)),
            "function_declaration" => TopLevel::Function(Function(node)),
            "method_declaration" => TopLevel::MethodDeclaration(MethodDeclaration(node)),
            "get_method_declaration" => TopLevel::GetMethodDeclaration(GetMethodDeclaration(node)),
            "empty_statement" => TopLevel::EmptyStatement(EmptyStatement(node)),
            _ => TopLevel::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TypeAliasUnderlyingType<'tree> {
    Type(Type<'tree>),
    BuiltinSpecifier(BuiltinSpecifier<'tree>),
}

impl<'tree> TypeAliasUnderlyingType<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.raw_node().utf8_text(source.as_bytes()).unwrap_or("")
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            TypeAliasUnderlyingType::Type(n) => n.raw_node(),
            TypeAliasUnderlyingType::BuiltinSpecifier(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for TypeAliasUnderlyingType<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind() {
            "builtin_specifier" => {
                TypeAliasUnderlyingType::BuiltinSpecifier(BuiltinSpecifier(node))
            }
            _ => TypeAliasUnderlyingType::Type(Type::from(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FunctionBody<'tree> {
    BlockStatement(BlockStatement<'tree>),
    AsmBody(AsmBody<'tree>),
    BuiltinSpecifier(BuiltinSpecifier<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> FunctionBody<'tree> {
    pub fn text(&self, source: &'tree str) -> &'tree str {
        match self {
            FunctionBody::BlockStatement(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FunctionBody::AsmBody(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FunctionBody::BuiltinSpecifier(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FunctionBody::Unmapped(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
        }
    }

    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            FunctionBody::BlockStatement(n) => n.0,
            FunctionBody::AsmBody(n) => n.0,
            FunctionBody::BuiltinSpecifier(n) => n.0,
            FunctionBody::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for FunctionBody<'t> {
    fn from(node: Node<'t>) -> Self {
        let kind = node.kind();
        match kind {
            "block_statement" => FunctionBody::BlockStatement(BlockStatement(node)),
            "asm_body" => FunctionBody::AsmBody(AsmBody(node)),
            "builtin_specifier" => FunctionBody::BuiltinSpecifier(BuiltinSpecifier(node)),
            _ => FunctionBody::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AsmBody<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for AsmBody<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> AsmBody<'tree> {
    pub fn params(&self) -> Vec<Ident<'tree>> {
        let Some(rearrange) = self.0.child_by_field_name("rearrange") else {
            return vec![];
        };
        let Some(params_node) = rearrange.child_by_field_name("params") else {
            return vec![];
        };
        let mut cursor = params_node.walk();
        params_node
            .children(&mut cursor)
            .filter(|n| n.kind() == "identifier")
            .map(Ident)
            .collect()
    }

    pub fn return_values(&self) -> Vec<NumberLiteral<'tree>> {
        let Some(rearrange) = self.0.child_by_field_name("rearrange") else {
            return vec![];
        };
        let Some(return_node) = rearrange.child_by_field_name("return") else {
            return vec![];
        };
        let mut cursor = return_node.walk();
        return_node
            .children(&mut cursor)
            .filter(|n| n.kind() == "number_literal")
            .map(NumberLiteral)
            .collect()
    }

    pub fn instructions(&self) -> Vec<ExprStringLiteral<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "string_literal")
            .map(ExprStringLiteral)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinSpecifier<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for BuiltinSpecifier<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TolkRequiredVersion<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TolkRequiredVersion<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TolkRequiredVersion<'tree> {
    pub fn value(&self) -> Option<StringLiteral<'tree>> {
        self.0.field("value")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Import<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Import<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Import<'tree> {
    pub fn path(&self) -> Option<StringLiteral<'tree>> {
        self.0.field("path")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConstantDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for ConstantDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> ConstantDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn value(&self) -> Option<Expression<'tree>> {
        self.0.field("value")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalVarDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for GlobalVarDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> GlobalVarDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeAliasDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypeAliasDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TypeAliasDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    pub fn underlying_type(&self) -> Option<TypeAliasUnderlyingType<'tree>> {
        self.0.field("underlying_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for StructDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> StructDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn pack_prefix(&self) -> Option<NumberLiteral<'tree>> {
        self.0.field("pack_prefix")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    pub fn body(&self) -> Option<StructBody<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructBody<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for StructBody<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> StructBody<'tree> {
    pub fn fields(&self) -> Vec<StructFieldDeclaration<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "struct_field_declaration")
            .map(StructFieldDeclaration)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructFieldDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for StructFieldDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> StructFieldDeclaration<'tree> {
    pub fn modifiers(&self) -> Option<StructFieldModifiers<'tree>> {
        self.0.field("modifiers")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn default(&self) -> Option<Expression<'tree>> {
        self.0.field("default")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructFieldModifier {
    Readonly,
    Private,
}

impl StructFieldModifier {
    pub fn as_str(&self) -> &'static str {
        match self {
            StructFieldModifier::Readonly => "readonly",
            StructFieldModifier::Private => "private",
        }
    }
}

impl<'tree> From<Node<'tree>> for StructFieldModifier {
    fn from(node: Node<'tree>) -> Self {
        match node.kind() {
            "readonly" => StructFieldModifier::Readonly,
            "private" => StructFieldModifier::Private,
            _ => panic!("Unknown struct field modifier: {}", node.kind()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructFieldModifiers<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for StructFieldModifiers<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> StructFieldModifiers<'tree> {
    pub fn modifiers(&self) -> Vec<StructFieldModifier> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter_map(|n| match n.kind() {
                "readonly" | "private" => Some(StructFieldModifier::from(n)),
                _ => None,
            })
            .collect()
    }

    pub fn has_readonly(&self) -> bool {
        self.modifiers().contains(&StructFieldModifier::Readonly)
    }

    pub fn has_private(&self) -> bool {
        self.modifiers().contains(&StructFieldModifier::Private)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for EnumDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> EnumDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn backed_type(&self) -> Option<Type<'tree>> {
        self.0.field("backed_type")
    }

    pub fn body(&self) -> Option<EnumBody<'tree>> {
        self.0.field("body")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumBody<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for EnumBody<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> EnumBody<'tree> {
    pub fn members(&self) -> Vec<EnumMemberDeclaration<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "enum_member_declaration")
            .map(EnumMemberDeclaration)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumMemberDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for EnumMemberDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> EnumMemberDeclaration<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn default(&self) -> Option<Expression<'tree>> {
        self.0.field("default")
    }

    pub fn owner(&'tree self) -> Option<EnumDeclaration<'tree>> {
        let node = parent_of_type(&self.0, "enum_declaration")?;
        Some(EnumDeclaration::from(node))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Function<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Function<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Function<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    pub fn parameters(&self) -> Vec<Parameter<'tree>> {
        let Some(list) = self.0.child_by_field_name("parameters") else {
            return vec![];
        };

        let mut cursor = list.walk();
        list.children(&mut cursor)
            .filter(|n| n.kind() == "parameter_declaration")
            .map(Parameter)
            .collect()
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    pub fn body(&self) -> Option<FunctionBody<'tree>> {
        self.0
            .child_by_field_name("body")
            .or_else(|| self.0.child_by_field_name("asm_body"))
            .or_else(|| self.0.child_by_field_name("builtin_specifier"))
            .map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Parameter<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Parameter<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Parameter<'tree> {
    pub fn mutate(&self) -> bool {
        self.0.field::<Ident>("mutate").is_some()
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    pub fn default(&self) -> Option<Expression<'tree>> {
        self.0.field("default")
    }

    pub fn raw_node(&self) -> &Node<'tree> {
        &self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MethodDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> MethodDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn receiver(&self) -> Option<MethodReceiver<'tree>> {
        self.0.field("receiver")
    }

    pub fn receiver_type(&self) -> Option<Type<'tree>> {
        self.receiver().and_then(|r| r.typ())
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    pub fn parameters(&self, sources: &str, skip_self: bool) -> Vec<Parameter<'tree>> {
        let Some(list) = self.0.child_by_field_name("parameters") else {
            return vec![];
        };

        let mut cursor = list.walk();
        let mut params = list
            .children(&mut cursor)
            .filter(|n| n.kind() == "parameter_declaration")
            .peekable();

        if skip_self
            && let Some(first) = params.peek()
            && first.utf8_text(sources.as_ref()) == Ok("self")
        {
            return params.skip(1).map(Parameter).collect();
        }

        params.map(Parameter).collect()
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    pub fn body(&self) -> Option<FunctionBody<'tree>> {
        self.0
            .child_by_field_name("body")
            .or_else(|| self.0.child_by_field_name("asm_body"))
            .or_else(|| self.0.child_by_field_name("builtin_specifier"))
            .map(Into::into)
    }

    pub fn is_instance(&self, sources: &str) -> bool {
        let parameters = self.parameters(sources, false);
        if parameters.is_empty() {
            return false;
        }

        let Some(first_name) = parameters[0].name() else {
            return false;
        };
        first_name.text(sources) == "self"
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GetMethodDeclaration<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for GetMethodDeclaration<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> GetMethodDeclaration<'tree> {
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    pub fn parameters(&self) -> Vec<Parameter<'tree>> {
        let Some(list) = self.0.child_by_field_name("parameters") else {
            return vec![];
        };

        let mut cursor = list.walk();
        list.children(&mut cursor)
            .filter(|n| n.kind() == "parameter_declaration")
            .map(Parameter)
            .collect()
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    pub fn body(&self) -> Option<FunctionBody<'tree>> {
        self.0
            .child_by_field_name("body")
            .or_else(|| self.0.child_by_field_name("asm_body"))
            .or_else(|| self.0.child_by_field_name("builtin_specifier"))
            .map(Into::into)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodReceiver<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for MethodReceiver<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> MethodReceiver<'tree> {
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("receiver_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmptyStatement<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for EmptyStatement<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnnotationList<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for AnnotationList<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> AnnotationList<'tree> {
    pub fn annotations(&self) -> Vec<Annotation<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "annotation")
            .map(Annotation)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Annotation<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for Annotation<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> Annotation<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn arguments(&self) -> Option<AnnotationArguments<'tree>> {
        self.0.field("arguments")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnnotationArguments<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for AnnotationArguments<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> AnnotationArguments<'tree> {
    pub fn arguments(&self) -> Vec<Expression<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.is_named() && n.kind() != "comment")
            .map(Expression::from)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeParameters<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypeParameters<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TypeParameters<'tree> {
    pub fn parameters(&self) -> Vec<TypeParameter<'tree>> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter(|n| n.kind() == "type_parameter")
            .map(TypeParameter)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeParameter<'tree>(pub Node<'tree>);

impl<'t> From<Node<'t>> for TypeParameter<'t> {
    fn from(n: Node<'t>) -> Self {
        Self(n)
    }
}

impl<'tree> TypeParameter<'tree> {
    pub fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }

    pub fn default(&self) -> Option<crate::types::Type<'tree>> {
        self.0.field("default")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BaseFunction<'tree> {
    Function(Function<'tree>),
    MethodDeclaration(MethodDeclaration<'tree>),
    GetMethodDeclaration(GetMethodDeclaration<'tree>),
}

impl<'tree> BaseFunction<'tree> {
    pub fn raw_node(&self) -> Node<'tree> {
        match self {
            BaseFunction::Function(f) => f.0,
            BaseFunction::MethodDeclaration(m) => m.0,
            BaseFunction::GetMethodDeclaration(g) => g.0,
        }
    }

    pub fn name(&self) -> Option<Ident<'tree>> {
        match self {
            BaseFunction::Function(f) => f.name(),
            BaseFunction::MethodDeclaration(m) => m.name(),
            BaseFunction::GetMethodDeclaration(g) => g.name(),
        }
    }

    pub fn type_parameters(&self) -> Vec<TypeParameter<'tree>> {
        match self {
            BaseFunction::Function(f) => f
                .type_parameters()
                .map(|tp| tp.parameters())
                .unwrap_or_default(),
            BaseFunction::MethodDeclaration(m) => m
                .type_parameters()
                .map(|tp| tp.parameters())
                .unwrap_or_default(),
            BaseFunction::GetMethodDeclaration(g) => g
                .type_parameters()
                .map(|tp| tp.parameters())
                .unwrap_or_default(),
        }
    }

    pub fn parameters(&self, sources: &str, skip_self: bool) -> Vec<Parameter<'tree>> {
        match self {
            BaseFunction::Function(f) => f.parameters(),
            BaseFunction::MethodDeclaration(m) => m.parameters(sources, skip_self),
            BaseFunction::GetMethodDeclaration(g) => g.parameters(),
        }
    }

    pub fn return_type(&self) -> Option<Type<'tree>> {
        match self {
            BaseFunction::Function(f) => f.return_type(),
            BaseFunction::MethodDeclaration(m) => m.return_type(),
            BaseFunction::GetMethodDeclaration(g) => g.return_type(),
        }
    }

    pub fn body(&self) -> Option<FunctionBody<'tree>> {
        match self {
            BaseFunction::Function(f) => f.body(),
            BaseFunction::MethodDeclaration(m) => m.body(),
            BaseFunction::GetMethodDeclaration(g) => g.body(),
        }
    }

    pub fn is_method(&self) -> bool {
        matches!(
            self,
            BaseFunction::MethodDeclaration(_) | BaseFunction::GetMethodDeclaration(_)
        )
    }

    pub fn is_instance_method(&self, sources: &str) -> bool {
        matches!(
            self,
            BaseFunction::MethodDeclaration(method) if method.is_instance(sources)
        )
    }

    pub fn receiver_type(&self) -> Option<Type<'tree>> {
        match self {
            BaseFunction::MethodDeclaration(m) => m.receiver_type(),
            BaseFunction::GetMethodDeclaration(_) => None,
            BaseFunction::Function(_) => None,
        }
    }
}
