mod context;
pub(super) mod providers;

use super::TlbParsedDocument;
use crate::completion::{CompletionProvider, collect_from_providers};
use crate::{CompletionList, DocumentSnapshot, Position};
use context::TlbCompletionContext;
use providers::{BuiltinTypesCompletionProvider, ReferenceCompletionProvider};

pub(super) fn completion(
    document: &DocumentSnapshot,
    _parsed: &TlbParsedDocument,
    position: Position,
) -> anyhow::Result<CompletionList> {
    let Some(context) = TlbCompletionContext::new(document, position)? else {
        return Ok(CompletionList::default());
    };

    let references = ReferenceCompletionProvider;
    let builtin_types = BuiltinTypesCompletionProvider;
    let providers: [&dyn CompletionProvider<TlbCompletionContext>; 2] =
        [&references, &builtin_types];
    Ok(collect_from_providers(&context, &providers))
}
