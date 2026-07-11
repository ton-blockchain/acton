use expect_test::{Expect, expect};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tolk_resolver::{FileDb, FileId, ProjectIndex, ProjectSource, ProjectSourceProvider, resolve};

const MAIN_PATH: &str = "/workspace/main.tolk";
const LIB_PATH: &str = "/workspace/lib.tolk";
const ALT_PATH: &str = "/workspace/alt.tolk";

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

    fn set(&mut self, path: &str, source: &str) {
        self.file_db
            .process_content(PathBuf::from(path), source)
            .expect("test source must parse");
        self.provider
            .files
            .insert(PathBuf::from(path), Arc::from(source));
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
