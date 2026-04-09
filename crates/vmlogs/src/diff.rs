use super::parser::{VmStack, VmStackValue};
use std::fmt;

#[derive(Debug, Clone)]
pub enum StackDiff {
    Same(usize),
    Removed(usize),
    Added(String),
    Changed { index: usize, value: String },
}

impl fmt::Display for StackDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackDiff::Same(n) => write!(f, "={n}"),
            StackDiff::Removed(n) => write!(f, "-{n}"),
            StackDiff::Added(val) => write!(f, "+{val}"),
            StackDiff::Changed { index, value } => write!(f, "~{index}:{value}"),
        }
    }
}

impl StackDiff {
    fn from_string(s: &str) -> Option<Self> {
        if let Some(n) = s.strip_prefix('=') {
            n.parse().ok().map(StackDiff::Same)
        } else if let Some(n) = s.strip_prefix('-') {
            n.parse().ok().map(StackDiff::Removed)
        } else if let Some(val) = s.strip_prefix('+') {
            Some(StackDiff::Added(val.to_string()))
        } else if let Some(rest) = s.strip_prefix('~') {
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2
                && let Ok(index) = parts[0].parse()
            {
                return Some(StackDiff::Changed {
                    index,
                    value: parts[1].to_string(),
                });
            }
            None
        } else {
            None
        }
    }
}

fn compute_stack_diff(prev: &[VmStackValue], current: &[VmStackValue]) -> Vec<StackDiff> {
    let mut diffs = Vec::new();
    let min_len = prev.len().min(current.len());

    let mut same_count = 0;
    for i in 0..min_len {
        let prev_str = prev[i].to_string();
        let curr_str = current[i].to_string();

        if prev_str == curr_str {
            same_count += 1;
        } else {
            if same_count > 0 {
                diffs.push(StackDiff::Same(same_count));
                same_count = 0;
            }
            diffs.push(StackDiff::Changed {
                index: i,
                value: curr_str,
            });
        }
    }

    if same_count > 0 {
        diffs.push(StackDiff::Same(same_count));
    }

    if current.len() > prev.len() {
        for item in &current[prev.len()..] {
            diffs.push(StackDiff::Added(item.to_string()));
        }
    } else if prev.len() > current.len() {
        diffs.push(StackDiff::Removed(prev.len() - current.len()));
    }

    diffs
}

fn apply_stack_diff(prev: &[String], diffs: &[StackDiff]) -> Vec<String> {
    let mut result = Vec::new();
    let mut prev_index = 0;

    for diff in diffs {
        match diff {
            StackDiff::Same(n) => {
                for _ in 0..*n {
                    if prev_index < prev.len() {
                        result.push(prev[prev_index].clone());
                        prev_index += 1;
                    }
                }
            }
            StackDiff::Removed(n) => {
                prev_index += n;
            }
            StackDiff::Added(val) => {
                result.push(val.clone());
            }
            StackDiff::Changed { value, .. } => {
                result.push(value.clone());
                prev_index += 1;
            }
        }
    }

    result
}

