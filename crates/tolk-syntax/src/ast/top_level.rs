use crate::ast::expressions::{Expr, Ident, NumberLit, StringLit, StringLit as ExprStringLiteral};
use crate::ast::node::{AstChildren, RawNode};
use crate::ast::statements::Block;
use crate::ast::types::Type;
use crate::ast::{
    AstNode, FunctionLike, HasAnnotations, HasGenericParams, HasName, HasTreeSitterKind,
    InvalidNodeKindError, TryFromNode,
};
use crate::{AstNodeBytesKind, impl_ast_node};
use tree_sitter::Node;

pub const CONTRACT_ENTRYPOINTS: &[&str] = &[
    "onInternalMessage",
    "onExternalMessage",
    "onRunTickTock",
    "onSplitPrepare",
    "onSplitInstall",
    "onBouncedMessage",
];

#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    TolkRequiredVersion(TolkRequiredVersion<'tree>),
    Import(Import<'tree>),
    Contract(Contract<'tree>),
    GlobalVar(GlobalVar<'tree>),
    Constant(Constant<'tree>),
    TypeAlias(TypeAlias<'tree>),
    Struct(Struct<'tree>),
    Enum(Enum<'tree>),
    Func(Func<'tree>),
    Method(Method<'tree>),
    GetMethod(GetMethod<'tree>),
    EmptyStmt(EmptyStmt<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> TopLevel<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub fn name_text(&self, source: &'tree str) -> &'tree str {
        let Some(name_node) = self.syntax().child_by_field_name("name") else {
            return "";
        };
        name_node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .trim_matches('`')
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            TopLevel::TolkRequiredVersion(n) => n.0,
            TopLevel::Import(n) => n.0,
            TopLevel::Contract(n) => n.0,
            TopLevel::GlobalVar(n) => n.0,
            TopLevel::Constant(n) => n.0,
            TopLevel::TypeAlias(n) => n.0,
            TopLevel::Struct(n) => n.0,
            TopLevel::Enum(n) => n.0,
            TopLevel::Func(n) => n.0,
            TopLevel::Method(n) => n.0,
            TopLevel::GetMethod(n) => n.0,
            TopLevel::EmptyStmt(n) => n.0,
            TopLevel::Unmapped(n) => n.0,
        }
    }
}

impl<'tree> HasName<'tree> for TopLevel<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        let name_node = self.syntax().child_by_field_name("name")?;
        Some(name_node.into())
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        Ok(Self::from(node))
    }
}

impl<'tree> AstNode<'tree> for TopLevel<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

impl<'tree> HasGenericParams<'tree> for TopLevel<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        match self {
            TopLevel::TypeAlias(n) => n.0.field("type_parameters"),
            TopLevel::Struct(n) => n.0.field("type_parameters"),
            TopLevel::Func(n) => n.0.field("type_parameters"),
            TopLevel::Method(n) => n.0.field("type_parameters"),
            TopLevel::GetMethod(n) => n.0.field("type_parameters"),
            _ => None,
        }
    }
}

impl<'tree> HasAnnotations<'tree> for TopLevel<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        match self {
            TopLevel::GlobalVar(n) => n.0.field("annotations"),
            TopLevel::Constant(n) => n.0.field("annotations"),
            TopLevel::TypeAlias(n) => n.0.field("annotations"),
            TopLevel::Struct(n) => n.0.field("annotations"),
            TopLevel::Enum(n) => n.0.field("annotations"),
            TopLevel::Func(n) => n.0.field("annotations"),
            TopLevel::Method(n) => n.0.field("annotations"),
            TopLevel::GetMethod(n) => n.0.field("annotations"),
            _ => None,
        }
    }
}

impl<'t> From<Node<'t>> for TopLevel<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"tolk_required_version" => TopLevel::TolkRequiredVersion(TolkRequiredVersion(node)),
            b"import_directive" => TopLevel::Import(Import(node)),
            b"contract_declaration" => TopLevel::Contract(Contract(node)),
            b"global_var_declaration" => TopLevel::GlobalVar(GlobalVar(node)),
            b"constant_declaration" => TopLevel::Constant(Constant(node)),
            b"type_alias_declaration" => TopLevel::TypeAlias(TypeAlias(node)),
            b"struct_declaration" => TopLevel::Struct(Struct(node)),
            b"enum_declaration" => TopLevel::Enum(Enum(node)),
            b"function_declaration" => TopLevel::Func(Func(node)),
            b"method_declaration" => TopLevel::Method(Method(node)),
            b"get_method_declaration" => TopLevel::GetMethod(GetMethod(node)),
            b"empty_statement" => TopLevel::EmptyStmt(EmptyStmt(node)),
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
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            TypeAliasUnderlyingType::Type(n) => n.syntax(),
            TypeAliasUnderlyingType::BuiltinSpecifier(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for TypeAliasUnderlyingType<'t> {
    fn from(node: Node<'t>) -> Self {
        match node.kind_bytes() {
            b"builtin_specifier" => {
                TypeAliasUnderlyingType::BuiltinSpecifier(BuiltinSpecifier(node))
            }
            _ => TypeAliasUnderlyingType::Type(Type::from(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FuncBody<'tree> {
    Block(Block<'tree>),
    AsmBody(AsmBody<'tree>),
    BuiltinSpecifier(BuiltinSpecifier<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> FuncBody<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        match self {
            FuncBody::Block(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FuncBody::AsmBody(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FuncBody::BuiltinSpecifier(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
            FuncBody::Unmapped(n) => n.0.utf8_text(source.as_bytes()).unwrap_or(""),
        }
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            FuncBody::Block(n) => n.0,
            FuncBody::AsmBody(n) => n.0,
            FuncBody::BuiltinSpecifier(n) => n.0,
            FuncBody::Unmapped(n) => n.0,
        }
    }
}

impl<'t> From<Node<'t>> for FuncBody<'t> {
    fn from(node: Node<'t>) -> Self {
        let kind = node.kind_bytes();
        match kind {
            b"block_statement" => FuncBody::Block(Block(node)),
            b"asm_body" => FuncBody::AsmBody(AsmBody(node)),
            b"builtin_specifier" => FuncBody::BuiltinSpecifier(BuiltinSpecifier(node)),
            _ => FuncBody::Unmapped(RawNode::new(node)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AsmBody<'tree>(pub Node<'tree>);

impl_ast_node!(AsmBody, "asm_body");

impl<'tree> AsmBody<'tree> {
    pub fn params(&self) -> AstChildren<'tree, Ident<'tree>> {
        self.0
            .child_by_field_name("rearrange")
            .and_then(|rearrange| rearrange.child_by_field_name("params"))
            .map(AstChildren::new)
            .unwrap_or_default()
    }

    pub fn return_values(&self) -> AstChildren<'tree, NumberLit<'tree>> {
        self.0
            .child_by_field_name("rearrange")
            .and_then(|rearrange| rearrange.child_by_field_name("return"))
            .map(AstChildren::new)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, ExprStringLiteral<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuiltinSpecifier<'tree>(pub Node<'tree>);

impl_ast_node!(BuiltinSpecifier, "builtin_specifier");

#[derive(Clone, Copy, Debug)]
pub struct TolkRequiredVersion<'tree>(pub Node<'tree>);

impl_ast_node!(TolkRequiredVersion, "tolk_required_version");

impl<'tree> TolkRequiredVersion<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<StringLit<'tree>> {
        self.0.field("value")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Import<'tree>(pub Node<'tree>);

impl_ast_node!(Import, "import_directive");

impl<'tree> Import<'tree> {
    #[must_use]
    pub fn path(&self) -> Option<StringLit<'tree>> {
        self.0.field("path")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Contract<'tree>(pub Node<'tree>);

impl_ast_node!(Contract, "contract_declaration");

impl<'tree> Contract<'tree> {
    #[must_use]
    pub fn body(&self) -> Option<ContractBody<'tree>> {
        self.0.field("body")
    }
}

impl<'tree> HasName<'tree> for Contract<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContractBody<'tree>(pub Node<'tree>);

impl_ast_node!(ContractBody, "contract_body");

impl<'tree> ContractBody<'tree> {
    #[must_use]
    pub fn fields(&self) -> AstChildren<'tree, ContractField<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContractField<'tree>(pub Node<'tree>);

impl_ast_node!(ContractField, "contract_field");

impl<'tree> ContractField<'tree> {
    #[must_use]
    pub fn value(&self) -> Option<ContractFieldValue<'tree>> {
        self.0.field("value")
    }
}

impl<'tree> HasName<'tree> for ContractField<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ContractFieldValue<'tree> {
    Type(Type<'tree>),
    Expr(Expr<'tree>),
}

impl<'tree> ContractFieldValue<'tree> {
    #[must_use]
    pub fn text(&self, source: &'tree str) -> &'tree str {
        self.syntax().utf8_text(source.as_bytes()).unwrap_or("")
    }

    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            ContractFieldValue::Type(n) => n.syntax(),
            ContractFieldValue::Expr(n) => n.syntax(),
        }
    }
}

impl<'t> From<Node<'t>> for ContractFieldValue<'t> {
    fn from(node: Node<'t>) -> Self {
        if Type::try_from_node(node).is_ok() {
            ContractFieldValue::Type(Type::from(node))
        } else {
            ContractFieldValue::Expr(Expr::from(node))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Constant<'tree>(pub Node<'tree>);

impl_ast_node!(Constant, "constant_declaration");

impl<'tree> Constant<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn value(&self) -> Option<Expr<'tree>> {
        self.0.field("value")
    }
}

impl<'tree> HasName<'tree> for Constant<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasAnnotations<'tree> for Constant<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.annotations()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalVar<'tree>(pub Node<'tree>);

impl_ast_node!(GlobalVar, "global_var_declaration");

impl<'tree> GlobalVar<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

impl<'tree> HasName<'tree> for GlobalVar<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasAnnotations<'tree> for GlobalVar<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.annotations()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeAlias<'tree>(pub Node<'tree>);

impl_ast_node!(TypeAlias, "type_alias_declaration");

impl<'tree> TypeAlias<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn underlying_type(&self) -> Option<TypeAliasUnderlyingType<'tree>> {
        self.0.field("underlying_type")
    }
}

impl<'tree> HasName<'tree> for TypeAlias<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasGenericParams<'tree> for TypeAlias<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }
}

impl<'tree> HasAnnotations<'tree> for TypeAlias<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Struct<'tree>(pub Node<'tree>);

impl_ast_node!(Struct, "struct_declaration");

impl<'tree> Struct<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn pack_prefix(&self) -> Option<NumberLit<'tree>> {
        self.0.field("pack_prefix")
    }

    #[must_use]
    pub fn body(&self) -> Option<StructBody<'tree>> {
        self.0.field("body")
    }
}

impl<'tree> HasName<'tree> for Struct<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasGenericParams<'tree> for Struct<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }
}

impl<'tree> HasAnnotations<'tree> for Struct<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructBody<'tree>(pub Node<'tree>);

impl_ast_node!(StructBody, "struct_body");

impl<'tree> StructBody<'tree> {
    #[must_use]
    pub fn fields(&self) -> AstChildren<'tree, StructField<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructField<'tree>(pub Node<'tree>);

impl_ast_node!(StructField, "struct_field_declaration");

impl<'tree> StructField<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn modifiers(&self) -> Option<StructFieldModifiers<'tree>> {
        self.0.field("modifiers")
    }

    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn default(&self) -> Option<Expr<'tree>> {
        self.0.field("default")
    }
}

impl<'tree> HasName<'tree> for StructField<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasAnnotations<'tree> for StructField<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        StructField::annotations(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructFieldModifier {
    Readonly,
    Private,
}

impl StructFieldModifier {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            StructFieldModifier::Readonly => "readonly",
            StructFieldModifier::Private => "private",
        }
    }
}

impl<'tree> From<Node<'tree>> for StructFieldModifier {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"readonly" => StructFieldModifier::Readonly,
            b"private" => StructFieldModifier::Private,
            _ => panic!("Unknown struct field modifier: {}", node.kind()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StructFieldModifiers<'tree>(pub Node<'tree>);

impl_ast_node!(StructFieldModifiers, "struct_field_modifiers");

impl StructFieldModifiers<'_> {
    #[must_use]
    pub fn modifiers(&self) -> Vec<StructFieldModifier> {
        let mut cursor = self.0.walk();
        self.0
            .children(&mut cursor)
            .filter_map(|n| match n.kind_bytes() {
                b"readonly" | b"private" => Some(StructFieldModifier::from(n)),
                _ => None,
            })
            .collect()
    }

    #[must_use]
    pub fn has_readonly(&self) -> bool {
        self.modifiers().contains(&StructFieldModifier::Readonly)
    }

    #[must_use]
    pub fn has_private(&self) -> bool {
        self.modifiers().contains(&StructFieldModifier::Private)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Enum<'tree>(pub Node<'tree>);

impl_ast_node!(Enum, "enum_declaration");

impl<'tree> Enum<'tree> {
    #[must_use]
    pub fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }

    #[must_use]
    pub fn backed_type(&self) -> Option<Type<'tree>> {
        self.0.field("backed_type")
    }

    #[must_use]
    pub fn body(&self) -> Option<EnumBody<'tree>> {
        self.0.field("body")
    }
}

impl<'tree> HasName<'tree> for Enum<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasAnnotations<'tree> for Enum<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.annotations()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumBody<'tree>(pub Node<'tree>);

impl_ast_node!(EnumBody, "enum_body");

impl<'tree> EnumBody<'tree> {
    #[must_use]
    pub fn members(&self) -> AstChildren<'tree, EnumMember<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumMember<'tree>(pub Node<'tree>);

impl_ast_node!(EnumMember, "enum_member_declaration");

impl<'tree> EnumMember<'tree> {
    #[must_use]
    pub fn default(&self) -> Option<Expr<'tree>> {
        self.0.field("default")
    }

    #[must_use]
    pub fn owner(&'tree self) -> Option<Enum<'tree>> {
        crate::match_parents!(&self.0, Enum(...))
    }
}

impl<'tree> HasName<'tree> for EnumMember<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Func<'tree>(pub Node<'tree>);

impl_ast_node!(Func, "function_declaration");

impl<'tree> Func<'tree> {
    pub fn parameters(self) -> AstChildren<'tree, Parameter<'tree>> {
        self.0
            .child_by_field_name("parameters")
            .map(AstChildren::new)
            .unwrap_or_default()
    }
}

impl<'tree> HasName<'tree> for Func<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasGenericParams<'tree> for Func<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }
}

impl<'tree> FunctionLike<'tree> for Func<'tree> {
    fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    fn body(&self) -> Option<FuncBody<'tree>> {
        body_impl(self.0)
    }

    fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>> {
        self.0
            .child_by_field_name("parameters")
            .map(AstChildren::new)
            .unwrap_or_default()
    }
}

impl<'tree> HasAnnotations<'tree> for Func<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Parameter<'tree>(pub Node<'tree>);

impl_ast_node!(Parameter, "parameter_declaration");

impl<'tree> Parameter<'tree> {
    #[must_use]
    pub fn mutate(&self) -> bool {
        self.0.field::<Ident<'_>>("mutate").is_some()
    }

    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }

    #[must_use]
    pub fn default(&self) -> Option<Expr<'tree>> {
        self.0.field("default")
    }
}

