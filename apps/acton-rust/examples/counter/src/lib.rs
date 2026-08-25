#![no_std]

use acton_lang::contract;

#[contract(
    name = "Counter",
    author = "TON Core",
    version = "0.1.0",
    description = "Minimal counter written with the Acton Rust DSL"
)]
pub mod counter {
    use acton_lang::{Context, ViewContext, get, message, receive, storage};

    #[storage]
    pub struct State {
        pub id: u32,
        pub counter: u32,
    }

    #[message(op = 0x7e8764ef)]
    pub struct IncreaseCounter {
        pub increase_by: u32,
    }

    #[message(op = 0x3a752f06)]
    pub struct ResetCounter {}

    #[receive]
    pub fn increase(ctx: Context<'_, State>, msg: IncreaseCounter) {
        ctx.state.counter += msg.increase_by * 2;
    }

    #[receive]
    pub fn reset(ctx: Context<'_, State>, _msg: ResetCounter) {
        ctx.state.counter = 0;
    }

    #[get]
    pub fn current_counter(ctx: ViewContext<'_, State>) -> u32 {
        ctx.state.counter
    }
}
