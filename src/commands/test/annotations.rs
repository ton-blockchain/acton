use tree_sitter::Node;

#[derive(Debug)]
pub struct TestAnnotations {
    pub annotations: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub gas_limit: Option<u64>,
    pub todo_description: Option<String>,
}

pub fn find_test_annotations(content: &str, child: Node) -> TestAnnotations {
    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;
    let Some(annotations_node) = child.child_by_field_name("annotations") else {
        return TestAnnotations {
            annotations,
            expected_exit_code,
            gas_limit,
            todo_description,
        };
    };

    let mut cursor = annotations_node.walk();
    for annotation in annotations_node.children(&mut cursor) {
        if annotation.kind() != "annotation" {
            continue;
        }

        if let Some(name_node) = annotation.child_by_field_name("name") {
            let annotation_name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
            if annotation_name != "custom" {
                continue;
            }
            let Some(args_node) = annotation.child_by_field_name("arguments") else {
                continue;
            };

            let mut arg_cursor = args_node.walk();

            for child in args_node.children(&mut arg_cursor) {
                match child.kind() {
                    "string_literal" => {
                        let text = child.utf8_text(content.as_bytes()).unwrap_or("");
                        let unquoted = text.trim_matches('"');
                        match unquoted {
                            "skip" => {
                                annotations.push("skip".to_string());
                            }
                            "todo" => {
                                annotations.push("todo".to_string());
                            }
                            _ => {}
                        }
                    }
                    "object_literal" => {
                        let values = parse_annotation_object(content, child);

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
    }
    TestAnnotations {
        annotations,
        expected_exit_code,
        gas_limit,
        todo_description,
    }
}

fn parse_annotation_object(content: &str, object_node: Node) -> TestAnnotations {
    let Some(arguments) = object_node.child_by_field_name("arguments") else {
        return TestAnnotations {
            annotations: Vec::new(),
            expected_exit_code: None,
            gas_limit: None,
            todo_description: None,
        };
    };

    let mut annotations = Vec::new();
    let mut expected_exit_code = None;
    let mut gas_limit = None;
    let mut todo_description = None;

    let mut cursor = arguments.walk();

    for field in arguments.children(&mut cursor) {
        if field.kind() == "instance_argument" {
            let Some(name_node) = field.child_by_field_name("name") else {
                continue;
            };

            let field_name = name_node.utf8_text(content.as_bytes()).unwrap_or("");

            match field_name {
                "skip" => {
                    let is_true = field
                        .child_by_field_name("value")
                        .map(|value| is_boolean_true(content, value))
                        .unwrap_or(true); // @custom({ skip }) -> true

                    if is_true {
                        annotations.push("skip".to_string());
                    }
                    continue;
                }
                "todo" => {
                    if let Some(value_node) = field.child_by_field_name("value") {
                        if let Some(description) = parse_string_literal(content, value_node) {
                            annotations.push("todo".to_string());
                            todo_description = Some(description);
                        } else if value_node.kind() == "boolean_literal"
                            && is_boolean_true(content, value_node)
                        {
                            annotations.push("todo".to_string());
                            todo_description = Some("TODO".to_string());
                        }
                    }
                    continue;
                }
                _ => {}
            }

            if let Some(value_node) = field.child_by_field_name("value") {
                match field_name {
                    "fail_with" => {
                        if let Some(number) = parse_number_literal(content, value_node)
                            && let Ok(code) = number.parse::<i32>()
                        {
                            expected_exit_code = Some(code);
                        }
                    }
                    "gas_limit" => {
                        if let Some(number) = parse_number_literal(content, value_node)
                            && let Ok(limit) = number.parse::<u64>()
                        {
                            gas_limit = Some(limit);
                        }
                    }
                    _ => {}
                }
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

fn is_boolean_true(content: &str, node: Node) -> bool {
    if node.kind() == "boolean_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        text == "true"
    } else {
        false
    }
}

fn parse_number_literal(content: &str, node: Node) -> Option<String> {
    if node.kind() == "number_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        Some(text.to_string())
    } else {
        None
    }
}

fn parse_string_literal(content: &str, node: Node) -> Option<String> {
    if node.kind() == "string_literal" {
        let text = node.utf8_text(content.as_bytes()).unwrap_or("");
        let unquoted = text.trim_matches('"');
        Some(unquoted.to_string())
    } else {
        None
    }
}
