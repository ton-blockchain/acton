use ton_executor::get::step::StepGetExecutor;
use ton_executor::message::step::StepExecutor;

#[derive(Clone)]
pub enum AnyExecutor {
    Get(StepGetExecutor),
    Message(StepExecutor),
}

impl AnyExecutor {
    pub fn step(&self) -> bool {
        match self {
            AnyExecutor::Get(get) => get.step(),
            AnyExecutor::Message(msg) => msg.step(),
        }
    }

    pub fn get_code_pos(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_code_pos(),
            AnyExecutor::Message(msg) => msg.get_code_pos(),
        }
    }

    pub fn get_stack(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_stack(),
            AnyExecutor::Message(msg) => msg.get_stack(),
        }
    }

    pub fn get_c7(&self) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_c7(),
            AnyExecutor::Message(msg) => msg.get_c7(),
        }
    }

    pub fn get_control_register(&self, idx: usize) -> String {
        match self {
            AnyExecutor::Get(get) => get.get_control_register(idx),
            AnyExecutor::Message(msg) => msg.get_control_register(idx),
        }
    }
}