impl<'tree> HasName<'tree> for Parameter<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Method<'tree>(pub Node<'tree>);

impl_ast_node!(Method, "method_declaration");

impl<'tree> Method<'tree> {
    #[must_use]
    pub fn receiver(&self) -> Option<MethodReceiver<'tree>> {
        self.0.field("receiver")
    }

    #[must_use]
    pub fn receiver_type(&self) -> Option<Type<'tree>> {
        self.receiver().and_then(|r| r.typ())
    }

    pub fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>> {
        let list = self.0.child_by_field_name("parameters");
        list.map(AstChildren::<Parameter>::new).unwrap_or_default()
    }

    pub fn parameters_ext(
        self,
        sources: &str,
        skip_self: bool,
    ) -> impl Iterator<Item = Parameter<'tree>> + use<'tree> {
        let list = self.0.child_by_field_name("parameters");
        let mut params = list
            .map(AstChildren::<Parameter>::new)
            .into_iter()
            .flatten()
            .peekable();

        let skip = skip_self
            && params
                .peek()
                .and_then(HasName::name)
                .is_some_and(|name| name.text_matches(sources, "self"));

        params.skip(usize::from(skip))
    }

    #[must_use]
    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    #[must_use]
    pub fn is_instance(&self, sources: &str) -> bool {
        let mut parameters = self.parameters_ext(sources, false);
        let Some(first) = parameters.next() else {
            return false;
        };

        let Some(first_name) = first.name() else {
            return false;
        };
        first_name.text_matches(sources, "self")
    }
}

