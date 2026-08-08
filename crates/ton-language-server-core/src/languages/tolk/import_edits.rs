use super::TolkResolveSnapshot;
use crate::{DocumentSnapshot, Range, TextEdit};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub(super) fn import_path_for(
    snapshot: &TolkResolveSnapshot,
    file_id: u32,
    target: &Path,
    stdlib_path: &Path,
    mappings: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    let current = snapshot.project_index.files().get(&file_id)?;
    import_path_from(&current.path, target, stdlib_path, mappings)
}

pub(super) fn import_path_from(
    current_file: &Path,
    target: &Path,
    stdlib_path: &Path,
    mappings: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    if let Ok(relative) = target.strip_prefix(stdlib_path) {
        return Some(format!("@stdlib/{}", path_without_tolk(relative)));
    }
    if let Some(mappings) = mappings {
        for (mapping, root) in mappings {
            if let Ok(relative) = target.strip_prefix(root) {
                let relative = path_without_tolk(relative);
                return Some(if relative.is_empty() {
                    mapping.clone()
                } else {
                    format!("{mapping}/{relative}")
                });
            }
        }
    }

    Some(path_without_tolk(&relative_path(
        current_file.parent()?,
        target,
    )?))
}

pub(super) fn import_edit(
    document: &DocumentSnapshot,
    root: tree_sitter::Node<'_>,
    import_path: &str,
) -> TextEdit {
    let mut cursor = root.walk();
    let leading = root
        .named_children(&mut cursor)
        .take_while(|node| matches!(node.kind(), "tolk_required_version" | "import_directive"))
        .last();
    let (offset, text) = match leading {
        Some(node) if node.kind() == "import_directive" => (
            line_end_offset(document.text(), node.end_byte()),
            format!("\nimport \"{import_path}\"\n"),
        ),
        Some(node) => (
            line_end_offset(document.text(), node.end_byte()),
            format!("\n\nimport \"{import_path}\"\n"),
        ),
        None => (0, format!("import \"{import_path}\"\n\n")),
    };
    let position = document
        .text_index()
        .offset_to_position(document.text(), offset);
    TextEdit::new(Range::new(position, position), text)
}

fn line_end_offset(source: &str, offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|tail| tail.find('\n'))
        .map_or(source.len(), |line_end| offset + line_end)
}

fn path_without_tolk(path: &Path) -> String {
    let mut path = path.to_path_buf();
    if path
        .extension()
        .is_some_and(|extension| extension == "tolk")
    {
        path.set_extension("");
    }
    path.to_string_lossy().replace('\\', "/")
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return None;
    }
    let mut result = PathBuf::new();
    for component in &from[common..] {
        if !matches!(component, Component::CurDir) {
            result.push("..");
        }
    }
    for component in &to[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}
