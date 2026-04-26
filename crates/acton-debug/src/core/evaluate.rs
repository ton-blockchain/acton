use crate::replayer::LocalVarRendered;
use crate::types_render::{RenderedValue, render_cell_like_as_type};
use anyhow::{Result, anyhow, bail};
use num_bigint::BigInt;
use std::cmp::Ordering;
use tolk_compiler::source_map::{Declaration, SourceMap};
use tolk_compiler::types_kernel::Ty;
use tolk_syntax::{
    AstNode, DotAccessField, Expr, FuncBody, FunctionLike, Stmt, TopLevel, Type,
    parse_tolk_int_literal,
};
use tvm_logs::parser::CellLike;

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSegment {
    Field(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedValuePath {
    root: String,
    segments: Vec<PathSegment>,
}

pub(crate) fn evaluate_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    expression: &str,
) -> Result<RenderedValue> {
    let source_file = parse_wrapped_source(expression)?;
    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    evaluate_parsed_expression(locals, source_map, expr, source_file.source.as_ref())
}

pub(crate) fn evaluate_condition_expression(
    locals: &[LocalVarRendered],
    expression: &str,
) -> Result<bool> {
    let source_file = parse_wrapped_source(expression)?;
    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    evaluate_boolean_expression(locals, None, expr, source_file.source.as_ref())
}

fn resolve_locals_path(
    locals: &[LocalVarRendered],
    path: &ParsedValuePath,
) -> Result<RenderedValue> {
    let root = locals
        .iter()
        .rev()
        .find(|local| normalize_identifier(&local.var_name) == path.root)
        .ok_or_else(|| anyhow!("Variable `{}` is not in scope", path.root))?;

    resolve_value_path(&root.value, &path.segments)
}

fn resolve_value_path(root: &RenderedValue, segments: &[PathSegment]) -> Result<RenderedValue> {
    let mut current = root.clone();
    for segment in segments {
        current = resolve_segment(&current, segment)?;
    }
    Ok(current)
}

fn resolve_segment(value: &RenderedValue, segment: &PathSegment) -> Result<RenderedValue> {
    let value = unwrap_visible_value(value);
    match segment {
        PathSegment::Field(name) => match value {
            RenderedValue::Struct { fields, .. }
            | RenderedValue::Address { fields, .. }
            | RenderedValue::CellLike { fields, .. }
            | RenderedValue::CellOf { fields, .. }
            | RenderedValue::EnumValue { fields, .. }
            | RenderedValue::UnionCase { fields, .. } => fields
                .iter()
                .find(|(field_name, _)| field_name == name)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| anyhow!("Field `{name}` is not available on `{value}`")),
            _ => bail!("Cannot access field `{name}` on `{value}`"),
        },
        PathSegment::Index(index) => match value {
            RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => items
                .get(*index)
                .cloned()
                .ok_or_else(|| anyhow!("Index {index} is out of bounds for `{value}`")),
            _ => bail!("Cannot index into `{value}`"),
        },
    }
}

fn unwrap_visible_value(mut value: &RenderedValue) -> &RenderedValue {
    loop {
        match value {
            RenderedValue::LastSeen { inner }
            | RenderedValue::LazyNotYetLoaded { preview: inner } => {
                value = inner;
            }
            _ => return value,
        }
    }
}

fn normalize_identifier(identifier: &str) -> &str {
    identifier
        .strip_prefix('`')
        .and_then(|inner| inner.strip_suffix('`'))
        .unwrap_or(identifier)
}

fn parse_wrapped_source(input: &str) -> Result<tolk_syntax::SourceFile> {
    let wrapped = format!("fun __acton_debug_eval__() {{ {input}; }}");
    let source_file = tolk_syntax::parse(&wrapped)?;
    if source_file.has_errors() {
        bail!("syntax error");
    }
    Ok(source_file)
}

fn wrapped_expression(source_file: &tolk_syntax::SourceFile) -> Option<Expr<'_>> {
    let TopLevel::Func(func) = source_file.top_levels().next()? else {
        return None;
    };
    let FuncBody::Block(body) = func.body()? else {
        return None;
    };
    let Stmt::ExprStmt(stmt) = body.stmts().next()? else {
        return None;
    };
    stmt.expr()
}

