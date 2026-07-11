use super::TlbParsedDocument;
use crate::{DocumentSnapshot, InlayHint, InlayHintKind, Position, Range, TextEdit};
use tlb_syntax::{AstNode, Declaration, Field, FieldKind, TopLevel};
use tree_sitter::Node;

const SHORT_TAG_MASK: u64 = (1_u64 << 59) - 1;
const TAG_MASK: u64 = (1_u64 << 63) - 1;
const HEX: &[u8; 16] = b"0123456789abcdef";

pub(super) fn inlay_hints(
    document: &DocumentSnapshot,
    parsed: &TlbParsedDocument,
    requested_range: Range,
) -> Vec<InlayHint> {
    parsed
        .source_file
        .top_levels()
        .filter_map(|top_level| {
            let TopLevel::Declaration(declaration) = top_level else {
                return None;
            };
            constructor_hint(document, declaration, requested_range)
        })
        .collect()
}

fn constructor_hint(
    document: &DocumentSnapshot,
    declaration: Declaration<'_>,
    requested_range: Range,
) -> Option<InlayHint> {
    let constructor = declaration.constructor()?;
    if constructor.tag().is_some() {
        return None;
    }
    let name = constructor.name()?;
    let position = document
        .text_index()
        .range_of_node(document.text(), name.syntax())
        .end;
    if !contains_position(requested_range, position) {
        return None;
    }

    let equation = print_constructor(document, declaration, true, false)?;
    let tag = ConstructorTag::from_crc32(crc32(equation.as_bytes())).to_string();
    let edit_range = Range::new(position, position);

    let mut hint = InlayHint::new(position, &tag, InlayHintKind::Type)
        .with_text_edit(TextEdit::new(edit_range, &tag));
    hint.padding_right = true;
    Some(hint)
}

fn contains_position(range: Range, position: Position) -> bool {
    range.start <= position && position <= range.end
}

fn print_constructor(
    document: &DocumentSnapshot,
    declaration: Declaration<'_>,
    skip_tag: bool,
    show_braces: bool,
) -> Option<String> {
    let constructor = declaration.constructor()?;
    let combinator = declaration.combinator()?;
    let mut result = constructor
        .name()
        .map(|name| document.text_of(name).to_owned())
        .unwrap_or_default();

    if !skip_tag && let Some(tag) = constructor.tag() {
        result.push_str(document.text_of(tag.syntax()));
    }

    let fields = declaration
        .fields()
        .map(|field| print_field(document, field, show_braces))
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    if !fields.is_empty() {
        result.push(' ');
        result.push_str(&fields.join(" "));
    }

    result.push_str(" = ");
    result.push_str(
        combinator
            .name()
            .map(|name| document.text_of(name))
            .unwrap_or_default(),
    );

    let parameters = combinator
        .params()
        .map(|parameter| {
            print_type_expression(document, parameter.syntax(), 100, !show_braces, false)
        })
        .filter(|parameter| !parameter.is_empty())
        .collect::<Vec<_>>();
    if !parameters.is_empty() {
        result.push(' ');
        result.push_str(&parameters.join(" "));
    }

    Some(result)
}

fn print_field(document: &DocumentSnapshot, field: Field<'_>, show_braces: bool) -> String {
    let Some(value) = field.value() else {
        return document.text_of(field).to_owned();
    };

    match value {
        FieldKind::FieldBuiltin(field) => {
            let (Some(name), Some(value)) = (field.name(), field.field()) else {
                return document.text_of(field).to_owned();
            };
            let inner = format!("{}:{}", document.text_of(name), document.text_of(value));
            if show_braces {
                format!("{{{inner}}}")
            } else {
                inner
            }
        }
        FieldKind::FieldCurlyExpr(field) => field.expr().map_or_else(
            || document.text_of(field).to_owned(),
            |expr| print_type_expression(document, expr.syntax(), 0, !show_braces, true),
        ),
        FieldKind::FieldNamed(field) => {
            let (Some(name), Some(expr)) = (field.name(), field.expr()) else {
                return document.text_of(field).to_owned();
            };
            format!(
                "{}:{}",
                document.text_of(name),
                print_type_expression(document, expr.syntax(), 0, !show_braces, false)
            )
        }
        FieldKind::FieldExpr(field) => field.expr().map_or_else(
            || document.text_of(field).to_owned(),
            |expr| print_type_expression(document, expr.syntax(), 0, show_braces, false),
        ),
        FieldKind::FieldAnonymous(_) | FieldKind::Unmapped(_) => {
            document.text_of(field).trim().to_owned()
        }
    }
}

