use crate::debugging::support::debug::DebugResult;

pub struct DebugTestOutput {
    pub result: DebugResult,
}

impl DebugTestOutput {
    pub fn new(result: DebugResult) -> Self {
        Self { result }
    }
}

pub trait DebugTestOutputExt {
    fn assert_trace_steps(&self, expected_count: usize) -> &Self;
    fn assert_position_at_step(
        &self,
        step_index: usize,
        expected_file: &str,
        expected_line: u32,
        expected_column: u32,
    ) -> &Self;
    fn assert_variable_at_step(
        &self,
        step_index: usize,
        var_name: &str,
        expected_value: &str,
    ) -> &Self;
    fn assert_trace_snapshot_matches(&self, path: &str) -> &Self;
}

impl DebugTestOutputExt for DebugTestOutput {
    fn assert_trace_steps(&self, expected_count: usize) -> &Self {
        let actual_count = self.result.trace().steps.len();
        assert_eq!(
            actual_count, expected_count,
            "Expected {} trace steps, but got {}",
            expected_count, actual_count
        );
        self
    }

    fn assert_position_at_step(
        &self,
        step_index: usize,
        expected_file: &str,
        expected_line: u32,
        expected_column: u32,
    ) -> &Self {
        let step = self
            .result
            .trace()
            .steps
            .get(step_index)
            .expect(&format!("Step {} not found in trace", step_index));

        if let Some(pos) = step.positions.first() {
            let Some(source) = &pos.source else {
                return self;
            };
            let Some(path) = &source.path else {
                return self;
            };

            let filename = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path);

            assert_eq!(
                filename, expected_file,
                "File mismatch at step {}",
                step_index
            );
            assert_eq!(
                pos.line, expected_line as i64,
                "Line mismatch at step {}: expected {}, got {}",
                step_index, expected_line, pos.line
            );
            assert_eq!(
                pos.column, expected_column as i64,
                "Column mismatch at step {}: expected {}, got {}",
                step_index, expected_column, pos.column
            );
        } else {
            panic!("No position information at step {}", step_index);
        }
        self
    }

    fn assert_variable_at_step(
        &self,
        step_index: usize,
        var_name: &str,
        expected_value: &str,
    ) -> &Self {
        let step = self
            .result
            .trace()
            .steps
            .get(step_index)
            .unwrap_or_else(|| panic!("Step {} not found in trace", step_index));

        let var = step
            .variables
            .iter()
            .find(|v| v.name == var_name)
            .unwrap_or_else(|| panic!("Variable '{}' not found at step {}", var_name, step_index));

        assert_eq!(
            &var.value, expected_value,
            "Variable '{}' value mismatch at step {}: expected '{}', got '{}'",
            var_name, step_index, expected_value, var.value
        );
        self
    }

    fn assert_trace_snapshot_matches(&self, path: &str) -> &Self {
        self.result.assert_trace_snapshot_matches(path);
        self
    }
}
