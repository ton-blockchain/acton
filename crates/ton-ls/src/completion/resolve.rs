use dashmap::DashMap;
use lsp_types::{Command, TextEdit, Url};

#[derive(Debug, Clone)]
pub(crate) struct AddImportAction {
    pub(crate) target_uri: Url,
    pub(crate) import_path: String,
    pub(crate) symbol_name: String,
}

#[derive(Debug, Clone)]
pub(crate) enum CompletionPostAction {
    None,
    AddImport(AddImportAction),
    AdditionalTextEdits(Vec<TextEdit>),
    Command(Command),
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionResolvePayload {
    pub(crate) action: CompletionPostAction,
}

#[derive(Debug, Default)]
pub(crate) struct CompletionResolveStore {
    items: DashMap<String, CompletionResolvePayload>,
}

impl CompletionResolveStore {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(&self, resolve_id: impl Into<String>, payload: CompletionResolvePayload) {
        self.items.insert(resolve_id.into(), payload);
    }

    #[must_use]
    pub(crate) fn get(&self, resolve_id: &str) -> Option<CompletionResolvePayload> {
        self.items.get(resolve_id).map(|entry| entry.clone())
    }

    #[must_use]
    pub(crate) fn take(&self, resolve_id: &str) -> Option<CompletionResolvePayload> {
        self.items.remove(resolve_id).map(|(_, payload)| payload)
    }

    pub(crate) fn remove(&self, resolve_id: &str) {
        let _ = self.items.remove(resolve_id);
    }

    pub(crate) fn clear(&self) {
        self.items.clear();
    }
}
