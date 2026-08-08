mod context;
mod providers;

use super::TasmSpec;
use crate::completion::{CompletionProvider, collect_from_providers};
use crate::{CompletionList, DocumentSnapshot, Position};
use context::TasmCompletionContext;
use providers::InstructionCompletionProvider;

pub(super) fn completion(
    spec: &TasmSpec,
    document: &DocumentSnapshot,
    position: Position,
) -> CompletionList {
    let context = TasmCompletionContext::new(spec, document, position);
    let instructions = InstructionCompletionProvider;
    let providers: [&dyn CompletionProvider<TasmCompletionContext<'_>>; 1] = [&instructions];
    collect_from_providers(&context, &providers)
}
