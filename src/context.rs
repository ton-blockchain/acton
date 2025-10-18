use emulator::tuple::stack::Tuple;

#[derive(Debug, Clone)]
pub struct AssertFailure {
    pub left: Tuple,
    pub left_type: String,
    pub right: Tuple,
    pub right_type: String,
    pub message: Option<String>,
    pub location: Option<String>,
}

pub struct Context<'a> {
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_test_output: bool,
    pub assert_failure: &'a mut Option<AssertFailure>,
}
