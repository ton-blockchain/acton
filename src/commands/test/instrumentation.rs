use tree_sitter::Node;

pub(crate) fn inject_locations_into_expect_calls(content: &str, file_path: &str) -> String {
    let Ok(tree) = tolk_parser::parser::parse(content) else {
        return "".to_string();
    };
    let root_node = tree.root_node();

    let mut replacements = Vec::new();
    find_expect_calls(&root_node, content, file_path, &mut replacements);

    let mut result = content.to_string();

    if !has_entry_function(&root_node, &result) {
        result += "\n\nfun main() {}"
    }

    for (start, end, replacement) in replacements.into_iter().rev() {
        result.replace_range(start..end, &replacement);
    }

    result
}

fn find_expect_calls(
    node: &Node,
    content: &str,
    file_path: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<()> {
    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else { break };
        find_expect_calls(&child, content, file_path, replacements);
    }

    if node.kind() != "function_call" {
        // fast path
        return None;
    }

    let callee_node = node.child_by_field_name("callee")?;

    if callee_node.kind() == "identifier"
        && callee_node.utf8_text(content.as_bytes()).unwrap_or("") == "expect"
    {
        let args_node = node.child_by_field_name("arguments")?;

        let mut arg_count = 0;
        let mut cursor = args_node.walk();
        for child in args_node.children(&mut cursor) {
            if child.kind() == "call_argument" {
                arg_count += 1;
            }
        }

        // Don't add location if it already passed by the user
        if arg_count == 1 {
            let column = callee_node.start_position().column + 1;
            let start = args_node.end_byte() - 1;
            let end = args_node.end_byte() - 1;

            let lines: Vec<&str> = content[..start].lines().collect();
            let line_number = lines.len();

            let location = format!(", \"{file_path}:{line_number}:{column}\"",);
            replacements.push((start, end, location));
        }
    }

    Some(())
}

fn has_entry_function(root_node: &Node, content: &str) -> bool {
    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.kind() == "function_declaration"
            && let Some(name_node) = child.child_by_field_name("name")
        {
            let name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
            if name == "main" || name == "onInternalMessage" {
                return true;
            }
        }
    }
    false
}