impl<'tree> HasName<'tree> for Method<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasGenericParams<'tree> for Method<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }
}

impl<'tree> HasAnnotations<'tree> for Method<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }
}

impl<'tree> FunctionLike<'tree> for Method<'tree> {
    fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    fn body(&self) -> Option<FuncBody<'tree>> {
        body_impl(self.0)
    }

    fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>> {
        let list = self.0.child_by_field_name("parameters");
        list.map(AstChildren::<Parameter>::new).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GetMethod<'tree>(pub Node<'tree>);

impl_ast_node!(GetMethod, "get_method_declaration");

impl<'tree> GetMethod<'tree> {
    pub fn parameters(self) -> AstChildren<'tree, Parameter<'tree>> {
        self.0
            .child_by_field_name("parameters")
            .map(AstChildren::new)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }
}

impl<'tree> HasName<'tree> for GetMethod<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

impl<'tree> HasGenericParams<'tree> for GetMethod<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }
}

impl<'tree> HasAnnotations<'tree> for GetMethod<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        self.0.field("annotations")
    }
}

impl<'tree> FunctionLike<'tree> for GetMethod<'tree> {
    fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    fn body(&self) -> Option<FuncBody<'tree>> {
        body_impl(self.0)
    }

    fn parameters(&self) -> AstChildren<'tree, Parameter<'tree>> {
        self.0
            .child_by_field_name("parameters")
            .map(AstChildren::new)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodReceiver<'tree>(pub Node<'tree>);

