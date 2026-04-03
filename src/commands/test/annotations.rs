use crate::commands::test::{FuzzConfig, TestAnnotation};
use tolk_syntax::{AstNode, Expr, GetMethod, HasAnnotations, HasName, ObjectLit};

#[derive(Debug, Default)]
pub(super) struct TestAnnotations {
    pub annotations: Vec<TestAnnotation>,
    pub fuzz: Option<FuzzConfig>,
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
    let mut fuzz = None;
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
                        annotations.push(TestAnnotation::Skip);
                    }
                    "todo" => {
                        annotations.push(TestAnnotation::Todo);
                    }
                    _ => {}
                },
                Expr::ObjectLit(arg) => {
                    let values = parse_annotation_object(content, arg);

                    annotations.extend(values.annotations);
                    if values.fuzz.is_some() {
                        fuzz = values.fuzz;
                    }
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
        fuzz,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}

fn parse_annotation_object(content: &str, object: ObjectLit<'_>) -> TestAnnotations {
    let mut annotations = Vec::new();
    let mut fuzz = None;
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
                    annotations.push(TestAnnotation::Skip);
                }
                continue;
            }
            "todo" => {
                if let Some(value) = key_value.value() {
                    match value {
                        Expr::StringLit(s) => {
                            annotations.push(TestAnnotation::Todo);
                            todo_description = Some(s.content(content).to_string());
                        }
                        Expr::BoolLit(b) if b.value() => {
                            annotations.push(TestAnnotation::Todo);
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
            "fuzz" => {
                fuzz = parse_fuzz_value(content, key_value.value());
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
        fuzz,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}

fn parse_fuzz_value(content: &str, value: Option<Expr<'_>>) -> Option<FuzzConfig> {
    match value? {
        Expr::BoolLit(b) if b.value() => Some(FuzzConfig::default()),
        Expr::NumberLit(n) => n
            .text(content)
            .parse::<usize>()
            .ok()
            .map(|runs| FuzzConfig { runs }),
        Expr::ObjectLit(object) => parse_fuzz_object(content, object),
        _ => None,
    }
}

fn parse_fuzz_object(content: &str, object: ObjectLit<'_>) -> Option<FuzzConfig> {
    let mut config = FuzzConfig::default();
    let mut found_runs = false;

    for key_value in object.arguments() {
        let Some(name_node) = key_value.name() else {
            continue;
        };

        if name_node.text(content) != "runs" {
            continue;
        }

        if let Some(Expr::NumberLit(n)) = key_value.value()
            && let Ok(runs) = n.text(content).parse::<usize>()
        {
            config.runs = runs;
            found_runs = true;
        }
    }

    found_runs.then_some(config)
}