fn print_type_expression(
    document: &DocumentSnapshot,
    node: Node<'_>,
    priority: u8,
    skip_parens: bool,
    normalize_binary: bool,
) -> String {
    if priority > 0 && skip_parens {
        return print_type_expression(document, node, 0, true, normalize_binary);
    }

    match node.kind() {
        "field" => print_field(document, Field(node), !skip_parens),
        "type_expr"
        | "simple_expr"
        | "ref_expr"
        | "ref_inner"
        | "type_parameter"
        | "cond_type_expr"
        | "builtin_expr"
        | "parens_expr"
        | "parens_type_expr"
        | "parens_compare_expr"
        | "compare_expr"
        | "cond_expr" => node.named_child(0).map_or_else(String::new, |child| {
            print_type_expression(document, child, priority, skip_parens, normalize_binary)
        }),
        "binary_expression" => {
            print_binary_expression(document, node, priority, skip_parens, normalize_binary)
        }
        "array_type" => field_node(node, "element_type").map_or_else(String::new, |element| {
            format!(
                "[{}]",
                print_type_expression(document, element, priority, skip_parens, normalize_binary)
            )
        }),
        "cell_ref_expr" => field_node(node, "expr").map_or_else(String::new, |inner| {
            format!(
                "^{}",
                print_type_expression(document, inner, 100, skip_parens, normalize_binary)
            )
        }),
        "cell_ref_inner" => node.named_child(0).map_or_else(String::new, |child| {
            print_type_expression(document, child, priority, skip_parens, normalize_binary)
        }),
        "combinator_expr" => {
            print_combinator_expression(document, node, priority, skip_parens, normalize_binary)
        }
        "cond_question_expr" => {
            let (Some(left), Some(right)) = (node.named_child(0), node.named_child(1)) else {
                return String::new();
            };
            let body = format!(
                "{} ? {}",
                print_type_expression(document, left, 96, skip_parens, normalize_binary),
                print_type_expression(document, right, 96, skip_parens, normalize_binary)
            );
            parenthesize(body, priority > 95)
        }
        "cond_dot_and_question_expr" => {
            let (Some(left), Some(right)) = (node.named_child(0), node.named_child(1)) else {
                return String::new();
            };
            format!(
                "{} ? {}",
                print_type_expression(document, left, priority, skip_parens, normalize_binary),
                print_type_expression(document, right, priority, skip_parens, normalize_binary)
            )
        }
        "cond_dotted" => {
            let (Some(expr), Some(number)) = (node.named_child(0), node.named_child(1)) else {
                return String::new();
            };
            let body = format!(
                "{}.{}",
                print_type_expression(document, expr, 98, skip_parens, normalize_binary),
                document.text_of(number)
            );
            parenthesize(body, priority > 97)
        }
        "builtin_zero_args" => "#".to_owned(),
        "builtin_one_arg" => {
            let (Some(operator), Some(expr)) =
                (field_node(node, "operator"), field_node(node, "expr"))
            else {
                return String::new();
            };
            let body = format!(
                "{} {}",
                document.text_of(operator),
                print_type_expression(document, expr, priority, skip_parens, normalize_binary)
            );
            parenthesize(body, !skip_parens)
        }
        "negate_expr" => field_node(node, "operand").map_or_else(String::new, |operand| {
            format!(
                "~{}",
                print_type_expression(document, operand, priority, skip_parens, normalize_binary)
            )
        }),
        "bit_size_expr" => field_node(node, "size").map_or_else(String::new, |size| {
            format!(
                "## {}",
                print_type_expression(document, size, priority, skip_parens, normalize_binary)
            )
        }),
        "array_multiplier" => {
            let (Some(size), Some(array)) = (field_node(node, "size"), field_node(node, "type"))
            else {
                return String::new();
            };
            format!(
                "{} * {}",
                print_type_expression(document, size, priority, skip_parens, normalize_binary),
                print_type_expression(document, array, priority, skip_parens, normalize_binary)
            )
        }
        "curly_expression" => {
            let inner = node.named_child(0).map_or_else(String::new, |child| {
                print_type_expression(document, child, priority, skip_parens, normalize_binary)
            });
            if skip_parens {
                inner
            } else {
                format!("{{{inner}}}")
            }
        }
        _ => document.text_of(node).trim().to_owned(),
    }
}