impl_ast_node!(MethodReceiver, "method_receiver");

impl<'tree> MethodReceiver<'tree> {
    #[must_use]
    pub fn typ(&self) -> Option<Type<'tree>> {
        self.0.field("receiver_type")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmptyStmt<'tree>(pub Node<'tree>);

impl_ast_node!(EmptyStmt, "empty_statement");

#[derive(Clone, Copy, Debug)]
pub struct AnnotationList<'tree>(pub Node<'tree>);

impl_ast_node!(AnnotationList, "annotation_list");

impl<'tree> AnnotationList<'tree> {
    #[must_use]
    pub fn annotations(&self) -> AstChildren<'tree, Annotation<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Annotation<'tree>(pub Node<'tree>);

impl_ast_node!(Annotation, "annotation");

#[derive(Clone, Copy, Debug)]
pub struct AnnotationName<'tree>(pub Node<'tree>);

impl_ast_node!(AnnotationName, "annotation_name");

impl<'tree> Annotation<'tree> {
    #[must_use]
    pub fn args(&self) -> Option<AnnotationArgs<'tree>> {
        self.0.field("arguments")
    }
}

impl<'tree> HasName<'tree> for Annotation<'tree> {
    type Name = AnnotationName<'tree>;

    fn name(&self) -> Option<AnnotationName<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnnotationArgs<'tree>(pub Node<'tree>);

