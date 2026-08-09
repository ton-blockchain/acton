use super::schema::is_acton_manifest;
use crate::DocumentUri;
use crate::types::normalize_path;
use std::fs;
use std::path::{Path, PathBuf};
use ton_json_schema::SchemaPathSegment;
use url::Url;

pub(super) fn resolve_path(
    document_uri: &DocumentUri,
    schema_path: &[SchemaPathSegment],
    literal: &str,
) -> Option<DocumentUri> {
    if !is_acton_manifest(document_uri) {
        return None;
    }
    if !is_path_field(schema_path) {
        return None;
    }

    let configured_path = string_value(literal)?;
    let path = resolve_configured_path(document_uri, &configured_path)?;
    path_uri(&path).map(DocumentUri::from)
}

fn is_path_field(path: &[SchemaPathSegment]) -> bool {
    if path.len() == 2 && key(path, 0) == Some("import-mappings") {
        return true;
    }

    exact(path, &["build", "gen-dir"])
        || exact(path, &["build", "out-dir"])
        || exact(path, &["build", "output-abi"])
        || exact(path, &["build", "output-fift"])
        || exact(path, &["build", "output-sources"])
        || contract_field(path, &["src"])
        || contract_field(path, &["types"])
        || contract_field(path, &["output"])
        || contract_dependency_path(path)
        || contract_field(path, &["wrappers", "tolk", "output-dir"])
        || contract_field(path, &["wrappers", "tolk", "test-output-dir"])
        || contract_field(path, &["wrappers", "typescript", "output-dir"])
        || exact(path, &["test", "coverage", "output-file"])
        || exact(path, &["test", "gas-profile"])
        || exact(path, &["test", "junit-path"])
        || exact(path, &["test", "mutation", "rules-file"])
        || exact(path, &["localnet", "db-path"])
        || exact(path, &["wrappers", "tolk", "output-dir"])
        || exact(path, &["wrappers", "tolk", "test-output-dir"])
        || exact(path, &["wrappers", "typescript", "output-dir"])
}

fn contract_field(path: &[SchemaPathSegment], suffix: &[&str]) -> bool {
    path.len() == suffix.len() + 2
        && key(path, 0) == Some("contracts")
        && matches!(path.get(1), Some(SchemaPathSegment::Key(_)))
        && suffix
            .iter()
            .enumerate()
            .all(|(index, expected)| key(path, index + 2) == Some(*expected))
}

fn contract_dependency_path(path: &[SchemaPathSegment]) -> bool {
    path.len() == 5
        && key(path, 0) == Some("contracts")
        && matches!(path.get(1), Some(SchemaPathSegment::Key(_)))
        && key(path, 2) == Some("depends")
        && matches!(path.get(3), Some(SchemaPathSegment::Index(_)))
        && key(path, 4) == Some("path")
}

fn exact(path: &[SchemaPathSegment], expected: &[&str]) -> bool {
    path.len() == expected.len()
        && expected
            .iter()
            .enumerate()
            .all(|(index, value)| key(path, index) == Some(*value))
}

fn key(path: &[SchemaPathSegment], index: usize) -> Option<&str> {
    match path.get(index) {
        Some(SchemaPathSegment::Key(value)) => Some(value),
        Some(SchemaPathSegment::Index(_)) | None => None,
    }
}

fn string_value(text: &str) -> Option<String> {
    let text = text.trim();
    let value = toml::from_str::<toml::Value>(&format!("value = {text}")).ok()?;
    value.get("value")?.as_str().map(ToOwned::to_owned)
}

fn resolve_configured_path(document_uri: &DocumentUri, configured_path: &str) -> Option<PathBuf> {
    let manifest_path = document_uri.logical_path();
    let base_dir = manifest_path.parent().unwrap_or_else(|| Path::new("/"));
    let path = Path::new(configured_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    let normalized = normalize_path(&resolved);
    fs::metadata(&normalized).ok().map(|_| normalized)
}

#[cfg(not(target_arch = "wasm32"))]
fn path_uri(path: &Path) -> Option<String> {
    Url::from_file_path(path).ok().map(|uri| uri.to_string())
}

#[cfg(target_arch = "wasm32")]
fn path_uri(path: &Path) -> Option<String> {
    let mut uri = Url::parse("file:///").ok()?;
    uri.set_path(path.to_str()?);
    Some(uri.to_string())
}

#[cfg(test)]
mod tests {
    use super::{contract_dependency_path, contract_field, exact, is_path_field};
    use ton_json_schema::SchemaPathSegment;
    use ton_json_schema::SchemaPathSegment::{Index, Key};

    fn keys(parts: &[&str]) -> Vec<SchemaPathSegment> {
        parts.iter().map(|part| Key((*part).to_owned())).collect()
    }

    #[test]
    fn recognizes_manifest_path_fields() {
        assert!(is_path_field(&keys(&["build", "out-dir"])));
        assert!(is_path_field(&keys(&[
            "wrappers",
            "typescript",
            "output-dir"
        ])));
        assert!(!is_path_field(&keys(&["package", "name"])));
    }

    #[test]
    fn recognizes_contract_path_shapes() {
        let dependency = vec![
            Key("contracts".to_owned()),
            Key("counter".to_owned()),
            Key("depends".to_owned()),
            Index(0),
            Key("path".to_owned()),
        ];
        assert!(contract_field(
            &keys(&["contracts", "counter", "src"]),
            &["src"]
        ));
        assert!(contract_dependency_path(&dependency));
        assert!(exact(
            &keys(&["localnet", "db-path"]),
            &["localnet", "db-path"]
        ));
    }
}