fn parse_expr_to_path(expr: Expr<'_>, source: &str) -> Result<ParsedValuePath> {
    match expr {
        Expr::Ident(ident) => Ok(ParsedValuePath {
            root: ident.normalized_name(source).to_owned(),
            segments: Vec::new(),
        }),
        Expr::Call(_) => bail!("function calls are not supported"),
        Expr::Paren(paren) => {
            let inner = paren
                .inner()
                .ok_or_else(|| anyhow!("expected expression inside parentheses"))?;
            parse_expr_to_path(inner, source)
        }
        Expr::NotNull(not_null) => {
            let inner = not_null
                .inner()
                .ok_or_else(|| anyhow!("expected expression before `!`"))?;
            parse_expr_to_path(inner, source)
        }
        Expr::DotAccess(dot_access) => {
            let obj = dot_access
                .obj()
                .ok_or_else(|| anyhow!("expected expression before `.`"))?;
            let field = dot_access
                .field()
                .ok_or_else(|| anyhow!("expected field or numeric index after `.`"))?;

            let mut path = parse_expr_to_path(obj, source)?;
            match field {
                DotAccessField::Ident(ident) => path
                    .segments
                    .push(PathSegment::Field(ident.normalized_name(source).to_owned())),
                DotAccessField::NumericIndex(index) => {
                    path.segments.push(PathSegment::Index(parse_numeric_index(
                        index.value(source),
                    )?));
                }
            }
            Ok(path)
        }
        _ => bail!("expected a variable path"),
    }
}

fn parse_numeric_index(raw: &str) -> Result<usize> {
    raw.parse::<usize>()
        .map_err(|err| anyhow!("invalid numeric index `{raw}`: {err}"))
}

fn evaluate_parsed_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    expr: Expr<'_>,
    source: &str,
) -> Result<RenderedValue> {
    match expr {
        Expr::BoolLit(bool_lit) => Ok(render_bool(bool_lit.value())),
        Expr::NumberLit(number_lit) => render_number_literal(number_lit.text(source)),
        Expr::StringLit(string_lit) => Ok(render_string_literal(string_lit.text(source))),
        Expr::NullLit(_) => Ok(render_null()),
        Expr::Paren(paren) => {
            let inner = paren
                .inner()
                .ok_or_else(|| anyhow!("expected expression inside parentheses"))?;
            evaluate_parsed_expression(locals, source_map, inner, source)
        }
        Expr::Unary(unary) => evaluate_unary_expression(locals, source_map, &unary, source),
        Expr::AsCast(as_cast) => evaluate_as_cast_expression(locals, source_map, &as_cast, source),
        Expr::Bin(bin) => match bin.operator_name(source) {
            "&&" => {
                let left = bin
                    .left()
                    .ok_or_else(|| anyhow!("expected left operand for `&&`"))?;
                let left = evaluate_boolean_expression(locals, source_map, left, source)?;
                if !left {
                    return Ok(render_bool(false));
                }

                let right = bin
                    .right()
                    .ok_or_else(|| anyhow!("expected right operand for `&&`"))?;
                Ok(render_bool(evaluate_boolean_expression(
                    locals, source_map, right, source,
                )?))
            }
            "||" => {
                let left = bin
                    .left()
                    .ok_or_else(|| anyhow!("expected left operand for `||`"))?;
                let left = evaluate_boolean_expression(locals, source_map, left, source)?;
                if left {
                    return Ok(render_bool(true));
                }

                let right = bin
                    .right()
                    .ok_or_else(|| anyhow!("expected right operand for `||`"))?;
                Ok(render_bool(evaluate_boolean_expression(
                    locals, source_map, right, source,
                )?))
            }
            "==" => evaluate_equality_expression(locals, source_map, &bin, source, true),
            "!=" => evaluate_equality_expression(locals, source_map, &bin, source, false),
            "<" | "<=" | ">" | ">=" => {
                evaluate_ordering_expression(locals, source_map, &bin, source)
            }
            operator => bail!("binary operator `{operator}` is not supported"),
        },
        _ => {
            let path = parse_expr_to_path(expr, source)?;
            resolve_locals_path(locals, &path)
        }
    }
}

fn evaluate_unary_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    unary: &tolk_syntax::Unary<'_>,
    source: &str,
) -> Result<RenderedValue> {
    let operator = unary.operator_name(source);
    let argument = unary
        .argument()
        .ok_or_else(|| anyhow!("expected expression after `{operator}`"))?;

    match operator {
        "!" => {
            let value = evaluate_parsed_expression(locals, source_map, argument, source)?;
            Ok(render_bool(!rendered_value_as_bool(&value)?))
        }
        "-" => {
            let value = evaluate_parsed_expression(locals, source_map, argument, source)?;
            let number = parse_rendered_number(&value)
                .ok_or_else(|| anyhow!("unary operator `-` requires numeric operand"))?;
            Ok(RenderedValue::typed_leaf((-number).to_string(), "int"))
        }
        _ => bail!("unary operator `{operator}` is not supported"),
    }
}

fn evaluate_boolean_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    expr: Expr<'_>,
    source: &str,
) -> Result<bool> {
    let value = evaluate_parsed_expression(locals, source_map, expr, source)?;
    rendered_value_as_bool(&value)
}

