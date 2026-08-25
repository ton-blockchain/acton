use std::error::Error;
use std::fmt::{self, Write};

use syn::{
    Attribute, BinOp, Expr, Fields, FnArg, GenericArgument, Item, ItemFn, ItemMod, ItemStruct,
    LitStr, PathArguments, ReturnType, Stmt, Type,
};

#[derive(Debug)]
pub struct CompileError {
    message: String,
}

impl CompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CompileError {}

impl From<syn::Error> for CompileError {
    fn from(error: syn::Error) -> Self {
        Self::new(error.to_string())
    }
}

struct ContractMetadata {
    name: String,
    author: String,
    version: String,
    description: String,
}

struct FieldSpec {
    tolk_name: String,
    ty: String,
}

struct StructSpec {
    name: String,
    fields: Vec<FieldSpec>,
}

struct MessageSpec {
    data: StructSpec,
    opcode: String,
}

struct HandlerSpec {
    message: String,
    body: Vec<String>,
}

struct GetterSpec {
    name: String,
    return_type: String,
    body: String,
}

struct ContractSpec {
    metadata: ContractMetadata,
    storage: StructSpec,
    messages: Vec<MessageSpec>,
    handlers: Vec<HandlerSpec>,
    getters: Vec<GetterSpec>,
}

/// Lowers a contract written in the supported Rust DSL subset to Tolk source.
///
/// # Errors
///
/// Returns [`CompileError`] when the Rust source is invalid or uses a construct
/// that the minimal DSL does not support yet.
pub fn compile_source(source: &str) -> Result<String, CompileError> {
    let file = syn::parse_file(source)?;
    let contract_module = file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Mod(module) if has_attribute(&module.attrs, "contract") => Some(module),
            _ => None,
        })
        .ok_or_else(|| CompileError::new("expected one inline #[contract(...)] module"))?;

    let spec = parse_contract(contract_module)?;
    emit_contract(&spec)
}

fn parse_contract(module: &ItemMod) -> Result<ContractSpec, CompileError> {
    let metadata = parse_contract_metadata(module)?;
    let (_, items) = module
        .content
        .as_ref()
        .ok_or_else(|| CompileError::new("the #[contract] module must be inline"))?;

    let mut storage = None;
    let mut messages = Vec::new();
    let mut handlers = Vec::new();
    let mut getters = Vec::new();

    for item in items {
        match item {
            Item::Struct(item) if has_attribute(&item.attrs, "storage") => {
                if storage.is_some() {
                    return Err(CompileError::new(
                        "the minimal compiler supports exactly one #[storage] struct",
                    ));
                }
                storage = Some(parse_struct(item)?);
            }
            Item::Struct(item) if has_attribute(&item.attrs, "message") => {
                messages.push(MessageSpec {
                    data: parse_struct(item)?,
                    opcode: parse_message_opcode(item)?,
                });
            }
            Item::Fn(item) if has_attribute(&item.attrs, "receive") => {
                handlers.push(parse_handler(item)?);
            }
            Item::Fn(item) if has_attribute(&item.attrs, "get") => {
                getters.push(parse_getter(item)?);
            }
            _ => {}
        }
    }

    let storage = storage.ok_or_else(|| CompileError::new("missing #[storage] struct"))?;
    if messages.is_empty() {
        return Err(CompileError::new(
            "expected at least one #[message(op = ...)] struct",
        ));
    }
    if handlers.is_empty() {
        return Err(CompileError::new(
            "expected at least one #[receive] handler",
        ));
    }

    for message in &messages {
        let handler_count = handlers
            .iter()
            .filter(|handler| handler.message == message.data.name)
            .count();
        if handler_count != 1 {
            return Err(CompileError::new(format!(
                "message `{}` must have exactly one #[receive] handler",
                message.data.name
            )));
        }
    }

    Ok(ContractSpec {
        metadata,
        storage,
        messages,
        handlers,
        getters,
    })
}

fn parse_contract_metadata(module: &ItemMod) -> Result<ContractMetadata, CompileError> {
    let attribute = find_attribute(&module.attrs, "contract")
        .ok_or_else(|| CompileError::new("missing #[contract(...)] attribute"))?;

    let mut name = None;
    let mut author = None;
    let mut version = None;
    let mut description = None;

    attribute.parse_nested_meta(|meta| {
        let value = meta.value()?;
        let literal: LitStr = value.parse()?;
        if meta.path.is_ident("name") {
            name = Some(literal.value());
        } else if meta.path.is_ident("author") {
            author = Some(literal.value());
        } else if meta.path.is_ident("version") {
            version = Some(literal.value());
        } else if meta.path.is_ident("description") {
            description = Some(literal.value());
        } else {
            return Err(meta.error("unsupported contract metadata key"));
        }
        Ok(())
    })?;

    Ok(ContractMetadata {
        name: name.ok_or_else(|| CompileError::new("contract metadata requires `name`"))?,
        author: author.unwrap_or_default(),
        version: version.unwrap_or_default(),
        description: description.unwrap_or_default(),
    })
}

