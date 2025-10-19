use abi::ABI;
use emulator::blockchain::Blockchain;
use emulator::tuple::stack::Tuple;

#[derive(Debug, Clone)]
pub struct AssertBinFailure {
    pub operator: String,
    pub left: Tuple,
    pub left_type: String,
    pub right: Tuple,
    pub right_type: String,
    pub message: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FailAssertFailure {
    pub message: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AssertFailure {
    Bin(AssertBinFailure),
    Fail(FailAssertFailure),
}

impl AssertFailure {
    pub fn message(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.message.clone(),
            AssertFailure::Fail(arg) => arg.message.clone(),
        }
    }

    pub fn location(&self) -> Option<String> {
        match self {
            AssertFailure::Bin(arg) => arg.location.clone(),
            AssertFailure::Fail(arg) => arg.location.clone(),
        }
    }
}

pub struct Context<'a> {
    pub stdout_buffer: String,
    pub stderr_buffer: String,
    pub capture_test_output: bool,
    pub assert_failure: &'a mut Option<AssertFailure>,
    pub blockchain: &'a mut Blockchain,
    pub abi: ABI,
}