fn rendered_value_as_bool(value: &RenderedValue) -> Result<bool> {
    match unwrap_visible_value(value) {
        RenderedValue::Leaf { value, .. } if value == "true" => Ok(true),
        RenderedValue::Leaf { value, .. } if value == "false" => Ok(false),
        other => bail!("logical operators require boolean operands, got `{other}`"),
    }
}

fn evaluate_equality_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    bin: &tolk_syntax::Bin<'_>,
    source: &str,
    expected_equal: bool,
) -> Result<RenderedValue> {
    let left = bin
        .left()
        .ok_or_else(|| anyhow!("expected left operand for equality comparison"))?;
    let right = bin
        .right()
        .ok_or_else(|| anyhow!("expected right operand for equality comparison"))?;

    let left = evaluate_parsed_expression(locals, source_map, left, source)?;
    let right = evaluate_parsed_expression(locals, source_map, right, source)?;
    let equal = rendered_value_text(&left) == rendered_value_text(&right);
    Ok(render_bool(equal == expected_equal))
}

fn evaluate_ordering_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    bin: &tolk_syntax::Bin<'_>,
    source: &str,
) -> Result<RenderedValue> {
    let operator = bin.operator_name(source);
    let left = bin
        .left()
        .ok_or_else(|| anyhow!("expected left operand for `{operator}`"))?;
    let right = bin
        .right()
        .ok_or_else(|| anyhow!("expected right operand for `{operator}`"))?;

    let left = evaluate_parsed_expression(locals, source_map, left, source)?;
    let right = evaluate_parsed_expression(locals, source_map, right, source)?;
    let comparison = compare_rendered_values_as_numbers(&left, &right, operator)?;

    let result = match operator {
        "<" => comparison == Ordering::Less,
        "<=" => comparison != Ordering::Greater,
        ">" => comparison == Ordering::Greater,
        ">=" => comparison != Ordering::Less,
        _ => unreachable!("unsupported ordering operator"),
    };
    Ok(render_bool(result))
}

fn evaluate_as_cast_expression(
    locals: &[LocalVarRendered],
    source_map: Option<&SourceMap>,
    as_cast: &tolk_syntax::AsCast<'_>,
    source: &str,
) -> Result<RenderedValue> {
    let expr = as_cast
        .expr()
        .ok_or_else(|| anyhow!("expected expression before `as`"))?;
    let target = as_cast
        .casted_to()
        .ok_or_else(|| anyhow!("expected type after `as`"))?;
    let source_map = source_map
        .ok_or_else(|| anyhow!("type casts are not supported in this debugger context"))?;

    let value = evaluate_parsed_expression(locals, Some(source_map), expr, source)?;
    let ty = lower_evaluate_cast_type(target, source, source_map)?;

    match &ty {
        Ty::CellOf { .. } => {
            render_cell_like_as_type(source_map, &value, &ty).map_err(anyhow::Error::msg)
        }
        _ => bail!("Debugger evaluate currently supports only casts to `Cell<T>`"),
    }
}