fn parse_struct(item: &ItemStruct) -> Result<StructSpec, CompileError> {
    let Fields::Named(fields) = &item.fields else {
        return Err(CompileError::new(format!(
            "`{}` must use named fields",
            item.ident
        )));
    };

    let fields = fields
        .named
        .iter()
        .map(|field| {
            let ident = field
                .ident
                .as_ref()
                .ok_or_else(|| CompileError::new("expected a named field"))?;
            let rust_name = ident.to_string();
            Ok(FieldSpec {
                tolk_name: to_lower_camel(&rust_name),
                ty: render_type(&field.ty)?,
            })
        })
        .collect::<Result<Vec<_>, CompileError>>()?;

    Ok(StructSpec {
        name: item.ident.to_string(),
        fields,
    })
}

fn parse_message_opcode(item: &ItemStruct) -> Result<String, CompileError> {
    let attribute = find_attribute(&item.attrs, "message")
        .ok_or_else(|| CompileError::new("missing #[message(op = ...)] attribute"))?;
    let mut opcode = None;

    attribute.parse_nested_meta(|meta| {
        if !meta.path.is_ident("op") {
            return Err(meta.error("only `op` is supported on #[message]"));
        }
        let literal: syn::LitInt = meta.value()?.parse()?;
        opcode = Some(literal.to_string());
        Ok(())
    })?;

    opcode.ok_or_else(|| {
        CompileError::new(format!(
            "message `{}` requires an explicit opcode",
            item.ident
        ))
    })
}

fn parse_handler(item: &ItemFn) -> Result<HandlerSpec, CompileError> {
    if item.sig.inputs.len() != 2 {
        return Err(CompileError::new(format!(
            "handler `{}` must accept Context<State> and one message",
            item.sig.ident
        )));
    }

    let message_argument = item
        .sig
        .inputs
        .iter()
        .nth(1)
        .ok_or_else(|| CompileError::new("missing message argument"))?;
    let FnArg::Typed(message_argument) = message_argument else {
        return Err(CompileError::new(
            "handler methods with self are not supported",
        ));
    };
    let message = simple_type_name(&message_argument.ty)?;
    let body = render_statements(&item.block.stmts)?;

    Ok(HandlerSpec { message, body })
}

fn parse_getter(item: &ItemFn) -> Result<GetterSpec, CompileError> {
    if item.sig.inputs.len() != 1 {
        return Err(CompileError::new(format!(
            "getter `{}` currently supports only ViewContext<State>",
            item.sig.ident
        )));
    }

    let ReturnType::Type(_, return_type) = &item.sig.output else {
        return Err(CompileError::new(format!(
            "getter `{}` requires a return type",
            item.sig.ident
        )));
    };
    let body = getter_expression(&item.block.stmts)?;

    Ok(GetterSpec {
        name: to_lower_camel(&item.sig.ident.to_string()),
        return_type: render_type(return_type)?,
        body,
    })
}

fn getter_expression(statements: &[Stmt]) -> Result<String, CompileError> {
    let [Stmt::Expr(expression, None)] = statements else {
        return Err(CompileError::new(
            "a minimal #[get] body must contain one trailing expression",
        ));
    };
    render_expression(expression)
}

fn render_statements(statements: &[Stmt]) -> Result<Vec<String>, CompileError> {
    statements
        .iter()
        .map(|statement| match statement {
            Stmt::Expr(expression, _) => Ok(format!("{};", render_expression(expression)?)),
            _ => Err(CompileError::new(
                "the minimal compiler supports expression statements only",
            )),
        })
        .collect()
}

