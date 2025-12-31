use crate::context::Context;
use ton_executor::BaseExecutor;

pub mod assert;
pub mod emulation;
pub mod env;
pub mod fs;
pub mod io;

pub fn register<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    io::register_extensions(executor, ctx);
    fs::register_extensions(executor, ctx);
    env::register_extensions(executor, ctx);
    assert::register_extensions(executor, ctx);
    emulation::register_extensions(executor, ctx);
}
