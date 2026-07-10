#[path = "../../../support/mod.rs"]
mod common;

pub(super) use common::MarkedSource;
use expect_test::Expect;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    CodeAction, DocumentEdits, DocumentUri, LanguageService, LanguageServiceConfig, Range,
    TextIndex,
};

pub(super) struct CodeActionTest<'a> {
    marked: MarkedSource,
    files: Vec<(&'a str, &'a str)>,
}

impl<'a> CodeActionTest<'a> {
    pub(super) fn new(source: &str) -> Self {
        Self {
            marked: MarkedSource::parse(source),
            files: Vec::new(),
        }
    }

    pub(super) fn file(mut self, uri: &'a str, source: &'a str) -> Self {
        self.files.push((uri, source));
        self
    }

    pub(super) fn check_applied(self, title: &str, expect: Expect) {
        let (actions, source, uri) = self.actions();
        let action = actions
            .iter()
            .find(|action| action.title == title)
            .unwrap_or_else(|| {
                let available = actions
                    .iter()
                    .map(|action| action.title.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                panic!("missing code action '{title}' for:\n{source}\navailable: {available}")
            });
        let document = action
            .edit
            .documents
            .iter()
            .find(|document| document.uri == uri)
            .expect("code action should edit the main document");

        expect.assert_eq(&apply_document_edits(&source, document));
    }

    pub(super) fn check_titles(self, expect: Expect) {
        let (actions, _, _) = self.actions();
        let actual = if actions.is_empty() {
            "<none>".to_owned()
        } else {
            actions
                .iter()
                .map(|action| action.title.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        expect.assert_eq(&actual);
    }

    fn actions(self) -> (Vec<CodeAction>, String, DocumentUri) {
        let uri = DocumentUri::from("file:///fixture/main.tolk");
        let mut service = LanguageService::new(LanguageServiceConfig::default());
        service.register_language(TolkLanguage::new());
        for (file_uri, source) in self.files {
            service
                .add_source_file(LANGUAGE_ID, file_uri, source)
                .expect("Tolk workspace source should be added");
        }
        service
            .open_document(uri.clone(), LANGUAGE_ID, 1, self.marked.source().to_owned())
            .expect("Tolk document should open");
        let position = self.marked.marker("caret").position;
        let actions = service
            .code_actions(&uri, Range::new(position, position))
            .expect("code action request should succeed");

        (actions, self.marked.source().to_owned(), uri)
    }
}

fn apply_document_edits(source: &str, document: &DocumentEdits) -> String {
    let index = TextIndex::new(source);
    let mut edits = document
        .edits
        .iter()
        .map(|edit| {
            (
                index.position_to_offset(source, edit.range.start),
                index.position_to_offset(source, edit.range.end),
                edit.new_text.as_str(),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));

    let mut result = source.to_owned();
    for (start, end, new_text) in edits {
        result.replace_range(start..end, new_text);
    }
    result
}
