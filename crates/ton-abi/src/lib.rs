pub mod snake_string;

use path_absolutize::Absolutize;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn resolve_mapped_path<'a>(
    import_path: &'a str,
    mappings: Option<&BTreeMap<String, String>>,
) -> Cow<'a, str> {
    if import_path.starts_with("@stdlib/") || import_path.starts_with("@fiftlib/") {
        return Cow::Borrowed(import_path);
    }

    if let Some(rest) = import_path.strip_prefix('@') {
        let (prefix_without_at, path) = rest.split_once('/').unwrap_or((rest, ""));
        let prefix_with_at = &import_path[..=prefix_without_at.len()];

        if let Some(mappings) = mappings {
            // Try both with and without @ prefix in the mappings keys
            let mapping = mappings
                .get(prefix_without_at)
                .or_else(|| mappings.get(prefix_with_at));

            if let Some(mapping) = mapping {
                let mapped_path =
                    add_tolk_extension_if_needed_to_path(Path::new(mapping).join(path));
                let Ok(abs_path) = mapped_path.absolutize() else {
                    return Cow::Borrowed(import_path);
                };
                return Cow::Owned(abs_path.to_string_lossy().to_string());
            }
        }
    }

    Cow::Borrowed(import_path)
}

pub fn get_file_dependencies(
    file_path: &str,
    include_itself: bool,
    mappings: Option<&BTreeMap<String, String>>,
) -> anyhow::Result<Vec<String>> {
    if file_path.ends_with(".boc") {
        if include_itself {
            return Ok(vec![file_path.to_owned()]);
        }

        return Ok(vec![]);
    }

    let mut dependencies = collect_file_dependency_paths_from_imports(file_path, mappings)?;

    if include_itself {
        dependencies.push(file_path.to_string());
    }

    Ok(dependencies)
}

fn collect_file_dependency_paths_from_imports(
    file_path: &str,
    mappings: Option<&BTreeMap<String, String>>,
) -> anyhow::Result<Vec<String>> {
    let mut processed = HashSet::with_capacity(4);
    collect_file_dependency_paths_from_imports_recursive(
        file_path.to_owned(),
        mappings,
        &mut processed,
    );
    processed.remove(file_path);

    let mut dependencies: Vec<String> = processed.into_iter().collect();
    dependencies.sort_unstable();
    Ok(dependencies)
}

fn collect_file_dependency_paths_from_imports_recursive(
    file_path: String,
    mappings: Option<&BTreeMap<String, String>>,
    processed: &mut HashSet<String>,
) {
    if processed.contains(file_path.as_str()) {
        return;
    }

    let base_path = match Path::new(&file_path).parent() {
        Some(path) => path.to_path_buf(),
        None => return,
    };

    let Ok(file) = fs::File::open(&file_path) else {
        return;
    };
    processed.insert(file_path);

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        let line_trimmed = line.trim_start();
        if line_trimmed.is_empty() {
            continue;
        }

        let Some(import_path) = parse_import_path_line(&line) else {
            if line_trimmed.starts_with("fun ") || line_trimmed.starts_with("struct ") {
                // start of definitions
                break;
            }
            continue;
        };

        let import_path = resolve_mapped_path(import_path, mappings);
        let Some(resolved_path) =
            resolve_import_path_from_base_path(&base_path, import_path.as_ref())
        else {
            continue;
        };

        collect_file_dependency_paths_from_imports_recursive(resolved_path, mappings, processed);
    }
}

#[cfg(test)]
fn collect_import_paths_only(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(parse_import_path_line)
        .map(ToString::to_string)
        .collect()
}

fn parse_import_path_line(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let rest = line.strip_prefix("import")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end_quote = rest.find('"')?;
    let import_path = &rest[..end_quote];
    (!import_path.is_empty()).then_some(import_path)
}

fn resolve_import_path_from_base_path(base_path: &Path, import_path: &str) -> Option<String> {
    if Path::new(import_path).is_absolute() {
        // path can be absolute after mappings
        return Some(import_path.to_owned());
    }

    let import_path = add_tolk_extension_if_needed(import_path.to_string());

    if import_path.starts_with("./") || import_path.starts_with("../") {
        let relative_path = base_path.join(import_path);
        return Some(relative_path.to_string_lossy().to_string());
    }

    let relative_path = base_path.join(&import_path);
    if relative_path.exists() {
        return Some(relative_path.to_string_lossy().to_string());
    }

    let with_ext = format!("{import_path}.tolk");
    let path_with_ext = base_path.join(with_ext);
    if path_with_ext.exists() {
        Some(path_with_ext.to_string_lossy().to_string())
    } else {
        None
    }
}

fn add_tolk_extension_if_needed(path: String) -> String {
    if path.ends_with(".tolk") {
        return path;
    }
    format!("{path}.tolk")
}

fn add_tolk_extension_if_needed_to_path(path: PathBuf) -> PathBuf {
    if path.extension() == Some(OsStr::new("tolk")) {
        return path;
    }
    path.with_extension("tolk")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("ton-abi-{prefix}-{}-{now}", std::process::id()))
    }

    #[test]
    fn test_collect_import_paths_only_simple_lines() {
        let content = r#"
    import "a"
import "b/c"
"#;

        let imports = collect_import_paths_only(content);
        assert_eq!(imports, vec!["a".to_string(), "b/c".to_string()]);
    }

    #[test]
    fn test_get_file_dependencies_scans_only_imports_and_recurses() {
        let test_dir = unique_test_dir("deps");
        fs::create_dir_all(&test_dir).unwrap();

        let main = test_dir.join("main.tolk");
        let dep_a = test_dir.join("a.tolk");
        let dep_b = test_dir.join("b.tolk");

        fs::write(
            &main,
            r#"
import "a"
// import "ignored"
this is invalid syntax but should not affect import scanning
"#,
        )
        .unwrap();
        fs::write(&dep_a, "import \"./b\"\n").unwrap();
        fs::write(&dep_b, "let x = 1\n").unwrap();

        let main_str = main.to_string_lossy().to_string();
        let deps = get_file_dependencies(&main_str, true, None).unwrap();

        let deps_canon: HashSet<String> = deps
            .iter()
            .map(|dep| {
                dunce::canonicalize(dep)
                    .unwrap_or_else(|_| PathBuf::from(dep))
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert!(
            deps_canon.contains(
                &dunce::canonicalize(&dep_a)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert!(
            deps_canon.contains(
                &dunce::canonicalize(&dep_b)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert_eq!(deps.last(), Some(&main_str));

        let _ = fs::remove_file(&main);
        let _ = fs::remove_file(&dep_a);
        let _ = fs::remove_file(&dep_b);
        let _ = fs::remove_dir_all(&test_dir);
    }
}
