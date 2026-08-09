use expect_test::expect;
use std::path::PathBuf;
use tolk_analysis::AnalysisDb;
use tolk_resolver::{FileDb, ProjectIndex, resolve};
use tolk_ty::{TypeDb, TypeInterner, WorkspaceBodyTypes, infer};

#[test]
fn reports_access_flags_for_each_usage() {
    let directory = tempfile::tempdir().expect("temporary directory must be created");
    let path = directory.path().join("main.tolk");
    let source = r"
struct Counter {
    value: int
}

fun Counter.increment(mutate self) {
    self.value += 1;
}

fun main() {
    var counter = Counter { value: 0 };
    counter.increment();
    val copy = counter;
    counter = Counter { value: 1 };
}
";
    std::fs::write(&path, source).expect("test source must be written");

    let stdlib_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolk-compiler/assets/tolk-stdlib");
    let file_db = FileDb::new(stdlib_path.clone(), None);
    let mut project_index = ProjectIndex::builder(&file_db, path.clone())
        .with_stdlib(stdlib_path)
        .build()
        .expect("test project must build");
    resolve(&file_db, &mut project_index);

    let file_id = project_index
        .files()
        .values()
        .find(|index| index.path.file_name() == path.file_name())
        .map(|index| index.id)
        .expect("root file must be indexed");
    let file = file_db
        .get_by_id(file_id)
        .expect("root source must be present");
    let mut body_types = WorkspaceBodyTypes::default();
    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, &file_db, &project_index);
    for declaration in file.source().top_levels() {
        let Some(symbol) = file.find_declaration(&declaration) else {
            continue;
        };
        body_types.entry(file_id).or_default().insert(
            symbol.id,
            infer(&mut type_db, file_id, symbol.id, &declaration),
        );
    }

    let facts = AnalysisDb::new()
        .use_facts(&file_db, &project_index, &body_types, file_id)
        .expect("usage facts must be available");
    let mut usages = facts.per_usage.iter().collect::<Vec<_>>();
    usages.sort_unstable_by_key(|(span, _)| span.start());
    let usages = usages
        .into_iter()
        .map(|(span, flags)| {
            format!(
                "{} [{}..{}]: {flags:?}",
                &source[span.start()..span.end()],
                span.start(),
                span.end(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    expect![[r"
        int [29..32]: UseFlags(READ)
        Counter [40..47]: UseFlags(READ)
        self [77..81]: UseFlags(READ | WRITE)
        value [82..87]: UseFlags(READ | WRITE)
        Counter [128..135]: UseFlags(READ)
        value [138..143]: UseFlags(READ)
        counter [154..161]: UseFlags(READ | WRITE | MUTATE)
        increment [162..171]: UseFlags(READ | WRITE | MUTATE)
        counter [190..197]: UseFlags(READ)
        counter [203..210]: UseFlags(WRITE)
        Counter [213..220]: UseFlags(READ)
        value [223..228]: UseFlags(READ)"]]
    .assert_eq(&usages);
}