fn lower_evaluate_cast_type(ty: Type<'_>, source: &str, source_map: &SourceMap) -> Result<Ty> {
    match ty {
        Type::TypeIdent(ident) => {
            lower_named_or_primitive_type(ident.text(source), None, source_map)
        }
        Type::TypeInstantiatedTs(inst) => {
            let name = inst
                .name()
                .ok_or_else(|| anyhow!("expected type name"))?
                .text(source);
            let type_args = inst
                .arguments()
                .map(|args| {
                    args.types()
                        .map(|arg| lower_evaluate_cast_type(arg, source, source_map))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?;
            lower_named_or_primitive_type(name, type_args, source_map)
        }
        Type::ParenthesizedType(paren) => {
            let inner = paren
                .inner()
                .ok_or_else(|| anyhow!("expected type inside parentheses"))?;
            lower_evaluate_cast_type(inner, source, source_map)
        }
        Type::NullableType(nullable) => {
            let inner = nullable
                .inner()
                .ok_or_else(|| anyhow!("expected type before `?`"))?;
            Ok(Ty::Nullable {
                inner: Box::new(lower_evaluate_cast_type(inner, source, source_map)?),
                stack_type_id: None,
                stack_width: None,
            })
        }
        Type::TensorType(tensor) => Ok(Ty::Tensor {
            items: tensor
                .elements()
                .map(|item| lower_evaluate_cast_type(item, source, source_map))
                .collect::<Result<Vec<_>>>()?,
        }),
        Type::TupleType(tuple) => Ok(Ty::ShapedTuple {
            items: tuple
                .elements()
                .map(|item| lower_evaluate_cast_type(item, source, source_map))
                .collect::<Result<Vec<_>>>()?,
        }),
        Type::UnionType(_) => bail!(
            "inline union types are not supported in debugger evaluate casts; cast to a named ABI type instead"
        ),
        Type::FunCallableType(_) => {
            bail!("callable types are not supported in debugger evaluate casts")
        }
        Type::NullLit(_) => Ok(Ty::NullLiteral),
        Type::Unmapped(raw) => bail!(
            "unsupported cast type `{}`",
            raw.0
                .utf8_text(source.as_bytes())
                .unwrap_or("<invalid utf8>")
        ),
    }
}

fn lower_named_or_primitive_type(
    name: &str,
    type_args: Option<Vec<Ty>>,
    source_map: &SourceMap,
) -> Result<Ty> {
    if let Some(ty) = lower_primitive_type(name, type_args.as_deref())? {
        return Ok(ty);
    }

    match resolve_named_declaration_kind(source_map, name) {
        Some(NamedDeclarationKind::Struct) => Ok(Ty::StructRef {
            struct_name: name.to_owned(),
            type_args,
        }),
        Some(NamedDeclarationKind::Alias) => Ok(Ty::AliasRef {
            alias_name: name.to_owned(),
            type_args,
        }),
        Some(NamedDeclarationKind::Enum) => {
            if type_args.as_ref().is_some_and(|args| !args.is_empty()) {
                bail!("enum `{name}` does not take type arguments");
            }
            Ok(Ty::EnumRef {
                enum_name: name.to_owned(),
            })
        }
        None => bail!("type `{name}` is not known in the current SourceMap"),
    }
}

fn lower_primitive_type(name: &str, type_args: Option<&[Ty]>) -> Result<Option<Ty>> {
    let primitive = match name {
        "int" => Some(Ty::Int),
        "coins" => Some(Ty::Coins),
        "bool" => Some(Ty::Bool),
        "cell" | "Cell" if type_args.is_none() => Some(Ty::Cell),
        "builder" => Some(Ty::Builder),
        "slice" => Some(Ty::Slice),
        "string" => Some(Ty::String),
        "RemainingBitsAndRefs" => Some(Ty::Remaining),
        "address" => Some(Ty::Address),
        "ext_address" => Some(Ty::AddressExt),
        "any_address" => Some(Ty::AddressAny),
        "null" => Some(Ty::NullLiteral),
        "Cell" => {
            let [inner] = type_args.unwrap_or_default() else {
                bail!("`Cell` expects exactly one type argument");
            };
            Some(Ty::CellOf {
                inner: Box::new(inner.clone()),
            })
        }
        "array" => {
            let [inner] = type_args.unwrap_or_default() else {
                bail!("`array` expects exactly one type argument");
            };
            Some(Ty::ArrayOf {
                inner: Box::new(inner.clone()),
            })
        }
        "lisp_list" => {
            let [inner] = type_args.unwrap_or_default() else {
                bail!("`lisp_list` expects exactly one type argument");
            };
            Some(Ty::LispListOf {
                inner: Box::new(inner.clone()),
            })
        }
        "map" => {
            let [key, value] = type_args.unwrap_or_default() else {
                bail!("`map` expects exactly two type arguments");
            };
            Some(Ty::MapKV {
                k: Box::new(key.clone()),
                v: Box::new(value.clone()),
            })
        }
        _ => parse_sized_primitive_type(name),
    };

    Ok(primitive)
}

fn parse_sized_primitive_type(name: &str) -> Option<Ty> {
    fn parse_suffix(name: &str, prefix: &str) -> Option<u32> {
        name.strip_prefix(prefix)
            .filter(|suffix| !suffix.is_empty())
            .and_then(|suffix| suffix.parse::<u32>().ok())
    }

    if let Some(n) = parse_suffix(name, "int") {
        return Some(Ty::IntN { n });
    }
    if let Some(n) = parse_suffix(name, "uint") {
        return Some(Ty::UintN { n });
    }
    if let Some(n) = parse_suffix(name, "varint") {
        return Some(Ty::VarintN { n });
    }
    if let Some(n) = parse_suffix(name, "varuint") {
        return Some(Ty::VaruintN { n });
    }
    if let Some(n) = parse_suffix(name, "bits") {
        return Some(Ty::BitsN { n });
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedDeclarationKind {
    Struct,
    Alias,
    Enum,
}

fn resolve_named_declaration_kind(
    source_map: &SourceMap,
    name: &str,
) -> Option<NamedDeclarationKind> {
    source_map
        .declarations()
        .iter()
        .find_map(|decl| match decl {
            Declaration::Struct(decl) if decl.name == name => Some(NamedDeclarationKind::Struct),
            Declaration::Alias(decl) if decl.name == name => Some(NamedDeclarationKind::Alias),
            Declaration::Enum(decl) if decl.name == name => Some(NamedDeclarationKind::Enum),
            _ => None,
        })
}

fn compare_rendered_values_as_numbers(
    left: &RenderedValue,
    right: &RenderedValue,
    operator: &str,
) -> Result<Ordering> {
    let left = parse_rendered_number(left)
        .ok_or_else(|| anyhow!("operator `{operator}` requires numeric operands"))?;
    let right = parse_rendered_number(right)
        .ok_or_else(|| anyhow!("operator `{operator}` requires numeric operands"))?;

    Ok(left.cmp(&right))
}

fn parse_rendered_number(value: &RenderedValue) -> Option<BigInt> {
    let text = rendered_value_text(value);
    text.parse::<BigInt>().ok()
}

fn rendered_raw_field_text(value: &RenderedValue) -> Option<String> {
    let (RenderedValue::CellLike { fields, .. } | RenderedValue::CellOf { fields, .. }) = value
    else {
        return None;
    };
    fields
        .iter()
        .find(|(name, _)| name == "raw")
        .map(|(_, raw_value)| raw_value.to_string())
}

fn rendered_value_text(value: &RenderedValue) -> String {
    let value = unwrap_visible_value(value);
    if let Some(raw_value) = rendered_raw_field_text(value) {
        return raw_value;
    }

    match value {
        RenderedValue::CellLike {
            type_name,
            raw: Some(CellLike::Cell(hex)),
            ..
        } if type_name != "slice" => format!("cell{{{hex}}}"),
        RenderedValue::CellOf {
            raw: Some(CellLike::Cell(hex)),
            ..
        } => format!("cell{{{hex}}}"),
        other => other.to_string(),
    }
}

fn render_number_literal(raw: &str) -> Result<RenderedValue> {
    let parsed = parse_tolk_int_literal(raw)
        .ok_or_else(|| anyhow!("numeric literal `{raw}` is not supported"))?;
    let normalized = match parsed.radix() {
        10 => parsed.digits().to_owned(),
        16 => BigInt::parse_bytes(parsed.digits().as_bytes(), 16)
            .ok_or_else(|| anyhow!("numeric literal `{raw}` is not supported"))?
            .to_string(),
        2 => BigInt::parse_bytes(parsed.digits().as_bytes(), 2)
            .ok_or_else(|| anyhow!("numeric literal `{raw}` is not supported"))?
            .to_string(),
        _ => bail!("numeric literal `{raw}` is not supported"),
    };

    Ok(RenderedValue::typed_leaf(normalized, "int"))
}

fn render_string_literal(raw: &str) -> RenderedValue {
    RenderedValue::typed_leaf(raw, "string")
}

fn render_null() -> RenderedValue {
    RenderedValue::typed_leaf("null", "null")
}

fn render_bool(value: bool) -> RenderedValue {
    RenderedValue::typed_leaf(value.to_string(), "bool")
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedValuePath, PathSegment, evaluate_condition_expression, evaluate_expression,
        parse_expr_to_path, parse_wrapped_source, rendered_value_text, wrapped_expression,
    };
    use crate::replayer::LocalVarRendered;
    use crate::types_render::{RenderedValue, render_runtime_vm_value};
    use anyhow::anyhow;
    use tolk_compiler::source_map::SourceMap;
    use tvm_logs::parser::{CellLike, CellSlice, VmStackValue};
    use tycho_types::boc::Boc;
    use tycho_types::cell::CellBuilder;

    fn parse_value_path(input: &str) -> anyhow::Result<ParsedValuePath> {
        let source_file = parse_wrapped_source(input)?;
        let expr = wrapped_expression(&source_file)
            .ok_or_else(|| anyhow!("expected a single expression statement"))?;
        parse_expr_to_path(expr, source_file.source.as_ref())
    }

    fn foo_source_map() -> SourceMap {
        serde_json::from_value(serde_json::json!({
            "files": [],
            "declarations": [{
                "kind": "struct",
                "name": "Foo",
                "ident_loc": [0, 0, 0, 0, 0],
                "fields": [{
                    "name": "value",
                    "ty": {"kind": "uintN", "n": 32}
                }]
            }],
            "unique_ty": [],
            "functions": [],
            "debug_marks": []
        }))
        .expect("valid source map")
    }

    fn foo_value_cell() -> tycho_types::cell::Cell {
        let mut builder = CellBuilder::new();
        builder.store_u32(42).expect("must store field");
        builder.build().expect("must build cell")
    }

    #[test]
    fn parses_dot_and_numeric_index_segments() {
        let path = parse_value_path(" foo.bar.0.baz.1 ").expect("path should parse");
        assert_eq!(
            path,
            ParsedValuePath {
                root: "foo".to_owned(),
                segments: vec![
                    PathSegment::Field("bar".to_owned()),
                    PathSegment::Index(0),
                    PathSegment::Field("baz".to_owned()),
                    PathSegment::Index(1),
                ],
            }
        );
    }

    #[test]
    fn parses_backticked_identifiers_and_not_null_operators() {
        let path = parse_value_path("`foo bar`!.`child value`.2!.baz").expect("path should parse");
        assert_eq!(
            path,
            ParsedValuePath {
                root: "foo bar".to_owned(),
                segments: vec![
                    PathSegment::Field("child value".to_owned()),
                    PathSegment::Index(2),
                    PathSegment::Field("baz".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn resolves_nested_fields_indices_and_last_seen_values() {
        let locals = vec![LocalVarRendered {
            var_name: "foo".to_owned(),
            value: RenderedValue::Struct {
                type_name: "Root".to_owned(),
                fields: vec![(
                    "bar".to_owned(),
                    RenderedValue::ArrayOf {
                        type_name: "Bar[]".to_owned(),
                        items: vec![RenderedValue::LastSeen {
                            inner: Box::new(RenderedValue::Struct {
                                type_name: "Leaf".to_owned(),
                                fields: vec![(
                                    "baz".to_owned(),
                                    RenderedValue::typed_leaf("42", "int"),
                                )],
                            }),
                        }],
                    },
                )],
            },
        }];

        let value =
            evaluate_expression(&locals, None, "foo.bar.0.baz").expect("path should resolve");
        assert_eq!(value.to_string(), "42");
    }

    #[test]
    fn prefers_last_visible_variable_when_names_shadow() {
        let locals = vec![
            LocalVarRendered {
                var_name: "foo".to_owned(),
                value: RenderedValue::typed_leaf("1", "int"),
            },
            LocalVarRendered {
                var_name: "foo".to_owned(),
                value: RenderedValue::typed_leaf("2", "int"),
            },
        ];

        let value = evaluate_expression(&locals, None, "foo").expect("path should resolve");
        assert_eq!(value.to_string(), "2");
    }

    #[test]
    fn rejects_function_calls_explicitly() {
        let err = parse_value_path("foo()").expect_err("call syntax should not parse");
        assert_eq!(err.to_string(), "function calls are not supported");
    }

    #[test]
    fn rejects_function_calls_inside_paths_explicitly() {
        let err = parse_value_path("foo().bar").expect_err("call syntax should not parse");
        assert_eq!(err.to_string(), "function calls are not supported");
    }

    #[test]
    fn rejects_bracket_index_syntax() {
        let err = parse_value_path("foo[0]").expect_err("bracket syntax should not parse");
        assert_eq!(err.to_string(), "expected a variable path");
    }

    #[test]
    fn evaluates_basic_logical_operators() {
        let locals = vec![LocalVarRendered {
            var_name: "foo".to_owned(),
            value: RenderedValue::Struct {
                type_name: "Flags".to_owned(),
                fields: vec![
                    (
                        "enabled".to_owned(),
                        RenderedValue::typed_leaf("true", "bool"),
                    ),
                    (
                        "blocked".to_owned(),
                        RenderedValue::typed_leaf("false", "bool"),
                    ),
                ],
            },
        }];

        let value = evaluate_expression(&locals, None, "foo.enabled && (!foo.blocked || false)")
            .expect("logical expression should resolve");
        assert_eq!(value.to_string(), "true");
    }

    #[test]
    fn evaluates_boolean_conditions() {
        let locals = vec![LocalVarRendered {
            var_name: "flag".to_owned(),
            value: RenderedValue::typed_leaf("true", "bool"),
        }];

        assert!(
            evaluate_condition_expression(&locals, "flag && true")
                .expect("boolean condition should resolve")
        );
    }

    #[test]
    fn logical_operators_short_circuit() {
        let locals = Vec::new();

        let value = evaluate_expression(&locals, None, "false && missing.flag")
            .expect("short-circuit should avoid rhs lookup");
        assert_eq!(value.to_string(), "false");

        let value = evaluate_expression(&locals, None, "true || missing.flag")
            .expect("short-circuit should avoid rhs lookup");
        assert_eq!(value.to_string(), "true");
    }

    #[test]
    fn rejects_non_boolean_logical_operands() {
        let locals = vec![LocalVarRendered {
            var_name: "count".to_owned(),
            value: RenderedValue::typed_leaf("42", "int"),
        }];

        let err = evaluate_expression(&locals, None, "count && true")
            .expect_err("non-boolean operand should be rejected");
        assert_eq!(
            err.to_string(),
            "logical operators require boolean operands, got `42`"
        );
    }

    #[test]
    fn evaluates_equality_and_inequality_by_rendered_text() {
        let locals = vec![
            LocalVarRendered {
                var_name: "lhs".to_owned(),
                value: RenderedValue::typed_leaf("42", "int"),
            },
            LocalVarRendered {
                var_name: "rhs".to_owned(),
                value: RenderedValue::typed_leaf("42", "uint32"),
            },
            LocalVarRendered {
                var_name: "other".to_owned(),
                value: RenderedValue::typed_leaf("7", "int"),
            },
        ];

        let value =
            evaluate_expression(&locals, None, "lhs == rhs").expect("equality should resolve");
        assert_eq!(value.to_string(), "true");

        let value =
            evaluate_expression(&locals, None, "lhs != other").expect("inequality should resolve");
        assert_eq!(value.to_string(), "true");
    }

    #[test]
    fn evaluates_numeric_comparisons() {
        let locals = vec![
            LocalVarRendered {
                var_name: "small".to_owned(),
                value: RenderedValue::typed_leaf("7", "int"),
            },
            LocalVarRendered {
                var_name: "big".to_owned(),
                value: RenderedValue::typed_leaf("42", "int"),
            },
        ];

        assert_eq!(
            evaluate_expression(&locals, None, "small < big")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "big >= small")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "big >= 42")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "small > -1")
                .expect("negative literal comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "-small < 0")
                .expect("unary minus on variables should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "small < 10")
                .expect("literal comparison should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn evaluates_numeric_comparisons_for_last_seen_values() {
        let locals = vec![
            LocalVarRendered {
                var_name: "msg".to_owned(),
                value: RenderedValue::Struct {
                    type_name: "Msg".to_owned(),
                    fields: vec![(
                        "itemIndex".to_owned(),
                        RenderedValue::LastSeen {
                            inner: Box::new(RenderedValue::typed_leaf("7", "uint32")),
                        },
                    )],
                },
            },
            LocalVarRendered {
                var_name: "storage".to_owned(),
                value: RenderedValue::LastSeen {
                    inner: Box::new(RenderedValue::Struct {
                        type_name: "Storage".to_owned(),
                        fields: vec![(
                            "nextItemIndex".to_owned(),
                            RenderedValue::typed_leaf("8", "uint32"),
                        )],
                    }),
                },
            },
        ];

        assert_eq!(
            evaluate_expression(&locals, None, "msg.itemIndex <= storage.nextItemIndex")
                .expect("last seen numeric comparison should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn evaluates_numeric_comparisons_for_lazy_preview_values() {
        let locals = vec![
            LocalVarRendered {
                var_name: "msg".to_owned(),
                value: RenderedValue::Struct {
                    type_name: "Msg".to_owned(),
                    fields: vec![(
                        "itemIndex".to_owned(),
                        RenderedValue::LazyNotYetLoaded {
                            preview: Box::new(RenderedValue::typed_leaf("7", "uint32")),
                        },
                    )],
                },
            },
            LocalVarRendered {
                var_name: "storage".to_owned(),
                value: RenderedValue::LazyNotYetLoaded {
                    preview: Box::new(RenderedValue::Struct {
                        type_name: "Storage".to_owned(),
                        fields: vec![(
                            "nextItemIndex".to_owned(),
                            RenderedValue::typed_leaf("8", "uint32"),
                        )],
                    }),
                },
            },
        ];

        assert_eq!(
            evaluate_expression(&locals, None, "msg.itemIndex <= storage.nextItemIndex")
                .expect("lazy preview numeric comparison should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn resolves_field_access_through_lazy_preview_struct() {
        let locals = vec![LocalVarRendered {
            var_name: "storage".to_owned(),
            value: RenderedValue::LazyNotYetLoaded {
                preview: Box::new(RenderedValue::Struct {
                    type_name: "Storage".to_owned(),
                    fields: vec![(
                        "nextItemIndex".to_owned(),
                        RenderedValue::typed_leaf("8", "uint32"),
                    )],
                }),
            },
        }];

        let value = evaluate_expression(&locals, None, "storage.nextItemIndex")
            .expect("field access through lazy preview should resolve");
        assert_eq!(value.to_string(), "8");
    }

    #[test]
    fn rejects_non_numeric_ordering_operands() {
        let locals = vec![LocalVarRendered {
            var_name: "name".to_owned(),
            value: RenderedValue::typed_leaf("alice", "string"),
        }];

        let err = evaluate_expression(&locals, None, "name < 10")
            .expect_err("non-numeric ordering operand should be rejected");
        assert_eq!(err.to_string(), "operator `<` requires numeric operands");
    }

    #[test]
    fn evaluates_string_literals_and_compares_them() {
        let locals = vec![LocalVarRendered {
            var_name: "name".to_owned(),
            value: RenderedValue::typed_leaf("\"alice\"", "string"),
        }];

        assert_eq!(
            evaluate_expression(&locals, None, "\"alice\"")
                .expect("string literal should resolve")
                .to_string(),
            "\"alice\""
        );
        assert_eq!(
            evaluate_expression(&locals, None, "name == \"alice\"")
                .expect("string equality should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "name != \"bob\"")
                .expect("string inequality should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn evaluates_null_literals_and_compares_them() {
        let locals = vec![
            LocalVarRendered {
                var_name: "missing".to_owned(),
                value: RenderedValue::typed_leaf("null", "address?"),
            },
            LocalVarRendered {
                var_name: "present".to_owned(),
                value: RenderedValue::typed_leaf("7", "int"),
            },
        ];

        assert_eq!(
            evaluate_expression(&locals, None, "null")
                .expect("null literal should resolve")
                .to_string(),
            "null"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "missing == null")
                .expect("null equality should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, None, "present != null")
                .expect("null inequality should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn evaluates_cell_cast_to_typed_cell_from_source_map() {
        let source_map = foo_source_map();
        let cell = foo_value_cell();
        let locals = vec![LocalVarRendered {
            var_name: "payload".to_owned(),
            value: render_runtime_vm_value(&VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(
                &cell,
            )))),
        }];

        let rendered = evaluate_expression(&locals, Some(&source_map), "payload as Cell<Foo>")
            .expect("cell cast should decode");

        let RenderedValue::CellOf {
            type_name, fields, ..
        } = rendered
        else {
            panic!("expected Cell<Foo>");
        };
        assert_eq!(type_name, "Cell<Foo>");
        assert_eq!(fields[0].0, "decoded");
        let RenderedValue::Struct {
            type_name,
            fields: decoded_fields,
        } = &fields[0].1
        else {
            panic!("expected decoded Foo");
        };
        assert_eq!(type_name, "Foo");
        assert_eq!(decoded_fields[0].0, "value");
        assert_eq!(decoded_fields[0].1.dap_parts().0, "42");
        assert_eq!(decoded_fields[0].1.dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn evaluates_slice_cast_to_typed_cell_from_source_map() {
        let source_map = foo_source_map();
        let cell = foo_value_cell();
        let locals = vec![LocalVarRendered {
            var_name: "payload".to_owned(),
            value: render_runtime_vm_value(&VmStackValue::CellSlice(CellSlice {
                value: Boc::encode_hex(&cell),
                bits: None,
                refs: None,
            })),
        }];

        let rendered = evaluate_expression(&locals, Some(&source_map), "payload as Cell<Foo>")
            .expect("slice cast should decode");

        let RenderedValue::CellOf { fields, .. } = rendered else {
            panic!("expected Cell<Foo>");
        };
        let RenderedValue::Struct {
            fields: decoded_fields,
            ..
        } = &fields[0].1
        else {
            panic!("expected decoded Foo");
        };
        assert_eq!(decoded_fields[0].0, "value");
        assert_eq!(decoded_fields[0].1.dap_parts().0, "42");
        assert_eq!(decoded_fields[0].1.dap_parts().1.as_deref(), Some("uint32"));
    }

    #[test]
    fn rendered_value_text_uses_raw_values_for_compact_cell_like_values() {
        let cell = foo_value_cell();
        let rendered =
            render_runtime_vm_value(&VmStackValue::Cell(CellLike::Cell(Boc::encode_hex(&cell))));

        assert_ne!(
            rendered.to_string(),
            format!("cell{{{}}}", Boc::encode_hex(&cell))
        );
        assert_eq!(
            rendered_value_text(&rendered),
            format!("cell{{{}}}", Boc::encode_hex(&cell))
        );

        let slice = RenderedValue::CellLike {
            type_name: "slice".to_owned(),
            value: "16 bits, 0 refs, hash: 0xdeadbeef...".to_owned(),
            fields: vec![("raw".to_owned(), RenderedValue::leaf("slice{abcd}"))],
            raw: None,
        };
        assert_eq!(rendered_value_text(&slice), "slice{abcd}");

        let builder = RenderedValue::CellLike {
            type_name: "builder".to_owned(),
            value: "16 bits, 1 refs, hash: 0xdeadbeef...".to_owned(),
            fields: vec![("raw".to_owned(), RenderedValue::leaf("builder{7} + 1 refs"))],
            raw: Some(CellLike::Builder("b5ee".to_owned())),
        };
        assert_eq!(rendered_value_text(&builder), "builder{7} + 1 refs");
    }
}
