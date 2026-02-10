use tolk_syntax::{AstNode, Expr, GetMethod, HasAnnotations, HasName, ObjectLit};

#[derive(Debug, Default)]
pub(super) struct TestAnnotations {
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
}

pub(super) fn find_test_annotations(content: &str, child: GetMethod<'_>) -> TestAnnotations {
    let Some(annotations_node) = child.annotations() else {
        // fast path
        return TestAnnotations::default();
    };

    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;

    for annotation in annotations_node.annotations() {
        let Some(name) = annotation.name() else {
            continue;
        };

        if !name.text_matches(content, "test") {
            continue;
        }

        let Some(args) = annotation.args() else {
            continue;
        };

        for arg in args.args() {
            match arg {
                Expr::StringLit(arg) => match arg.content(content) {
                    "skip" => {
                        annotations.push("skip".to_owned());
                    }
                    "todo" => {
                        annotations.push("todo".to_owned());
                    }
                    _ => {}
                },
                Expr::ObjectLit(arg) => {
                    let values = parse_annotation_object(content, arg);

                    annotations.extend(values.annotations);
                    if values.expected_exit_code.is_some() {
                        expected_exit_code = values.expected_exit_code;
                    }
                    if values.gas_limit.is_some() {
                        gas_limit = values.gas_limit;
                    }
                    if values.todo_description.is_some() {
                        todo_description = values.todo_description;
                    }
                }
                _ => {}
            }
        }
    }
    TestAnnotations {
        annotations,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}

fn parse_annotation_object(content: &str, object: ObjectLit<'_>) -> TestAnnotations {
    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;

    for key_value in object.arguments() {
        let Some(name_node) = key_value.name() else {
            continue;
        };

        let field_name = name_node.text(content);

        match field_name {
            "skip" => {
                let is_true = key_value.value().is_none_or(|value| match value {
                    Expr::BoolLit(b) => b.value(),
                    _ => false,
                });

                if is_true {
                    annotations.push("skip".to_owned());
                }
                continue;
            }
            "todo" => {
                if let Some(value) = key_value.value() {
                    match value {
                        Expr::StringLit(s) => {
                            annotations.push("todo".to_owned());
                            todo_description = Some(s.content(content).to_string());
                        }
                        Expr::BoolLit(b) if b.value() => {
                            annotations.push("todo".to_owned());
                            todo_description = Some("TODO".to_string());
                        }
                        _ => {}
                    }
                }
                continue;
            }
            "fail_with" => {
                if let Some(Expr::NumberLit(n)) = key_value.value()
                    && let Ok(code) = n.text(content).parse::<i32>()
                {
                    expected_exit_code = Some(code);
                }
            }
            "gas_limit" => {
                if let Some(Expr::NumberLit(n)) = key_value.value()
                    && let Ok(limit) = n.text(content).parse::<u64>()
                {
                    gas_limit = Some(limit);
                }
            }
            _ => {}
        }
    }

    TestAnnotations {
        annotations,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}
