use expect_test::expect;
use std::path::PathBuf;
use tolk_analysis::AnalysisDb;
use tolk_resolver::{FileDb, ProjectIndex, resolve};
use tolk_ty::{TypeDb, TypeInterner, WorkspaceBodyTypes, infer};

#[test]
fn reports_access_flags_for_each_usage() {
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
    .assert_eq(&snapshot_use_facts(source));
}

#[test]
fn reports_access_flags_for_generic_method_calls() {
    let source = r"
type Slice<T> = slice

@inline
fun Slice<T>.load(self): T {
    var this = self;
    return this.loadAny<T>();
}

fun Slice<T>.loadInPlace(mutate self): T {
    return self.loadAny<T>();
}

fun Slice<T>.peek(self): T {
    return T.fromSlice(self);
}

fun main(cs: slice) {
    var mutable = cs;
    mutable.loadAny<uint16>();
    var immutable = cs as Slice<uint16>;
    immutable.peek();
}
";

    expect![[r"
        slice [17..22]: UseFlags(READ)
        Slice [36..41]: UseFlags(READ)
        T [57..58]: UseFlags(READ)
        self [76..80]: UseFlags(READ)
        this [93..97]: UseFlags(READ | WRITE | MUTATE)
        loadAny [98..105]: UseFlags(READ | WRITE | MUTATE)
        T [106..107]: UseFlags(READ)
        Slice [119..124]: UseFlags(READ)
        T [154..155]: UseFlags(READ)
        self [169..173]: UseFlags(READ | WRITE | MUTATE)
        loadAny [174..181]: UseFlags(READ | WRITE | MUTATE)
        T [182..183]: UseFlags(READ)
        Slice [195..200]: UseFlags(READ)
        T [216..217]: UseFlags(READ)
        T [231..232]: UseFlags(READ)
        fromSlice [233..242]: UseFlags(READ)
        self [243..247]: UseFlags(READ)
        slice [266..271]: UseFlags(READ)
        cs [293..295]: UseFlags(READ)
        mutable [301..308]: UseFlags(READ | WRITE | MUTATE)
        loadAny [309..316]: UseFlags(READ | WRITE | MUTATE)
        uint16 [317..323]: UseFlags(READ)
        cs [348..350]: UseFlags(READ)
        Slice [354..359]: UseFlags(READ)
        uint16 [360..366]: UseFlags(READ)
        immutable [373..382]: UseFlags(READ)
        peek [383..387]: UseFlags(READ)"]]
    .assert_eq(&snapshot_use_facts(source));
}

fn snapshot_use_facts(source: &str) -> String {
    let directory = tempfile::tempdir().expect("temporary directory must be created");
    let path = directory.path().join("main.tolk");
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
    usages
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
        .join("\n")
}