fn render_expression(expression: &Expr) -> Result<String, CompileError> {
    match expression {
        Expr::Assign(assign) => Ok(format!(
            "{} = {}",
            render_expression(&assign.left)?,
            render_expression(&assign.right)?
        )),
        Expr::Binary(binary) => Ok(format!(
            "{} {} {}",
            render_expression(&binary.left)?,
            render_binary_operator(&binary.op)?,
            render_expression(&binary.right)?
        )),
        Expr::Field(field) => {
            let base = render_expression(&field.base)?;
            let syn::Member::Named(member) = &field.member else {
                return Err(CompileError::new("tuple field access is not supported yet"));
            };
            let field_name = to_lower_camel(&member.to_string());
            if base == "ctx" && field_name == "state" {
                Ok("storage".to_owned())
            } else {
                Ok(format!("{base}.{field_name}"))
            }
        }
        Expr::Lit(literal) => Ok(match &literal.lit {
            syn::Lit::Int(value) => value.to_string(),
            syn::Lit::Bool(value) => value.value.to_string(),
            _ => {
                return Err(CompileError::new(
                    "only integer and boolean literals are supported",
                ));
            }
        }),
        Expr::Paren(paren) => Ok(format!("({})", render_expression(&paren.expr)?)),
        Expr::Path(path) => {
            let segments = path
                .path
                .segments
                .iter()
                .map(|segment| to_lower_camel(&segment.ident.to_string()))
                .collect::<Vec<_>>();
            Ok(segments.join("."))
        }
        Expr::Unary(unary) => {
            let operator = match unary.op {
                syn::UnOp::Not(_) => "!",
                syn::UnOp::Neg(_) => "-",
                _ => return Err(CompileError::new("unsupported unary operator")),
            };
            Ok(format!("{operator}{}", render_expression(&unary.expr)?))
        }
        _ => Err(CompileError::new(
            "unsupported Rust expression in the minimal DSL",
        )),
    }
}

fn render_binary_operator(operator: &BinOp) -> Result<&'static str, CompileError> {
    match operator {
        BinOp::Add(_) => Ok("+"),
        BinOp::Sub(_) => Ok("-"),
        BinOp::Mul(_) => Ok("*"),
        BinOp::Div(_) => Ok("/"),
        BinOp::Rem(_) => Ok("%"),
        BinOp::And(_) => Ok("&&"),
        BinOp::Or(_) => Ok("||"),
        BinOp::BitXor(_) => Ok("^"),
        BinOp::BitAnd(_) => Ok("&"),
        BinOp::BitOr(_) => Ok("|"),
        BinOp::Shl(_) => Ok("<<"),
        BinOp::Shr(_) => Ok(">>"),
        BinOp::Eq(_) => Ok("=="),
        BinOp::Lt(_) => Ok("<"),
        BinOp::Le(_) => Ok("<="),
        BinOp::Ne(_) => Ok("!="),
        BinOp::Ge(_) => Ok(">="),
        BinOp::Gt(_) => Ok(">"),
        BinOp::AddAssign(_) => Ok("+="),
        BinOp::SubAssign(_) => Ok("-="),
        BinOp::MulAssign(_) => Ok("*="),
        BinOp::DivAssign(_) => Ok("/="),
        BinOp::RemAssign(_) => Ok("%="),
        BinOp::BitXorAssign(_) => Ok("^="),
        BinOp::BitAndAssign(_) => Ok("&="),
        BinOp::BitOrAssign(_) => Ok("|="),
        BinOp::ShlAssign(_) => Ok("<<="),
        BinOp::ShrAssign(_) => Ok(">>="),
        _ => Err(CompileError::new("unsupported binary operator")),
    }
}

fn render_type(ty: &Type) -> Result<String, CompileError> {
    match ty {
        Type::Path(path) => {
            let segment = path
                .path
                .segments
                .last()
                .ok_or_else(|| CompileError::new("expected a type name"))?;
            match segment.ident.to_string().as_str() {
                "bool" => Ok("bool".to_owned()),
                "u8" => Ok("uint8".to_owned()),
                "u16" => Ok("uint16".to_owned()),
                "u32" => Ok("uint32".to_owned()),
                "u64" => Ok("uint64".to_owned()),
                "u128" => Ok("uint128".to_owned()),
                "i8" => Ok("int8".to_owned()),
                "i16" => Ok("int16".to_owned()),
                "i32" => Ok("int32".to_owned()),
                "i64" => Ok("int64".to_owned()),
                "Option" => {
                    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                        return Err(CompileError::new("Option requires one type argument"));
                    };
                    let inner = arguments
                        .args
                        .iter()
                        .find_map(|argument| match argument {
                            GenericArgument::Type(inner) => Some(inner),
                            _ => None,
                        })
                        .ok_or_else(|| CompileError::new("Option requires one type argument"))?;
                    Ok(format!("{}?", render_type(inner)?))
                }
                unsupported => Err(CompileError::new(format!(
                    "unsupported Rust type `{unsupported}` in the minimal DSL"
                ))),
            }
        }
        _ => Err(CompileError::new(
            "only path types are supported in the minimal DSL",
        )),
    }
}