impl_ast_node!(AnnotationArgs, "annotation_arguments");

impl<'tree> AnnotationArgs<'tree> {
    #[must_use]
    pub fn args(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeParameters<'tree>(pub Node<'tree>);

impl_ast_node!(TypeParameters, "type_parameters");

impl<'tree> TypeParameters<'tree> {
    #[must_use]
    pub fn parameters(&self) -> AstChildren<'tree, TypeParameter<'tree>> {
        AstChildren::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TypeParameter<'tree>(pub Node<'tree>);

impl_ast_node!(TypeParameter, "type_parameter");

impl<'tree> TypeParameter<'tree> {
    #[must_use]
    pub fn default(&self) -> Option<Type<'tree>> {
        self.0.field("default")
    }
}

impl<'tree> HasName<'tree> for TypeParameter<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        self.0.field("name")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BaseFunction<'tree> {
    Function(Func<'tree>),
    MethodDeclaration(Method<'tree>),
    GetMethodDeclaration(GetMethod<'tree>),
}

impl<'tree> BaseFunction<'tree> {
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            BaseFunction::Function(f) => f.0,
            BaseFunction::MethodDeclaration(m) => m.0,
            BaseFunction::GetMethodDeclaration(g) => g.0,
        }
    }

    #[must_use]
    pub fn type_parameters_node(&self) -> Option<TypeParameters<'tree>> {
        match self {
            BaseFunction::Function(f) => f.type_parameters(),
            BaseFunction::MethodDeclaration(m) => m.type_parameters(),
            BaseFunction::GetMethodDeclaration(g) => g.type_parameters(),
        }
    }

