#![no_std]

pub use acton_lang_macros::{contract, get, message, receive, storage};

pub struct Context<'state, State> {
    pub state: &'state mut State,
}

impl<'state, State> Context<'state, State> {
    pub const fn new(state: &'state mut State) -> Self {
        Self { state }
    }
}

pub struct ViewContext<'state, State> {
    pub state: &'state State,
}

impl<'state, State> ViewContext<'state, State> {
    pub const fn new(state: &'state State) -> Self {
        Self { state }
    }
}

pub mod prelude {
    pub use crate::{Context, ViewContext, contract, get, message, receive, storage};
}
