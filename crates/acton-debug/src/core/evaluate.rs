use crate::replayer::LocalVarRendered;
use crate::types_render::RenderedValue;
use anyhow::{Result, anyhow, bail};

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
            "Unsupported expression `{expression}`: {err}. Only simple field/index paths like `foo.bar[0].baz` are supported"
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
    let mut parser = PathParser::new(input);
    let root = parser.parse_identifier()?;
    let mut segments = Vec::new();

    loop {
        parser.skip_whitespace();
        while parser.consume_if('!') {
            parser.skip_whitespace();
        }

        match parser.peek() {
            None => break,
            Some('.') => {
                parser.bump();
                parser.skip_whitespace();
                if parser.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                    segments.push(PathSegment::Index(parser.parse_index()?));
                } else {
                    segments.push(PathSegment::Field(parser.parse_identifier()?));
                }
            }
            Some('[') => {
                parser.bump();
                parser.skip_whitespace();
                let index = parser.parse_index()?;
                parser.skip_whitespace();
                parser.expect(']')?;
                segments.push(PathSegment::Index(index));
            }
            Some(other) => bail!("unexpected character `{other}`"),
        }
    }

    Ok(ParsedValuePath { root, segments })
}

struct PathParser<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> PathParser<'a> {
    const fn new(input: &'a str) -> Self {
        Self { input, offset: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    fn consume_if(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        match self.bump() {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => bail!("expected `{expected}`, got `{actual}`"),
            None => bail!("expected `{expected}`"),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn parse_identifier(&mut self) -> Result<String> {
        self.skip_whitespace();
        match self.peek() {
            Some('`') => self.parse_backticked_identifier(),
            Some(ch) if is_ident_start(ch) => self.parse_plain_identifier(),
            Some(ch) => bail!("expected identifier, got `{ch}`"),
            None => bail!("expected identifier"),
        }
    }

    fn parse_plain_identifier(&mut self) -> Result<String> {
        let mut identifier = String::new();
        let first = self.bump().ok_or_else(|| anyhow!("expected identifier"))?;
        if !is_ident_start(first) {
            bail!("expected identifier, got `{first}`");
        }
        identifier.push(first);

        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                identifier.push(ch);
                self.bump();
            } else {
                break;
            }
        }

        Ok(identifier)
    }

    fn parse_backticked_identifier(&mut self) -> Result<String> {
        self.expect('`')?;
        let start = self.offset;
        while let Some(ch) = self.peek() {
            if ch == '`' {
                let identifier = &self.input[start..self.offset];
                self.bump();
                if identifier.is_empty() {
                    bail!("empty backticked identifier");
                }
                return Ok(identifier.to_owned());
            }
            self.bump();
        }
        bail!("unterminated backticked identifier")
    }

    fn parse_index(&mut self) -> Result<usize> {
        self.skip_whitespace();
        let start = self.offset;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }

        if start == self.offset {
            bail!("expected numeric index");
        }

        self.input[start..self.offset]
            .parse::<usize>()
            .map_err(|err| anyhow!("invalid numeric index: {err}"))
    }
}

const fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

const fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::{ParsedValuePath, PathSegment, evaluate_expression, parse_value_path};
    use crate::replayer::LocalVarRendered;
    use crate::types_render::RenderedValue;

    #[test]
    fn parses_dot_and_bracket_path_segments() {
        let path = parse_value_path(" foo.bar[0].baz.1 ").expect("path should parse");
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
        let path = parse_value_path("`foo bar`!.`child value`[2]!.baz").expect("path should parse");
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

        let value = evaluate_expression(&locals, "foo.bar[0].baz").expect("path should resolve");
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
    fn rejects_non_path_expressions() {
        let err = parse_value_path("foo()").expect_err("call syntax should not parse");
        assert_eq!(err.to_string(), "unexpected character `(`");
    }
}