#[must_use]
pub fn convert_to_diff_logs(input: &str) -> String {
    let mut output = String::new();
    let mut prev_stack: Option<Vec<VmStackValue>> = None;

    for line in input.lines() {
        if let Some(stack_content) = line.strip_prefix("stack: ") {
            let stack = VmStack::new(stack_content.trim());
            let current_stack = stack.parsed();

            if let Some(prev) = &prev_stack {
                let diffs = compute_stack_diff(prev, &current_stack);
                if diffs.is_empty() {
                    output.push_str("rel stack: []\n");
                } else {
                    output.push_str("rel stack: [ ");
                    output.push_str(
                        &diffs
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                    output.push_str(" ]\n");
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }

            prev_stack = Some(current_stack);
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    if output.ends_with('\n') {
        output.pop();
    }

    output
}

#[must_use]
pub fn convert_from_diff_logs(input: &str) -> String {
    let mut output = String::new();
    let mut current_stack: Vec<String> = Vec::new();

    for line in input.lines() {
        if let Some(rel_stack_content) = line.strip_prefix("rel stack: ") {
            let content = rel_stack_content.trim();
            let diffs = parse_diff_line(content);
            current_stack = apply_stack_diff(&current_stack, &diffs);

            output.push_str("stack: [ ");
            output.push_str(&current_stack.join(" "));
            output.push_str(" ]\n");
        } else {
            output.push_str(line);
            output.push('\n');

            if line.starts_with("stack: ")
                && let Some(stack_content) = line.strip_prefix("stack: ")
            {
                let content = stack_content.trim();
                let stack = VmStack::new(content);
                current_stack = stack.parsed().iter().map(ToString::to_string).collect();
            }
        }
    }

    if output.ends_with('\n') {
        output.pop();
    }

    output
}

fn parse_diff_line(content: &str) -> Vec<StackDiff> {
    let content = content.trim_start_matches('[').trim_end_matches(']').trim();
    if content.is_empty() {
        return Vec::new();
    }

    let mut diffs = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in content.chars() {
        if ch == '[' || ch == '(' {
            depth += 1;
            current.push(ch);
        } else if ch == ']' || ch == ')' {
            depth -= 1;
            current.push(ch);
        } else if ch == ' ' && depth == 0 {
            if !current.is_empty() {
                if let Some(diff) = StackDiff::from_string(&current) {
                    diffs.push(diff);
                }
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty()
        && let Some(diff) = StackDiff::from_string(&current)
    {
        diffs.push(diff);
    }

    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_compute_stack_diff_all_same() {
        let input = "stack: [ 0 ]\nstack: [ 0 ]";
        let output = convert_to_diff_logs(input);
        assert_eq!(output, "stack: [ 0 ]\nrel stack: [ =1 ]");
    }

    #[test]
    fn test_compute_stack_diff_add() {
        let input = "stack: [ 0 ]\nstack: [ 0 1 ]";
        let output = convert_to_diff_logs(input);
        assert_eq!(output, "stack: [ 0 ]\nrel stack: [ =1 +1 ]");
    }

    #[test]
    fn test_compute_stack_diff_remove() {
        let input = "stack: [ 0 1 ]\nstack: [ 0 ]";
        let output = convert_to_diff_logs(input);
        assert_eq!(output, "stack: [ 0 1 ]\nrel stack: [ =1 -1 ]");
    }

    #[test]
    fn test_compute_stack_diff_change() {
        let input = "stack: [ 0 ]\nstack: [ 1 ]";
        let output = convert_to_diff_logs(input);
        assert_eq!(output, "stack: [ 0 ]\nrel stack: [ ~0:1 ]");
    }

    #[test]
    fn test_convert_from_diff_logs() {
        let diff_logs = "stack: [ 0 ]\nrel stack: [ =1 +1 ]";
        let output = convert_from_diff_logs(diff_logs);
        assert_eq!(output, "stack: [ 0 ]\nstack: [ 0 1 ]");
    }

    #[test]
    fn test_full_conversion_cycle() {
        let input = "stack: [ 0 ]\nstack: [ 0 1 ]\nstack: [ 0 1 2 ]";
        let diff_logs = convert_to_diff_logs(input);
        let restored = convert_from_diff_logs(&diff_logs);

        let original_lines = input.lines();
        let restored_lines = restored.lines();

        assert_eq!(original_lines.count(), restored_lines.count());
    }

    #[test]
    fn test_real_logs() {
        let logs = [
            "testdata/test-can-mint3000000.logs",
            "testdata/test-can-mint5000000.logs",
            "testdata/test-can-mint6000000.logs",
            "testdata/test-can-mint7000000.logs",
            "testdata/test-can-mint8000000.logs",
            "testdata/test-no-forward-ton-amount-no-forward10000000.logs",
            "testdata/test-not-wallet-owner-should-not-be-able-to-burn-jettons6000000.logs",
        ];
        for path in logs {
            let input = fs::read_to_string(path).unwrap();
            let diff_logs = convert_to_diff_logs(&input);
            let restored = convert_from_diff_logs(&diff_logs);

            let original_lines = input.lines();
            let restored_lines = restored.lines();

            assert_eq!(original_lines.count(), restored_lines.count());
        }
    }
}
