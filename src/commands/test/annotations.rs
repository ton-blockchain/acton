use crate::commands::test::{FuzzConfig, TestAnnotation};
use tolk_syntax::{
    Annotation, AnnotationArgs, AstNode, Expr, GetMethod, HasAnnotations, HasName, ObjectLit,
};

#[derive(Debug, Default)]
pub(super) struct TestAnnotations {
    pub annotations: Vec<TestAnnotation>,
    pub fuzz: Option<FuzzConfig>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub status_description: Option<String>,
}

impl TestAnnotations {
    fn merge_from(&mut self, other: Self) {
        self.annotations.extend(other.annotations);
        if other.fuzz.is_some() {
            self.fuzz = other.fuzz;
        }
        if other.expected_exit_code.is_some() {
            self.expected_exit_code = other.expected_exit_code;
        }
        if other.gas_limit.is_some() {
            self.gas_limit = other.gas_limit;
        }
        if other.status_description.is_some() {
            self.status_description = other.status_description;
        }
    }
}

pub(super) fn find_test_annotations(content: &str, child: GetMethod<'_>) -> TestAnnotations {
    let Some(annotations_node) = child.annotations() else {
        // fast path
        return TestAnnotations::default();
    };

    let mut parsed = TestAnnotations::default();

    for annotation in annotations_node.annotations() {
        let Some(name) = annotation.name() else {
            continue;
        };

        let annotation_name = name.text(content);
        let annotation_values = if annotation_name.starts_with("test.") {
            parse_dotted_test_annotation(content, annotation_name, annotation)
        } else {
            continue;
        };

        parsed.merge_from(annotation_values);
    }

    parsed
}

fn parse_dotted_test_annotation(
    content: &str,
    annotation_name: &str,
    annotation: Annotation<'_>,
) -> TestAnnotations {
    let Some(kind) = annotation_name.strip_prefix("test.") else {
        return TestAnnotations::default();
    };

    match kind {
        "skip" => parse_dotted_skip(content, annotation.args()),
        "todo" => parse_dotted_todo(content, annotation.args()),
        "fail_with" => parse_dotted_fail_with(content, annotation.args()),
        "gas_limit" => parse_dotted_gas_limit(content, annotation.args()),
        "fuzz" => parse_dotted_fuzz(content, annotation.args()),
        _ => TestAnnotations::default(),
    }
}

fn first_annotation_arg(args: Option<AnnotationArgs<'_>>) -> Option<Expr<'_>> {
    args.and_then(|args| args.args().next())
}

fn parse_dotted_skip(content: &str, args: Option<AnnotationArgs<'_>>) -> TestAnnotations {
    let mut parsed = TestAnnotations::default();

    match first_annotation_arg(args) {
        None => parsed.annotations.push(TestAnnotation::Skip),
        Some(Expr::StringLit(value)) => {
            parsed.annotations.push(TestAnnotation::Skip);
            parsed.status_description = Some(value.content(content).to_string());
        }
        Some(Expr::BoolLit(value)) if value.value() => {
            parsed.annotations.push(TestAnnotation::Skip);
        }
        _ => {}
    }

    parsed
}

fn parse_dotted_todo(content: &str, args: Option<AnnotationArgs<'_>>) -> TestAnnotations {
    let mut parsed = TestAnnotations::default();

    match first_annotation_arg(args) {
        None => parsed.annotations.push(TestAnnotation::Todo),
        Some(Expr::StringLit(value)) => {
            parsed.annotations.push(TestAnnotation::Todo);
            parsed.status_description = Some(value.content(content).to_string());
        }
        Some(Expr::BoolLit(value)) if value.value() => {
            parsed.annotations.push(TestAnnotation::Todo);
            parsed.status_description = Some("TODO".to_string());
        }
        _ => {}
    }

    parsed
}

fn parse_dotted_fail_with(content: &str, args: Option<AnnotationArgs<'_>>) -> TestAnnotations {
    let mut parsed = TestAnnotations::default();

    if let Some(Expr::NumberLit(value)) = first_annotation_arg(args)
        && let Some(code) = value.parse_i32(content)
    {
        parsed.expected_exit_code = Some(code);
    }

    parsed
}

fn parse_dotted_gas_limit(content: &str, args: Option<AnnotationArgs<'_>>) -> TestAnnotations {
    let mut parsed = TestAnnotations::default();

    if let Some(Expr::NumberLit(value)) = first_annotation_arg(args)
        && let Some(limit) = value.parse_u64(content)
    {
        parsed.gas_limit = Some(limit);
    }

    parsed
}

fn parse_dotted_fuzz(content: &str, args: Option<AnnotationArgs<'_>>) -> TestAnnotations {
    let fuzz = first_annotation_arg(args).map_or_else(
        || Some(FuzzConfig::default()),
        |value| parse_fuzz_value(content, Some(value)),
    );

    TestAnnotations {
        fuzz,
        ..TestAnnotations::default()
    }
}

fn parse_fuzz_value(content: &str, value: Option<Expr<'_>>) -> Option<FuzzConfig> {
    match value? {
        Expr::BoolLit(b) if b.value() => Some(FuzzConfig::default()),
        Expr::NumberLit(n) => n.parse_u32(content).map(|runs| FuzzConfig {
            runs: Some(runs as usize),
            max_test_rejects: None,
            seed: None,
        }),
        Expr::ObjectLit(object) => parse_fuzz_object(content, object),
        _ => None,
    }
}

fn parse_fuzz_object(content: &str, object: ObjectLit<'_>) -> Option<FuzzConfig> {
    let mut config = FuzzConfig::default();
    let mut found_field = false;

    for key_value in object.arguments() {
        let Some(name_node) = key_value.name() else {
            continue;
        };

        match name_node.text(content) {
            "runs" => {
                if let Some(Expr::NumberLit(n)) = key_value.value()
                    && let Some(runs) = n.parse_u32(content)
                {
                    config.runs = Some(runs as usize);
                    found_field = true;
                }
            }
            "max_test_rejects" => {
                if let Some(Expr::NumberLit(n)) = key_value.value()
                    && let Some(max_test_rejects) = n.parse_u32(content)
                {
                    config.max_test_rejects = Some(max_test_rejects as usize);
                    found_field = true;
                }
            }
            "seed" => {
                if let Some(Expr::NumberLit(n)) = key_value.value()
                    && let Some(seed) = n.parse_u64(content)
                {
                    config.seed = Some(seed);
                    found_field = true;
                }
            }
            _ => {}
        }
    }

    found_field.then_some(config)
}
