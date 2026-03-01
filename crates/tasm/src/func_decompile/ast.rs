use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParamAst {
    pub ty: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodSignatureAst {
    pub return_type: String,
    pub name: String,
    pub params: Vec<ParamAst>,
    pub qualifiers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExprAst {
    Atom(String),
    NullLiteral,
    Unary {
        op: String,
        expr: Box<ExprAst>,
    },
    Binary {
        lhs: Box<ExprAst>,
        op: String,
        rhs: Box<ExprAst>,
        wrap_lhs: bool,
        wrap_rhs: bool,
    },
    Ternary {
        condition: Box<ExprAst>,
        then_expr: Box<ExprAst>,
        else_expr: Box<ExprAst>,
    },
    Tuple(Vec<ExprAst>),
    Call {
        callee: String,
        args: Vec<ExprAst>,
    },
}

impl From<String> for ExprAst {
    fn from(value: String) -> Self {
        if value == "null()" {
            Self::NullLiteral
        } else {
            Self::Atom(value)
        }
    }
}

impl From<&str> for ExprAst {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Var {
    Name(String),
    Tensor(Vec<Var>),
}

impl Var {
    #[must_use]
    pub(crate) fn name(name: impl Into<String>) -> Self {
        Self::Name(name.into())
    }

    #[must_use]
    pub(crate) fn tensor(items: Vec<Var>) -> Self {
        Self::Tensor(items)
    }
}

impl From<String> for Var {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<&str> for Var {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StmtAst {
    Comment(String),
    VarDecl {
        binding: Var,
        expr: ExprAst,
    },
    Assign {
        target: String,
        expr: ExprAst,
    },
    Return(Option<ExprAst>),
    Call {
        callee: String,
        args: Vec<ExprAst>,
    },
    If {
        negated: bool,
        condition: ExprAst,
        then_body: Vec<StmtAst>,
        else_body: Option<Vec<StmtAst>>,
    },
    Repeat {
        count: ExprAst,
        body: Vec<StmtAst>,
    },
    DoUntil {
        body: Vec<StmtAst>,
        condition: ExprAst,
    },
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodAst {
    pub signature: MethodSignatureAst,
    pub leading_comments: Vec<String>,
    pub body: Vec<StmtAst>,
}

impl MethodSignatureAst {
    #[must_use]
    pub(crate) fn render(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|p| format!("{} {}", p.ty, p.name))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = format!("{} {}({})", self.return_type, self.name, params);
        if !self.qualifiers.is_empty() {
            out.push(' ');
            out.push_str(&self.qualifiers.join(" "));
        }
        out.push_str(" {");
        out
    }
}

impl StmtAst {
    #[must_use]
    pub(crate) fn expr(line: impl Into<String>) -> Self {
        Self::Expr(line.into())
    }

    #[must_use]
    pub(crate) fn comment(line: impl Into<String>) -> Self {
        Self::Comment(line.into())
    }

    #[must_use]
    pub(crate) fn var(name: impl Into<String>, expr: impl Into<String>) -> Self {
        let expr = expr.into();
        Self::VarDecl {
            binding: Var::Name(name.into()),
            expr: if expr == "null()" {
                ExprAst::NullLiteral
            } else {
                ExprAst::Atom(expr)
            },
        }
    }

    #[must_use]
    pub(crate) fn assign(target: impl Into<String>, expr: impl Into<String>) -> Self {
        let expr = expr.into();
        Self::Assign {
            target: target.into(),
            expr: if expr == "null()" {
                ExprAst::NullLiteral
            } else {
                ExprAst::Atom(expr)
            },
        }
    }
}

pub(crate) fn render_method_ast(ast: &MethodAst, out: &mut String) {
    let _ = writeln!(out, "{}", ast.signature.render());
    for comment in &ast.leading_comments {
        let _ = writeln!(out, "    {}", comment.trim());
    }
    render_stmt_list(&ast.body, 1, out);
    out.push_str("}\n\n");
}

pub(crate) fn render_expr_ast(expr: &ExprAst) -> String {
    render_expr(expr)
}

fn render_stmt_list(stmts: &[StmtAst], depth: usize, out: &mut String) {
    for stmt in stmts {
        render_stmt(stmt, depth, out);
    }
}

fn render_stmt(stmt: &StmtAst, depth: usize, out: &mut String) {
    let indent = "    ".repeat(depth);
    match stmt {
        StmtAst::Comment(line) => {
            let _ = writeln!(out, "{indent}{line}");
        }
        StmtAst::VarDecl { binding, expr } => {
            let _ = writeln!(
                out,
                "{indent}var {} = {};",
                render_tensor_expr(binding),
                render_expr(expr)
            );
        }
        StmtAst::Assign { target, expr } => {
            let _ = writeln!(out, "{indent}{target} = {};", render_expr(expr));
        }
        StmtAst::Return(Some(expr)) => {
            let _ = writeln!(out, "{indent}return {};", render_expr(expr));
        }
        StmtAst::Return(None) => {
            let _ = writeln!(out, "{indent}return ();");
        }
        StmtAst::Call { callee, args } => {
            let rendered_args = args.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            let _ = writeln!(out, "{indent}{callee}({rendered_args});");
        }
        StmtAst::If {
            negated,
            condition,
            then_body,
            else_body,
        } => {
            let keyword = if *negated { "ifnot" } else { "if" };
            let _ = writeln!(out, "{indent}{keyword} ({}) {{", render_expr(condition));
            render_stmt_list(then_body, depth + 1, out);
            if let Some(else_body) = else_body {
                let _ = writeln!(out, "{indent}}} else {{");
                render_stmt_list(else_body, depth + 1, out);
            }
            let _ = writeln!(out, "{indent}}}");
        }
        StmtAst::Repeat { count, body } => {
            let _ = writeln!(out, "{indent}repeat ({}) {{", render_expr(count));
            render_stmt_list(body, depth + 1, out);
            let _ = writeln!(out, "{indent}}}");
        }
        StmtAst::DoUntil { body, condition } => {
            let _ = writeln!(out, "{indent}do {{");
            render_stmt_list(body, depth + 1, out);
            let _ = writeln!(out, "{indent}}} until ({});", render_expr(condition));
        }
        StmtAst::Expr(line) => {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}

fn render_expr(expr: &ExprAst) -> String {
    match expr {
        ExprAst::Atom(s) => s.clone(),
        ExprAst::NullLiteral => "null()".to_string(),
        ExprAst::Unary { op, expr } => {
            format!("{op}({})", render_expr(expr))
        }
        ExprAst::Binary {
            lhs,
            op,
            rhs,
            wrap_lhs,
            wrap_rhs,
        } => {
            let lhs = render_expr(lhs);
            let rhs = render_expr(rhs);
            let lhs = if *wrap_lhs { format!("({lhs})") } else { lhs };
            let rhs = if *wrap_rhs { format!("({rhs})") } else { rhs };
            format!("{lhs} {op} {rhs}")
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            format!(
                "({}) ? ({}) : ({})",
                render_expr(condition),
                render_expr(then_expr),
                render_expr(else_expr)
            )
        }
        ExprAst::Tuple(items) => {
            let rendered = items.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("({rendered})")
        }
        ExprAst::Call { callee, args } => {
            let rendered_args = args.iter().map(render_expr).collect::<Vec<_>>().join(", ");
            format!("{callee}({rendered_args})")
        }
    }
}

fn render_tensor_expr(tensor: &Var) -> String {
    match tensor {
        Var::Name(name) => name.clone(),
        Var::Tensor(items) => {
            let rendered = items
                .iter()
                .map(render_tensor_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({rendered})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExprAst, MethodAst, MethodSignatureAst, ParamAst, StmtAst, render_method_ast};

    #[test]
    fn renders_structured_ast() {
        let ast = MethodAst {
            signature: MethodSignatureAst {
                return_type: "int".to_string(),
                name: "method_1".to_string(),
                params: vec![ParamAst {
                    ty: "int".to_string(),
                    name: "arg0".to_string(),
                }],
                qualifiers: vec!["impure".to_string(), "method_id(1)".to_string()],
            },
            leading_comments: vec![";; dict_method_id: 1".to_string()],
            body: vec![
                StmtAst::var("v0", "0"),
                StmtAst::If {
                    negated: false,
                    condition: ExprAst::Atom("arg0".to_string()),
                    then_body: vec![StmtAst::assign("v0", "1")],
                    else_body: Some(vec![StmtAst::assign("v0", "2")]),
                },
                StmtAst::DoUntil {
                    body: vec![StmtAst::assign("v0", "v0 + 1")],
                    condition: ExprAst::Atom("v0 > 10".to_string()),
                },
                StmtAst::Return(Some(ExprAst::Atom("v0".to_string()))),
            ],
        };

        let mut out = String::new();
        render_method_ast(&ast, &mut out);

        assert!(out.contains("int method_1(int arg0) impure method_id(1) {"));
        assert!(out.contains("if (arg0) {"));
        assert!(out.contains("} else {"));
        assert!(out.contains("do {"));
        assert!(out.contains("} until (v0 > 10);"));
        assert!(out.contains("return v0;"));
    }
}
