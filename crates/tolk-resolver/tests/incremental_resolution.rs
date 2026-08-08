#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolk_resolver::{FileDb, FileId, ProjectIndex, ProjectSource, ProjectSourceProvider, resolve};

const MAIN_PATH: &str = "/workspace/main.tolk";
const LIB_PATH: &str = "/workspace/lib.tolk";
const ALT_PATH: &str = "/workspace/alt.tolk";
const WORKSPACE_COMMON_PATH: &str = "/workspace/support/common.tolk";

#[test]
fn body_edit_reuses_every_other_file() {
    let mut fixture = Fixture::new();
    let first = fixture.resolved_project();

    fixture.set(
        MAIN_PATH,
        r#"
            import "lib"

            fun main(): int {
                return helper() + 1;
            }
        "#,
    );
    let second = fixture.project();

    check_reuse(
        first,
        second,
        &[MAIN_PATH],
        expect![[r#"
        reused: 2
        pending: main.tolk
        shared: alt.tolk, lib.tolk
    "#]],
    );
}

#[test]
fn exported_name_change_invalidates_importers() {
    let mut fixture = Fixture::new();
    let first = fixture.resolved_project();

    fixture.set(LIB_PATH, "fun renamed(): int { return 1; }");
    let second = fixture.project();

    check_reuse(
        first,
        second,
        &[LIB_PATH],
        expect![[r#"
        reused: 1
        pending: lib.tolk, main.tolk
        shared: alt.tolk
    "#]],
    );
}

#[test]
fn import_change_only_invalidates_the_edited_file() {
    let mut fixture = Fixture::new();
    let first = fixture.resolved_project();

    fixture.set(
        MAIN_PATH,
        r#"
            import "alt"

            fun main(): int {
                return alternative();
            }
        "#,
    );
    let second = fixture.project();

    check_reuse(
        first,
        second,
        &[MAIN_PATH],
        expect![[r#"
        reused: 2
        pending: main.tolk
        shared: alt.tolk, lib.tolk
    "#]],
    );
}

#[test]
fn project_index_fast_path_updates_spans_and_shares_unchanged_files() {
    let mut fixture = Fixture::new();
    let first = fixture.resolved_project();
    let main_id = file_id(&first, MAIN_PATH);
    let previous_import_span = first.imports()[&main_id][0].import().span;

    fixture.set(
        MAIN_PATH,
        r#"
            // The leading comment relocates both the import and declaration.
            import "lib"

            fun main(): int {
                return helper() + 1;
            }
        "#,
    );

    let changed = BTreeSet::from([main_id]);
    let mut second = first
        .with_updated_files(&fixture.file_db, &changed)
        .expect("body-only edit should use the project-index fast path");
    let current_file = fixture
        .file_db
        .get_by_id(main_id)
        .expect("changed file must remain indexed");
    let current_import_span = current_file.index().imports[0].span;
    let resolution_cache_cleared = second.resolved_uses().is_empty();
    let reused = second.reuse_resolved_uses_from(&first, &changed);

    let actual = format!(
        "main index shared: {}\nunchanged indexes shared: {}\nimport span changed: {}\nimport span current: {}\nresolution cache cleared: {}\nresolution entries reused: {reused}\n",
        Arc::ptr_eq(&first.files()[&main_id], &second.files()[&main_id]),
        [LIB_PATH, ALT_PATH].iter().all(|path| {
            let file_id = file_id(&first, path);
            Arc::ptr_eq(&first.files()[&file_id], &second.files()[&file_id])
        }),
        previous_import_span != current_import_span,
        second.imports()[&main_id][0].import().span == current_import_span,
        resolution_cache_cleared,
    );

    expect![[r#"
        main index shared: false
        unchanged indexes shared: true
        import span changed: true
        import span current: true
        resolution cache cleared: true
        resolution entries reused: 2
    "#]]
    .assert_eq(&actual);
}

#[test]
fn project_index_fast_path_accepts_semantic_and_error_tolerant_edits() {
    let mut fixture = Fixture::new();
    let initial = fixture.project();
    let lib_id = file_id(&initial, LIB_PATH);
    let main_id = file_id(&initial, MAIN_PATH);

    fixture.set(
        LIB_PATH,
        "fun helper(value: int): bool { return value > 0; }",
    );
    let signature = initial
        .with_updated_files(&fixture.file_db, &BTreeSet::from([lib_id]))
        .is_some();

    let after_signature = fixture.project();
    fixture.set(LIB_PATH, "fun helper(value: int): bool { return value >");
    let incomplete_body = after_signature
        .with_updated_files(&fixture.file_db, &BTreeSet::from([lib_id]))
        .is_some();

    fixture.set(
        LIB_PATH,
        "fun helper(value: int): bool { return value >= 0; }",
    );
    fixture.set(
        MAIN_PATH,
        r#"
            import "lib"

            fun main(): int {
                return helper(1) ? 1 : 0;
            }
        "#,
    );
    let multiple_files = after_signature
        .with_updated_files(&fixture.file_db, &BTreeSet::from([lib_id, main_id]))
        .map(|updated| {
            let alt_id = file_id(&initial, ALT_PATH);
            Arc::ptr_eq(&after_signature.files()[&alt_id], &updated.files()[&alt_id])
        });

    let actual = format!(
        "signature edit: {signature}\nincomplete body: {incomplete_body}\nmultiple files: {multiple_files:?}\n"
    );
    expect![[r#"
        signature edit: true
        incomplete body: true
        multiple files: Some(true)
    "#]]
    .assert_eq(&actual);
}

#[test]
fn project_index_fast_path_rejects_graph_and_global_symbol_changes() {
    let mut fixture = Fixture::new();
    let initial = fixture.project();
    let main_id = file_id(&initial, MAIN_PATH);
    let lib_id = file_id(&initial, LIB_PATH);

    fixture.set(
        MAIN_PATH,
        r#"
            import "alt"

            fun main(): int {
                return alternative();
            }
        "#,
    );
    let import_changed = initial
        .with_updated_files(&fixture.file_db, &BTreeSet::from([main_id]))
        .is_some();

    fixture.set(
        LIB_PATH,
        r#"
            fun helper(): int { return 1; }
            fun added(): int { return 2; }
        "#,
    );
    let declaration_added = initial
        .with_updated_files(&fixture.file_db, &BTreeSet::from([lib_id]))
        .is_some();

    fixture.set(
        LIB_PATH,
        r#"
            struct Storage {
                counter: int
            }
        "#,
    );
    let struct_project = fixture.project();
    let struct_id = file_id(&struct_project, LIB_PATH);
    fixture.set(
        LIB_PATH,
        r#"
            struct Storage {
                renamed: int
            }
        "#,
    );
    let nested_field_renamed = struct_project
        .with_updated_files(&fixture.file_db, &BTreeSet::from([struct_id]))
        .is_some();
    let no_changed_files = struct_project
        .with_updated_files(&fixture.file_db, &BTreeSet::new())
        .is_some();

    let actual = format!(
        "import changed: {import_changed}\ndeclaration added: {declaration_added}\nnested field renamed: {nested_field_renamed}\nno changed files: {no_changed_files}\n"
    );
    expect![[r#"
        import changed: false
        declaration added: false
        nested field renamed: false
        no changed files: false
    "#]]
    .assert_eq(&actual);
}

#[test]
fn global_environment_uses_only_the_stdlib_common_file_as_prelude() {
    let stdlib_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolk-compiler/assets/tolk-stdlib");
    let mut fixture = Fixture {
        file_db: FileDb::new(stdlib_path, None),
        provider: MemoryProvider::default(),
    };
    fixture.set(LIB_PATH, "fun helper(): int { return 1; }");
    fixture.set(ALT_PATH, "fun alternative(): int { return 2; }");
    fixture.set(MAIN_PATH, "fun main() {}");
    fixture.set(
        fixture.file_db.stdlib_path().join("common.tolk"),
        "type stdlib_only = int",
    );
    fixture.set(WORKSPACE_COMMON_PATH, "type workspace_only = int");

    let project = ProjectIndex::builder(&fixture.file_db, PathBuf::from(MAIN_PATH))
        .with_additional_roots([PathBuf::from(WORKSPACE_COMMON_PATH)])
        .with_stdlib(fixture.file_db.stdlib_path().to_owned())
        .build_with_provider(&fixture.provider)
        .expect("test project must build");
    let main_id = file_id(&project, MAIN_PATH);
    let environment = tolk_resolver::symbol_resolver::GlobalEnv::new(&project, main_id);

    let mut actual = String::new();
    for name in ["stdlib_only", "workspace_only"] {
        let mut paths = environment
            .visible
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|symbol_id| project.files().get(&symbol_id.file_id))
            .map(|file| {
                format!(
                    "{:?}/{}",
                    file.source_kind,
                    file.path
                        .file_name()
                        .expect("test file must have a name")
                        .to_string_lossy()
                )
            })
            .collect::<Vec<_>>();
        paths.sort();

        let visible_from = if paths.is_empty() {
            "<hidden>".to_owned()
        } else {
            paths.join(", ")
        };
        let _ = writeln!(actual, "{name}: {visible_from}");
    }

    expect![[r#"
        stdlib_only: Stdlib/common.tolk
        workspace_only: <hidden>
    "#]]
    .assert_eq(&actual);
}

#[test]
fn file_db_fork_preserves_ids_and_isolates_speculative_edits() {
    let file_db = FileDb::new(PathBuf::from("/__stdlib__"), None);
    let original_main = file_db
        .process_content(PathBuf::from(MAIN_PATH), "fun main() {}")
        .expect("main file must parse");
    let original_lib = file_db
        .process_content(PathBuf::from(LIB_PATH), "fun helper() {}")
        .expect("library file must parse");

    let fork = file_db.fork();
    let speculative_main = fork
        .process_content(PathBuf::from(MAIN_PATH), "fun main() { DummyIdentifier; }")
        .expect("speculative main file must parse");
    let shared_lib = fork
        .get_by_path(Path::new(LIB_PATH))
        .expect("unchanged file must exist in the fork");

    let actual = format!(
        "id preserved: {}\noriginal: {}\nspeculative: {}\nunchanged file shared: {}\n",
        original_main.id() == speculative_main.id(),
        original_main.source().source.trim(),
        speculative_main.source().source.trim(),
        Arc::ptr_eq(&original_lib, &shared_lib),
    );
    expect![[r#"
        id preserved: true
        original: fun main() {}
        speculative: fun main() { DummyIdentifier; }
        unchanged file shared: true
    "#]]
    .assert_eq(&actual);
}

fn check_reuse(
    first: ProjectIndex,
    mut second: ProjectIndex,
    changed_paths: &[&str],
    expected: Expect,
) {
    let changed_file_ids = changed_paths
        .iter()
        .map(|path| file_id(&second, path))
        .collect::<BTreeSet<_>>();
    let reused = second.reuse_resolved_uses_from(&first, &changed_file_ids);

    let pending = file_names(
        &second,
        second
            .files()
            .keys()
            .filter(|file_id| !second.resolved_uses().contains_key(file_id))
            .copied(),
    );
    let shared = file_names(
        &second,
        second
            .resolved_uses()
            .iter()
            .filter_map(|(&file_id, uses)| {
                Arc::ptr_eq(uses, first.get_resolved_uses(file_id)?).then_some(file_id)
            }),
    );

    let mut actual = String::new();
    let _ = writeln!(actual, "reused: {reused}");
    let _ = writeln!(actual, "pending: {}", pending.join(", "));
    let _ = writeln!(actual, "shared: {}", shared.join(", "));
    expected.assert_eq(&actual);
}

fn file_names(index: &ProjectIndex, file_ids: impl IntoIterator<Item = FileId>) -> Vec<String> {
    let mut names = file_ids
        .into_iter()
        .map(|file_id| {
            index.files()[&file_id]
                .path
                .file_name()
                .expect("test file must have a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn file_id(index: &ProjectIndex, path: &str) -> FileId {
    index
        .get_file_by_path(Path::new(path))
        .expect("test file must be indexed")
}

struct Fixture {
    file_db: FileDb,
    provider: MemoryProvider,
}

impl Fixture {
    fn new() -> Self {
        let mut fixture = Self {
            file_db: FileDb::new(PathBuf::from("/__stdlib__"), None),
            provider: MemoryProvider::default(),
        };
        fixture.set(LIB_PATH, "fun helper(): int { return 1; }");
        fixture.set(ALT_PATH, "fun alternative(): int { return 2; }");
        fixture.set(
            MAIN_PATH,
            r#"
                import "lib"

                fun main(): int {
                    return helper();
                }
            "#,
        );
        fixture
    }

    fn set(&mut self, path: impl AsRef<Path>, source: &str) {
        let path = path.as_ref().to_path_buf();
        self.file_db
            .process_content(path.clone(), source)
            .expect("test source must parse");
        self.provider.files.insert(path, Arc::from(source));
    }

    fn project(&self) -> ProjectIndex {
        ProjectIndex::builder(&self.file_db, PathBuf::from(MAIN_PATH))
            .with_additional_roots([PathBuf::from(LIB_PATH), PathBuf::from(ALT_PATH)])
            .build_with_provider(&self.provider)
            .expect("test project must build")
    }

    fn resolved_project(&self) -> ProjectIndex {
        let mut project = self.project();
        resolve(&self.file_db, &mut project);
        project
    }
}

#[derive(Default)]
struct MemoryProvider {
    files: BTreeMap<PathBuf, Arc<str>>,
}

impl ProjectSourceProvider for MemoryProvider {
    fn canonicalize(&self, path: &Path) -> anyhow::Result<PathBuf> {
        Ok(path.to_path_buf())
    }

    fn source(&self, path: &Path) -> anyhow::Result<Option<ProjectSource>> {
        Ok(self.files.get(path).cloned().map(ProjectSource::Text))
    }
}