    #[must_use]
    pub fn parameters(self) -> AstChildren<'tree, Parameter<'tree>> {
        match self {
            BaseFunction::Function(f) => f.parameters(),
            BaseFunction::MethodDeclaration(m) => m.parameters(),
            BaseFunction::GetMethodDeclaration(g) => g.parameters(),
        }
    }

    #[must_use]
    pub fn return_type(&self) -> Option<Type<'tree>> {
        match self {
            BaseFunction::Function(f) => f.return_type(),
            BaseFunction::MethodDeclaration(m) => m.return_type(),
            BaseFunction::GetMethodDeclaration(g) => g.return_type(),
        }
    }

    #[must_use]
    pub fn body(&self) -> Option<FuncBody<'tree>> {
        match self {
            BaseFunction::Function(f) => f.body(),
            BaseFunction::MethodDeclaration(m) => m.body(),
            BaseFunction::GetMethodDeclaration(g) => g.body(),
        }
    }

    #[must_use]
    pub const fn is_method(&self) -> bool {
        matches!(
            self,
            BaseFunction::MethodDeclaration(_) | BaseFunction::GetMethodDeclaration(_)
        )
    }

    #[must_use]
    pub fn is_instance_method(&self, sources: &str) -> bool {
        matches!(
            self,
            BaseFunction::MethodDeclaration(method) if method.is_instance(sources)
        )
    }

    #[must_use]
    pub fn receiver_type(&self) -> Option<Type<'tree>> {
        match self {
            BaseFunction::MethodDeclaration(m) => m.receiver_type(),
            BaseFunction::Function(_) | BaseFunction::GetMethodDeclaration(_) => None,
        }
    }
}

impl<'tree> HasName<'tree> for BaseFunction<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Ident<'tree>> {
        match self {
            BaseFunction::Function(f) => f.name(),
            BaseFunction::MethodDeclaration(m) => m.name(),
            BaseFunction::GetMethodDeclaration(g) => g.name(),
        }
    }
}

impl<'tree> HasAnnotations<'tree> for BaseFunction<'tree> {
    fn annotations(&self) -> Option<AnnotationList<'tree>> {
        match self {
            BaseFunction::Function(f) => f.annotations(),
            BaseFunction::MethodDeclaration(m) => m.annotations(),
            BaseFunction::GetMethodDeclaration(g) => g.annotations(),
        }
    }
}

impl<'tree> HasGenericParams<'tree> for BaseFunction<'tree> {
    fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        match self {
            BaseFunction::Function(f) => f.type_parameters(),
            BaseFunction::MethodDeclaration(m) => m.type_parameters(),
            BaseFunction::GetMethodDeclaration(g) => g.type_parameters(),
        }
    }
}

fn body_impl(node: Node) -> Option<FuncBody> {
    node.child_by_field_name("body")
        .or_else(|| node.child_by_field_name("asm_body"))
        .or_else(|| node.child_by_field_name("builtin_specifier"))
        .map(Into::into)
}
