use super::TolkCompletionProviderContext;
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// Completes Tolk import paths from workspace files, stdlib roots, and Acton mappings.
///
/// Suggestions are limited to the current path segment and preserve the logical
/// import spelling expected by the configured project.
pub(crate) struct ImportPathCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for ImportPathCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        context.syntax.inside_import()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let (import_path, full_range) =
            super::string_prefix_and_range(context.syntax, context.document)?;
        let current_path = context.document.uri().logical_path();
        let current_dir = current_path.parent().unwrap_or_else(|| Path::new("/"));
        if matches!(import_path.as_str(), "" | "@") {
            add_path(
                collector,
                &import_path,
                full_range,
                "@stdlib/",
                CompletionItemKind::Folder,
                None,
            );
            if let Some(mappings) = context.workspace.mappings {
                for alias in mappings.keys() {
                    let alias = alias.strip_prefix('@').unwrap_or(alias);
                    add_path(
                        collector,
                        &import_path,
                        full_range,
                        &format!("@{alias}/"),
                        CompletionItemKind::Folder,
                        None,
                    );
                }
            }
        }

        let (root, relative_prefix) = if let Some(path) = import_path.strip_prefix("@stdlib/") {
            (context.workspace.stdlib_path, path)
        } else if let Some((root, path)) = mapped_path(&import_path, context.workspace.mappings) {
            (root, path)
        } else if import_path.starts_with('@') {
            return None;
        } else {
            (current_dir, import_path.as_str())
        };
        let (directory, segment) = split_path_prefix(relative_prefix);
        let directory = normalize_path(&root.join(directory));
        let segment_start = context.syntax.offset.saturating_sub(segment.len());
        let range = context.document.text_index().range_for_offsets(
            context.document.text(),
            segment_start,
            context.syntax.offset,
        );
        let mut files = BTreeSet::new();
        let mut folders = BTreeSet::new();
        for path in context.workspace.paths {
            if path == &current_path || path.extension().is_none_or(|extension| extension != "tolk")
            {
                continue;
            }
            let Ok(relative) = path.strip_prefix(&directory) else {
                continue;
            };
            let mut components = relative.components();
            let Some(first) = components.next() else {
                continue;
            };
            let first = first.as_os_str().to_string_lossy();
            if components.next().is_some() {
                let label = format!("{first}/");
                if label.starts_with(segment) {
                    folders.insert(label);
                }
            } else if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
                && stem.starts_with(segment)
            {
                files.insert(stem.to_owned());
            }
        }
        for folder in folders {
            add_path(
                collector,
                segment,
                range,
                &folder,
                CompletionItemKind::Folder,
                None,
            );
        }
        for file in files {
            add_path(
                collector,
                segment,
                range,
                &file,
                CompletionItemKind::File,
                Some(".tolk"),
            );
        }
        Some(())
    }
}

fn add_path(
    collector: &mut CompletionCollector,
    prefix: &str,
    range: crate::Range,
    path: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
) {
    let mut item = CompletionItem::new(path, kind).with_replacement(range, path);
    item.detail = detail.map(str::to_owned);
    collector.add(
        item,
        CompletionRank::new(CompletionCategory::ContextElement).with_prefix(prefix, path),
    );
}

fn mapped_path<'a>(
    import_path: &'a str,
    mappings: Option<&'a std::collections::BTreeMap<String, String>>,
) -> Option<(&'a Path, &'a str)> {
    for (alias, root) in mappings? {
        let alias = alias.strip_prefix('@').unwrap_or(alias);
        let prefix = format!("@{alias}/");
        if let Some(path) = import_path.strip_prefix(&prefix) {
            return Some((Path::new(root), path));
        }
    }
    None
}

fn split_path_prefix(path: &str) -> (&str, &str) {
    path.rfind(['/', '\\'])
        .map_or(("", path), |separator| path.split_at(separator + 1))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                result.push(component.as_os_str());
            }
        }
    }
    result
}
