use crate::common::{MarkedSource, dedent_block};
use expect_test::Expect;
use std::fmt::Write as _;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    CompletionItem, CompletionItemKind, CompletionTrigger, DocumentUri, InsertTextFormat,
    LanguageService, LanguageServiceConfig, Position, Range, TextEdit, TextIndex, WorkspaceConfig,
};

pub(super) struct CompletionTest<'a> {
    source: &'a str,
    uri: &'a str,
    manifest: &'a str,
    files: Vec<(&'a str, &'a str)>,
    labels: Vec<&'a str>,
    prefix: Option<&'a str>,
    trigger: CompletionTrigger,
}

impl<'a> CompletionTest<'a> {
    pub(super) const fn new(source: &'a str) -> Self {
        Self {
            source,
            uri: "file:///workspace/main.tolk",
            manifest: "",
            files: Vec::new(),
            labels: Vec::new(),
            prefix: None,
            trigger: CompletionTrigger::invoked(),
        }
    }

    pub(super) const fn uri(mut self, uri: &'a str) -> Self {
        self.uri = uri;
        self
    }

    pub(super) const fn manifest(mut self, manifest: &'a str) -> Self {
        self.manifest = manifest;
        self
    }

    pub(super) fn file(mut self, path: &'a str, source: &'a str) -> Self {
        self.files.push((path, source));
        self
    }

    pub(super) fn labels(mut self, labels: &'a [&'a str]) -> Self {
        self.labels.extend_from_slice(labels);
        self
    }

    pub(super) const fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub(super) fn trigger_character(mut self, character: &'a str) -> Self {
        self.trigger = CompletionTrigger::character(character);
        self
    }

    pub(super) fn check(self, expected: Expect) {
        let completion = self
            .complete()
            .unwrap_or_else(|error| panic!("completion test failed: {error:#}"));
        let actual = render_completion_table(&completion.items);
        expected.assert_eq(&actual);
    }

    pub(super) fn check_applied(self, label: &'a str, expected: Expect) {
        self.check_applied_matching(label, None, expected);
    }

    pub(super) fn check_applied_kind(
        self,
        label: &'a str,
        kind: CompletionItemKind,
        expected: Expect,
    ) {
        self.check_applied_matching(label, Some(kind), expected);
    }

    fn check_applied_matching(
        self,
        label: &'a str,
        kind: Option<CompletionItemKind>,
        expected: Expect,
    ) {
        let completion = self
            .complete()
            .unwrap_or_else(|error| panic!("completion test failed: {error:#}"));
        let matching = completion
            .items
            .iter()
            .filter(|item| item.label == label && kind.is_none_or(|kind| item.kind == Some(kind)))
            .collect::<Vec<_>>();
        let [item] = matching.as_slice() else {
            let available = completion
                .items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            panic!(
                "expected exactly one completion item labeled '{label}' with kind {kind:?}, got \
                 {}; available: [{available}]",
                matching.len(),
            );
        };
        let actual = apply_completion(&completion.source, completion.position, item);
        expected.assert_eq(&actual);
    }

    fn complete(self) -> anyhow::Result<CompletionResult> {
        let marked = MarkedSource::parse(self.source);
        let [marker] = marked.markers() else {
            anyhow::bail!("completion source must contain exactly one <caret> marker");
        };
        let uri = DocumentUri::from(self.uri);
        let mut service = LanguageService::new(LanguageServiceConfig::default());
        service.register_language(TolkLanguage::new());
        service.set_workspace_config(
            LANGUAGE_ID,
            WorkspaceConfig::new(
                "file:///workspace",
                Some(DocumentUri::from("file:///workspace/Acton.toml")),
                dedent_block(self.manifest),
            ),
        )?;
        for (path, source) in self.files {
            let uri = if path.contains("://") {
                path.to_owned()
            } else {
                format!("file:///workspace/{}", path.trim_start_matches('/'))
            };
            service.add_source_file(LANGUAGE_ID, uri, dedent_block(source))?;
        }
        service.open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())?;
        let completion = service.completion(&uri, marker.position, self.trigger)?;
        Ok(CompletionResult {
            source: marked.source().to_owned(),
            position: marker.position,
            items: select(completion.items, &self.labels, self.prefix),
        })
    }
}

struct CompletionResult {
    source: String,
    position: Position,
    items: Vec<CompletionItem>,
}