fn print_binary_expression(
    document: &DocumentSnapshot,
    node: Node<'_>,
    priority: u8,
    skip_parens: bool,
    normalize_binary: bool,
) -> String {
    let (Some(mut left_node), Some(mut right_node), Some(operator)) = (
        field_node(node, "left"),
        field_node(node, "right"),
        field_node(node, "operator"),
    ) else {
        return String::new();
    };
    let operator = match document.text_of(operator) {
        ">=" => "<=",
        ">" => "<",
        other => other,
    };

    if !normalize_binary && expression_is_number(right_node) {
        std::mem::swap(&mut left_node, &mut right_node);
    }

    let result_priority = match operator {
        "+" => 21,
        "*" => 30,
        _ => 0,
    };
    let left = print_type_expression(
        document,
        right_node,
        result_priority + 1,
        skip_parens,
        normalize_binary,
    );
    let right = print_type_expression(
        document,
        left_node,
        result_priority,
        skip_parens,
        normalize_binary,
    );
    let body = if normalize_binary {
        format!("{operator} {left} {right}")
    } else {
        format!("{right} {operator} {left}")
    };

    parenthesize(body, result_priority != 0 && priority > result_priority)
}

fn expression_is_number(mut node: Node<'_>) -> bool {
    loop {
        if node.kind() == "number" {
            return true;
        }
        let Some(child) = node.named_child(0) else {
            return false;
        };
        node = child;
    }
}

fn print_combinator_expression(
    document: &DocumentSnapshot,
    node: Node<'_>,
    priority: u8,
    skip_parens: bool,
    normalize_binary: bool,
) -> String {
    let Some(name) = field_node(node, "name") else {
        return String::new();
    };
    let mut cursor = node.walk();
    let parameters = node
        .children_by_field_name("params", &mut cursor)
        .map(|parameter| {
            print_type_expression(document, parameter, 91, skip_parens, normalize_binary)
        })
        .collect::<Vec<_>>();
    let show_parens = priority > 90 && !parameters.is_empty();
    let body = std::iter::once(document.text_of(name).to_owned())
        .chain(parameters)
        .collect::<Vec<_>>()
        .join(" ");
    parenthesize(body, show_parens)
}

fn field_node<'tree>(node: Node<'tree>, name: &str) -> Option<Node<'tree>> {
    node.child_by_field_name(name)
}

fn parenthesize(value: String, condition: bool) -> String {
    if condition {
        format!("({value})")
    } else {
        value
    }
}

#[derive(Clone, Copy)]
struct ConstructorTag(u64);

impl ConstructorTag {
    const fn from_crc32(hash: u32) -> Self {
        Self(((hash as u64) << 32) | 0x8000_0000)
    }
}

impl std::fmt::Display for ConstructorTag {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            return formatter.write_str("$_");
        }

        let mut tag = self.0;
        if SHORT_TAG_MASK & tag == 0 {
            formatter.write_str("$")?;
            let mut count = 0;
            while tag & TAG_MASK != 0 {
                let bit = (tag >> 63) & 1;
                write!(formatter, "{bit}")?;
                tag = tag.wrapping_shl(1);
                count += 1;
            }
            if count == 0 {
                formatter.write_str("_")?;
            }
            return Ok(());
        }

        formatter.write_str("#")?;
        while tag & TAG_MASK != 0 {
            let digit = usize::try_from((tag >> 60) & 15).unwrap_or_default();
            write!(formatter, "{}", char::from(HEX[digit]))?;
            tag = tag.wrapping_shl(4);
        }
        if tag == 0 {
            formatter.write_str("_")?;
        }
        Ok(())
    }
}

const fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    let mut index = 0;
    while index < bytes.len() {
        crc ^= bytes[index] as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        index += 1;
    }
    !crc
}
