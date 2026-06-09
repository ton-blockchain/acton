use crate::context::Context;
use ton_executor::BaseExecutor;

pub mod assert;
pub mod bench;
pub mod crypto;
pub mod emulation;
pub mod env;
pub mod fs;
pub mod io;

#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum SearchParamIndex {
    To = 0,
    From = 1,
    Value = 2,
    ExitCode = 3,
    Success = 4,
    Aborted = 5,
    Deploy = 6,
    Bounce = 7,
    Bounced = 8,
    Opcode = 9,
    ActionExitCode = 10,
    ComputePhaseSkipped = 11,
    Body = 12,
    StateInit = 13,
    SendMode = 14,
}

impl SearchParamIndex {
    pub(crate) const fn as_usize(self) -> usize {
        self as usize
    }
}

pub fn register<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    io::register_extensions(executor, ctx);
    fs::register_extensions(executor, ctx);
    env::register_extensions(executor, ctx);
    assert::register_extensions(executor, ctx);
    bench::register_extensions(executor, ctx);
    emulation::register_extensions(executor, ctx);
    crypto::register_extensions(executor, ctx);
}