fn render_completion_table(items: &[CompletionItem]) -> String {
    if items.is_empty() {
        return "<none>".to_owned();
    }
    let rows = items
        .iter()
        .map(|item| {
            let kind = item
                .kind
                .map_or_else(|| "-".to_owned(), |kind| format!("{kind:?}"));
            let detail = item.detail.as_deref().map_or_else(String::new, escape);
            let (edit, text) = if let Some(edit) = &item.text_edit {
                (
                    format!(
                        "{}:{}-{}:{}",
                        edit.range.start.line,
                        edit.range.start.character,
                        edit.range.end.line,
                        edit.range.end.character,
                    ),
                    escape(&edit.new_text),
                )
            } else {
                (
                    String::new(),
                    item.insert_text.as_deref().map_or_else(String::new, escape),
                )
            };
            (escape(&item.label), kind, detail, edit, text)
        })
        .collect::<Vec<_>>();
    let label_width = column_width("label", rows.iter().map(|row| row.0.as_str()));
    let kind_width = column_width("kind", rows.iter().map(|row| row.1.as_str()));
    let detail_width = column_width("detail", rows.iter().map(|row| row.2.as_str()));
    let edit_width = column_width("edit", rows.iter().map(|row| row.3.as_str()));
    let mut output = format!(
        "{:<label_width$}  {:<kind_width$}  {:<detail_width$}  {:<edit_width$}  text",
        "label", "kind", "detail", "edit",
    );
    for (label, kind, detail, edit, text) in rows {
        output.push('\n');
        let _ = write!(
            output,
            "{label:<label_width$}  {kind:<kind_width$}  {detail:<detail_width$}  {edit:<edit_width$}  {text}",
        );
    }
    output
}

fn column_width<'a>(header: &str, values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).fold(header.len(), usize::max)
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn apply_completion(source: &str, position: Position, item: &CompletionItem) -> String {
    let text_index = TextIndex::new(source);
    let default_range = Range::new(position, position);
    let main_edit = item.text_edit.clone().unwrap_or_else(|| {
        TextEdit::new(
            default_range,
            item.insert_text
                .clone()
                .unwrap_or_else(|| item.label.clone()),
        )
    });
    let (main_text, tab_stop) = if item.insert_text_format == InsertTextFormat::Snippet {
        expand_snippet(&main_edit.new_text)
    } else {
        (main_edit.new_text.clone(), main_edit.new_text.len())
    };
    let main_start = text_index.position_to_offset(source, main_edit.range.start);
    let mut edits = item
        .additional_text_edits
        .iter()
        .cloned()
        .map(|edit| (edit, false))
        .collect::<Vec<_>>();
    edits.push((TextEdit::new(main_edit.range, main_text), true));
    edits.sort_by_key(|(edit, _)| {
        std::cmp::Reverse(text_index.position_to_offset(source, edit.range.start))
    });

    let mut result = source.to_owned();
    let mut caret = main_start + tab_stop;
    let mut main_applied = false;
    for (edit, is_main) in edits {
        let start = text_index.position_to_offset(source, edit.range.start);
        let end = text_index.position_to_offset(source, edit.range.end);
        result.replace_range(start..end, &edit.new_text);
        if is_main {
            main_applied = true;
            caret = start + tab_stop;
        } else if main_applied && start <= caret {
            caret = caret.saturating_sub(end - start) + edit.new_text.len();
        }
    }
    result.insert_str(caret, "<caret>");
    result
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn expand_snippet(snippet: &str) -> (String, usize) {
    let bytes = snippet.as_bytes();
    let mut output = String::with_capacity(snippet.len());
    let mut tab_stops = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            output.push(bytes[index + 1] as char);
            index += 2;
            continue;
        }
        if bytes[index] != b'$' {
            let character = snippet[index..]
                .chars()
                .next()
                .expect("snippet index must be on a character boundary");
            output.push(character);
            index += character.len_utf8();
            continue;
        }

        if bytes.get(index + 1) == Some(&b'{') {
            let Some(relative_end) = snippet[index + 2..].find('}') else {
                output.push('$');
                index += 1;
                continue;
            };
            let end = index + 2 + relative_end;
            let placeholder = &snippet[index + 2..end];
            let (number, default) = placeholder
                .split_once(':')
                .map_or((placeholder, ""), |(number, default)| (number, default));
            if let Ok(number) = number.parse::<usize>() {
                output.push_str(default);
                tab_stops.push((number, output.len()));
                index = end + 1;
                continue;
            }
        } else {
            let digits_start = index + 1;
            let mut digits_end = digits_start;
            while bytes.get(digits_end).is_some_and(u8::is_ascii_digit) {
                digits_end += 1;
            }
            if digits_end > digits_start
                && let Ok(number) = snippet[digits_start..digits_end].parse::<usize>()
            {
                tab_stops.push((number, output.len()));
                index = digits_end;
                continue;
            }
        }
        output.push('$');
        index += 1;
    }

    let caret = tab_stops
        .iter()
        .filter(|(number, _)| *number > 0)
        .min_by_key(|(number, _)| *number)
        .or_else(|| tab_stops.iter().find(|(number, _)| *number == 0))
        .map_or(output.len(), |(_, offset)| *offset);
    (output, caret)
}

fn select(
    items: Vec<CompletionItem>,
    labels: &[&str],
    prefix: Option<&str>,
) -> Vec<CompletionItem> {
    items
        .into_iter()
        .filter(|item| {
            (labels.is_empty() || labels.iter().any(|label| item.label == *label))
                && prefix.is_none_or(|prefix| item.label.starts_with(prefix))
        })
        .collect()
}
