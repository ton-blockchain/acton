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
pub(crate) enum StmtAst {
    Comment(String),
    VarDecl {
        name: String,
        expr: String,
    },
    Assign {
        target: String,
        expr: String,
    },
    Return(Option<String>),
    If {
        negated: bool,
        condition: String,
        then_body: Vec<StmtAst>,
        else_body: Option<Vec<StmtAst>>,
    },
    Repeat {
        count: String,
        body: Vec<StmtAst>,
    },
    DoUntil {
        body: Vec<StmtAst>,
        condition: String,
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
        Self::VarDecl {
            name: name.into(),
            expr: expr.into(),
        }
    }

    #[must_use]
    pub(crate) fn assign(target: impl Into<String>, expr: impl Into<String>) -> Self {
        Self::Assign {
            target: target.into(),
            expr: expr.into(),
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
        StmtAst::VarDecl { name, expr } => {
            let _ = writeln!(out, "{indent}var {name} = {expr};");
        }
        StmtAst::Assign { target, expr } => {
            let _ = writeln!(out, "{indent}{target} = {expr};");
        }
        StmtAst::Return(Some(expr)) => {
            let _ = writeln!(out, "{indent}return {expr};");
        }
        StmtAst::Return(None) => {
            let _ = writeln!(out, "{indent}return ();");
        }
        StmtAst::If {
            negated,
            condition,
            then_body,
            else_body,
        } => {
            let keyword = if *negated { "ifnot" } else { "if" };
            let _ = writeln!(out, "{indent}{keyword} ({condition}) {{");
            render_stmt_list(then_body, depth + 1, out);
            if let Some(else_body) = else_body {
                let _ = writeln!(out, "{indent}}} else {{");
                render_stmt_list(else_body, depth + 1, out);
            }
            let _ = writeln!(out, "{indent}}}");
        }
        StmtAst::Repeat { count, body } => {
            let _ = writeln!(out, "{indent}repeat ({count}) {{");
            render_stmt_list(body, depth + 1, out);
            let _ = writeln!(out, "{indent}}}");
        }
        StmtAst::DoUntil { body, condition } => {
            let _ = writeln!(out, "{indent}do {{");
            render_stmt_list(body, depth + 1, out);
            let _ = writeln!(out, "{indent}}} until ({condition});");
        }
        StmtAst::Expr(line) => {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MethodAst, MethodSignatureAst, ParamAst, StmtAst, render_method_ast};

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
                    condition: "arg0".to_string(),
                    then_body: vec![StmtAst::assign("v0", "1")],
                    else_body: Some(vec![StmtAst::assign("v0", "2")]),
                },
                StmtAst::DoUntil {
                    body: vec![StmtAst::assign("v0", "v0 + 1")],
                    condition: "v0 > 10".to_string(),
                },
                StmtAst::Return(Some("v0".to_string())),
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
