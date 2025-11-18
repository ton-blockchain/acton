use crate::context::Context;
use emulator::traits::BaseExecutor;

pub mod assert;
pub mod emulation;
pub mod io;

pub fn register<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    io::register_extensions(executor, ctx);
    assert::register_extensions(executor, ctx);
    emulation::register_extensions(executor, ctx);
}
