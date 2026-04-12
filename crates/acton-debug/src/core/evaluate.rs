use crate::replayer::LocalVarRendered;
use crate::types_render::RenderedValue;
use anyhow::{Result, anyhow, bail};
use tolk_syntax::{DotAccessField, Expr, FuncBody, FunctionLike, Stmt, TopLevel};

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
    let path = parse_value_path(expression).map_err(|err| {
        anyhow!(
            "Unsupported expression `{expression}`: {err}. Only simple field/index paths like `foo.bar.0.baz` are supported"
        )
    })?;

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

fn parse_value_path(input: &str) -> Result<ParsedValuePath> {
    let wrapped = format!("fun __acton_debug_eval__() {{ {input}; }}");
    let source_file = tolk_syntax::parse(&wrapped)?;
    if source_file.has_errors() {
        bail!("syntax error");
    }

    let expr = wrapped_expression(&source_file)
        .ok_or_else(|| anyhow!("expected a single expression statement"))?;
    parse_expr_to_path(expr, &wrapped)
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
}