fn simple_type_name(ty: &Type) -> Result<String, CompileError> {
    let Type::Path(path) = ty else {
        return Err(CompileError::new("expected a named message type"));
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .ok_or_else(|| CompileError::new("expected a named message type"))
}

fn emit_contract(spec: &ContractSpec) -> Result<String, CompileError> {
    let mut output = String::new();
    let incoming_messages = spec
        .messages
        .iter()
        .map(|message| message.data.name.as_str())
        .collect::<Vec<_>>()
        .join(" | ");

    writeln!(output, "contract {} {{", spec.metadata.name).map_err(format_error)?;
    writeln!(
        output,
        "    author: \"{}\"",
        escape_tolk_string(&spec.metadata.author)
    )
    .map_err(format_error)?;
    writeln!(
        output,
        "    version: \"{}\"",
        escape_tolk_string(&spec.metadata.version)
    )
    .map_err(format_error)?;
    writeln!(
        output,
        "    description: \"{}\"",
        escape_tolk_string(&spec.metadata.description)
    )
    .map_err(format_error)?;
    writeln!(output, "    incomingMessages: AllowedMessage").map_err(format_error)?;
    writeln!(output, "    storage: {}", spec.storage.name).map_err(format_error)?;
    writeln!(output, "}}\n").map_err(format_error)?;

    emit_struct(&mut output, &spec.storage, None)?;
    for message in &spec.messages {
        emit_struct(&mut output, &message.data, Some(&message.opcode))?;
    }

    writeln!(output, "type AllowedMessage = {incoming_messages}\n").map_err(format_error)?;
    writeln!(
        output,
        "fun {}.load(): {} {{",
        spec.storage.name, spec.storage.name
    )
    .map_err(format_error)?;
    writeln!(
        output,
        "    return {}.fromCell(contract.getData());",
        spec.storage.name
    )
    .map_err(format_error)?;
    writeln!(output, "}}\n").map_err(format_error)?;
    writeln!(output, "fun {}.save(self) {{", spec.storage.name).map_err(format_error)?;
    writeln!(output, "    contract.setData(self.toCell());").map_err(format_error)?;
    writeln!(output, "}}\n").map_err(format_error)?;

    writeln!(output, "fun onInternalMessage(in: InMessage) {{").map_err(format_error)?;
    writeln!(
        output,
        "    val msg = lazy AllowedMessage.fromSlice(in.body);\n"
    )
    .map_err(format_error)?;
    writeln!(output, "    match (msg) {{").map_err(format_error)?;
    for handler in &spec.handlers {
        writeln!(output, "        {} => {{", handler.message).map_err(format_error)?;
        writeln!(
            output,
            "            var storage = lazy {}.load();",
            spec.storage.name
        )
        .map_err(format_error)?;
        for statement in &handler.body {
            writeln!(output, "            {statement}").map_err(format_error)?;
        }
        writeln!(output, "            storage.save();").map_err(format_error)?;
        writeln!(output, "        }}").map_err(format_error)?;
    }
    writeln!(output, "        else => {{").map_err(format_error)?;
    writeln!(
        output,
        "            assert (in.body.isEmpty()) throw 0xFFFF;"
    )
    .map_err(format_error)?;
    writeln!(output, "        }}").map_err(format_error)?;
    writeln!(output, "    }}").map_err(format_error)?;
    writeln!(output, "}}\n").map_err(format_error)?;

    for getter in &spec.getters {
        writeln!(
            output,
            "get fun {}(): {} {{",
            getter.name, getter.return_type
        )
        .map_err(format_error)?;
        writeln!(
            output,
            "    val storage = lazy {}.load();",
            spec.storage.name
        )
        .map_err(format_error)?;
        writeln!(output, "    return {};", getter.body).map_err(format_error)?;
        writeln!(output, "}}\n").map_err(format_error)?;
    }

    Ok(output)
}

fn emit_struct(
    output: &mut String,
    spec: &StructSpec,
    prefix: Option<&str>,
) -> Result<(), CompileError> {
    match prefix {
        Some(prefix) => writeln!(output, "struct ({prefix}) {} {{", spec.name),
        None => writeln!(output, "struct {} {{", spec.name),
    }
    .map_err(format_error)?;
    for field in &spec.fields {
        writeln!(output, "    {}: {}", field.tolk_name, field.ty).map_err(format_error)?;
    }
    writeln!(output, "}}\n").map_err(format_error)?;
    Ok(())
}

fn find_attribute<'attrs>(
    attributes: &'attrs [Attribute],
    name: &str,
) -> Option<&'attrs Attribute> {
    attributes
        .iter()
        .find(|attribute| attribute.path().is_ident(name))
}

fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    find_attribute(attributes, name).is_some()
}

fn escape_tolk_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn to_lower_camel(value: &str) -> String {
    let mut parts = value.split('_');
    let mut result = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            result.extend(first.to_uppercase());
            result.extend(characters);
        }
    }
    result
}

fn format_error(_: fmt::Error) -> CompileError {
    CompileError::new("failed to format generated Tolk")
}
