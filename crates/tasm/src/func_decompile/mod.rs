mod ast;
mod inspect;
mod method_model;
mod pipeline;
mod render;
mod stage_patterns;
mod stage_rename;
mod stage_simplify;
mod stage_stack;

#[cfg(test)]
mod tests;

pub use pipeline::{FuncDecompiler, FuncDecompilerOptions};
