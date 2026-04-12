use crate::replayer::LocalVarRendered;
use crate::types_render::RenderedValue;
use anyhow::{Result, anyhow, bail};
use num_bigint::BigInt;
use std::cmp::Ordering;
use tolk_syntax::{
    AstNode, DotAccessField, Expr, FuncBody, FunctionLike, Stmt, TopLevel, parse_tolk_int_literal,
};

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
    expression: &str,
) -> Result<RenderedValue> {
    let source_file = parse_wrapped_source(expression)?;
    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    evaluate_parsed_expression(locals, expr, source_file.source.as_ref())
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
    let value = unwrap_last_seen(value);
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

fn unwrap_last_seen(mut value: &RenderedValue) -> &RenderedValue {
    while let RenderedValue::LastSeen { inner } = value {
        value = inner;
    }
    value
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

#[cfg(test)]
fn parse_value_path(input: &str) -> Result<ParsedValuePath> {
    let source_file = parse_wrapped_source(input)?;
    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    parse_expr_to_path(expr, source_file.source.as_ref())
}

fn wrapped_expression<'tree>(source_file: &'tree tolk_syntax::SourceFile) -> Option<Expr<'tree>> {
    let func = match source_file.top_levels().next()? {
        TopLevel::Func(func) => func,
        _ => return None,
    };
    let body = match func.body()? {
        FuncBody::Block(block) => block,
        _ => return None,
    };
    let stmt = match body.stmts().next()? {
        Stmt::ExprStmt(stmt) => stmt,
        _ => return None,
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
    expr: Expr<'_>,
    source: &str,
) -> Result<RenderedValue> {
    match expr {
        Expr::BoolLit(bool_lit) => Ok(render_bool(bool_lit.value())),
        Expr::NumberLit(number_lit) => render_number_literal(number_lit.text(source)),
        Expr::StringLit(string_lit) => Ok(render_string_literal(string_lit.text(source))),
        Expr::Paren(paren) => {
            let inner = paren
                .inner()
                .ok_or_else(|| anyhow!("expected expression inside parentheses"))?;
            evaluate_parsed_expression(locals, inner, source)
        }
        Expr::Unary(unary) => match unary.operator_name(source) {
            "!" => {
                let argument = unary
                    .argument()
                    .ok_or_else(|| anyhow!("expected expression after `!`"))?;
                let value = evaluate_parsed_expression(locals, argument, source)?;
                Ok(render_bool(!rendered_value_as_bool(&value)?))
            }
            operator => bail!("unary operator `{operator}` is not supported"),
        },
        Expr::Bin(bin) => match bin.operator_name(source) {
            "&&" => {
                let left = bin
                    .left()
                    .ok_or_else(|| anyhow!("expected left operand for `&&`"))?;
                let left = evaluate_boolean_expression(locals, left, source)?;
                if !left {
                    return Ok(render_bool(false));
                }

                let right = bin
                    .right()
                    .ok_or_else(|| anyhow!("expected right operand for `&&`"))?;
                Ok(render_bool(evaluate_boolean_expression(
                    locals, right, source,
                )?))
            }
            "||" => {
                let left = bin
                    .left()
                    .ok_or_else(|| anyhow!("expected left operand for `||`"))?;
                let left = evaluate_boolean_expression(locals, left, source)?;
                if left {
                    return Ok(render_bool(true));
                }

                let right = bin
                    .right()
                    .ok_or_else(|| anyhow!("expected right operand for `||`"))?;
                Ok(render_bool(evaluate_boolean_expression(
                    locals, right, source,
                )?))
            }
            "==" => evaluate_equality_expression(locals, &bin, source, true),
            "!=" => evaluate_equality_expression(locals, &bin, source, false),
            "<" | "<=" | ">" | ">=" => evaluate_ordering_expression(locals, &bin, source),
            operator => bail!("binary operator `{operator}` is not supported"),
        },
        _ => {
            let path = parse_expr_to_path(expr, source)?;
            resolve_locals_path(locals, &path)
        }
    }
}

fn evaluate_boolean_expression(
    locals: &[LocalVarRendered],
    expr: Expr<'_>,
    source: &str,
) -> Result<bool> {
    let value = evaluate_parsed_expression(locals, expr, source)?;
    rendered_value_as_bool(&value)
}

fn rendered_value_as_bool(value: &RenderedValue) -> Result<bool> {
    match unwrap_last_seen(value) {
        RenderedValue::Leaf { value, .. } if value == "true" => Ok(true),
        RenderedValue::Leaf { value, .. } if value == "false" => Ok(false),
        other => bail!("logical operators require boolean operands, got `{other}`"),
    }
}

fn evaluate_equality_expression(
    locals: &[LocalVarRendered],
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

    let left = evaluate_parsed_expression(locals, left, source)?;
    let right = evaluate_parsed_expression(locals, right, source)?;
    let equal = rendered_value_text(&left) == rendered_value_text(&right);
    Ok(render_bool(equal == expected_equal))
}

fn evaluate_ordering_expression(
    locals: &[LocalVarRendered],
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

    let left = evaluate_parsed_expression(locals, left, source)?;
    let right = evaluate_parsed_expression(locals, right, source)?;
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

fn compare_rendered_values_as_numbers(
    left: &RenderedValue,
    right: &RenderedValue,
    operator: &str,
) -> Result<Ordering> {
    let left = parse_rendered_number(left)
        .ok_or_else(|| anyhow!("operator `{operator}` requires numeric operands"))?;
    let right = parse_rendered_number(right)
        .ok_or_else(|| anyhow!("operator `{operator}` requires numeric operands"))?;

    Ok(match (left, right) {
        (ComparableNumber::Integer(left), ComparableNumber::Integer(right)) => left.cmp(&right),
        (left, right) => left
            .as_f64()?
            .partial_cmp(&right.as_f64()?)
            .ok_or_else(|| anyhow!("operator `{operator}` requires finite numeric operands"))?,
    })
}

fn parse_rendered_number(value: &RenderedValue) -> Option<ComparableNumber> {
    let text = rendered_value_text(value);
    if let Ok(value) = text.parse::<BigInt>() {
        return Some(ComparableNumber::Integer(value));
    }

    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(ComparableNumber::Float)
}

fn rendered_value_text(value: &RenderedValue) -> String {
    unwrap_last_seen(value).to_string()
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

fn render_bool(value: bool) -> RenderedValue {
    RenderedValue::typed_leaf(value.to_string(), "bool")
}

enum ComparableNumber {
    Integer(BigInt),
    Float(f64),
}

impl ComparableNumber {
    fn as_f64(&self) -> Result<f64> {
        match self {
            Self::Integer(value) => value
                .to_string()
                .parse::<f64>()
                .map_err(|_| anyhow!("numeric value is out of range for comparison")),
            Self::Float(value) => Ok(*value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParsedValuePath, PathSegment, evaluate_expression, parse_value_path};
    use crate::replayer::LocalVarRendered;
    use crate::types_render::RenderedValue;

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

        let value = evaluate_expression(&locals, "foo.bar.0.baz").expect("path should resolve");
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

        let value = evaluate_expression(&locals, "foo").expect("path should resolve");
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

        let value = evaluate_expression(&locals, "foo.enabled && (!foo.blocked || false)")
            .expect("logical expression should resolve");
        assert_eq!(value.to_string(), "true");
    }

    #[test]
    fn logical_operators_short_circuit() {
        let locals = Vec::new();

        let value = evaluate_expression(&locals, "false && missing.flag")
            .expect("short-circuit should avoid rhs lookup");
        assert_eq!(value.to_string(), "false");

        let value = evaluate_expression(&locals, "true || missing.flag")
            .expect("short-circuit should avoid rhs lookup");
        assert_eq!(value.to_string(), "true");
    }

    #[test]
    fn rejects_non_boolean_logical_operands() {
        let locals = vec![LocalVarRendered {
            var_name: "count".to_owned(),
            value: RenderedValue::typed_leaf("42", "int"),
        }];

        let err = evaluate_expression(&locals, "count && true")
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

        let value = evaluate_expression(&locals, "lhs == rhs").expect("equality should resolve");
        assert_eq!(value.to_string(), "true");

        let value =
            evaluate_expression(&locals, "lhs != other").expect("inequality should resolve");
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
            LocalVarRendered {
                var_name: "decimal".to_owned(),
                value: RenderedValue::typed_leaf("7.5", "float"),
            },
        ];

        assert_eq!(
            evaluate_expression(&locals, "small < big")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, "big >= small")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, "decimal <= decimal")
                .expect("comparison should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, "small < 10")
                .expect("literal comparison should resolve")
                .to_string(),
            "true"
        );
    }

    #[test]
    fn rejects_non_numeric_ordering_operands() {
        let locals = vec![LocalVarRendered {
            var_name: "name".to_owned(),
            value: RenderedValue::typed_leaf("alice", "string"),
        }];

        let err = evaluate_expression(&locals, "name < 10")
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
            evaluate_expression(&locals, "\"alice\"")
                .expect("string literal should resolve")
                .to_string(),
            "\"alice\""
        );
        assert_eq!(
            evaluate_expression(&locals, "name == \"alice\"")
                .expect("string equality should resolve")
                .to_string(),
            "true"
        );
        assert_eq!(
            evaluate_expression(&locals, "name != \"bob\"")
                .expect("string inequality should resolve")
                .to_string(),
            "true"
        );
    }
}
