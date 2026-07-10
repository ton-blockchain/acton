mod context;
mod providers;

use super::FiftParsedDocument;
use crate::completion::{CompletionProvider, collect_from_providers};
use crate::{CompletionList, DocumentSnapshot, Position};
use context::FiftCompletionContext;
use providers::{DeclarationCompletionProvider, LanguageWordCompletionProvider};

pub(super) fn completion(
    document: &DocumentSnapshot,
    parsed: &FiftParsedDocument,
    position: Position,
) -> CompletionList {
    let context = FiftCompletionContext::new(document, parsed, position);
    let declarations = DeclarationCompletionProvider;
    let language_words = LanguageWordCompletionProvider;
    let providers: [&dyn CompletionProvider<FiftCompletionContext<'_>>; 2] =
        [&declarations, &language_words];
    collect_from_providers(&context, &providers)
}
