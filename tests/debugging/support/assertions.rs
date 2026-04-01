use crate::debugging::support::debug::DebugResult;

pub(crate) struct DebugTestOutput {
    pub result: DebugResult,
}

impl DebugTestOutput {
    pub(crate) fn new(result: DebugResult) -> Self {
        Self { result }
    }
}

pub(crate) trait DebugTestOutputExt {
    fn assert_trace_steps(&self, expected_count: usize) -> &Self;
    #[allow(dead_code)]
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
            "Expected {expected_count} trace steps, but got {actual_count}"
        );
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
            .unwrap_or_else(|| panic!("Step {step_index} not found in trace"));

        let var = step
            .variables
            .iter()
            .find(|v| v.name == var_name)
            .unwrap_or_else(|| panic!("Variable '{var_name}' not found at step {step_index}"));

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
