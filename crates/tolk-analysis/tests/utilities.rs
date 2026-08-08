use expect_test::expect;
use num_bigint::BigInt;
use std::path::PathBuf;
use tolk_analysis::{
    ConstantEvaluationContext, ConstantEvaluator, ConstantValue, SerializationSize,
    compute_get_method_id, compute_struct_opcode,
};
use tolk_resolver::{FileDb, FileId, ProjectIndex, Resolved, Span, SymbolId, resolve};

struct EvaluationContext {
    file_db: FileDb,
    project_index: ProjectIndex,
}

impl EvaluationContext {
    fn new(source: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory must be created");
        let path = directory.path().join("main.tolk");
        std::fs::write(&path, source).expect("test source must be written");

        let file_db = FileDb::new(PathBuf::from("/__stdlib__"), None);
        let mut project_index = ProjectIndex::builder(&file_db, path)
            .build()
            .expect("test project must build");
        resolve(&file_db, &mut project_index);

        Self {
            file_db,
            project_index,
        }
    }

    fn symbol(&self, name: &str) -> SymbolId {
        self.project_index.global_symbols()[name][0]
    }
}

impl ConstantEvaluationContext for EvaluationContext {
    fn file_db(&self) -> &FileDb {
        &self.file_db
    }

    fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    fn resolve_at(&self, file_id: FileId, span: Span) -> Option<Resolved> {
        self.project_index
            .get_resolved_uses(file_id)?
            .find_use(span.start())
            .map(|usage| usage.resolved.clone())
    }
}

#[test]
fn evaluates_constants_and_enum_members() {
    let context = EvaluationContext::new(
        r"
            const BASE = 10;
            const VALUE = (BASE + 2) * 3;

            enum Mode {
                First = VALUE,
                Second,
            }
        ",
    );
    let mut evaluator = ConstantEvaluator::new(&context);
    let value = evaluator.evaluate_constant(context.symbol("VALUE"));
    let mut enum_values = evaluator
        .evaluate_enum_values(context.symbol("Mode"))
        .expect("enum values must be evaluated")
        .iter()
        .map(|(symbol_id, value)| {
            let symbol = context
                .project_index
                .resolve_symbol(*symbol_id)
                .expect("enum member must resolve");
            format!("{}: {}", symbol.name, value.format())
        })
        .collect::<Vec<_>>();
    enum_values.sort_unstable();

    let evaluated = format!("VALUE: {}\n{}", value.format(), enum_values.join("\n"));
    expect![[r"
        VALUE: 36 (0x24)
        First: 36 (0x24)
        Second: 37 (0x25)"]]
    .assert_eq(&evaluated);
}

#[test]
fn evaluates_gram_constants_at_the_coins_boundary() {
    let context = EvaluationContext::new(
        r#"
            const BILLION_GRAMS = grams("1000000000");
            const MAX_COINS = grams("1329227995784915872903807060.280344575");
            const MIN_COINS = grams("-1329227995784915872903807060.280344575");
            const OVERFLOW = grams("1329227995784915872903807060.280344576");
        "#,
    );
    let mut evaluator = ConstantEvaluator::new(&context);
    let evaluated = ["BILLION_GRAMS", "MAX_COINS", "MIN_COINS", "OVERFLOW"]
        .into_iter()
        .map(|name| {
            let value = evaluator.evaluate_constant(context.symbol(name));
            let value = match value {
                ConstantValue::Int(value) => value.to_string(),
                value => value.format(),
            };
            format!("{name}: {value}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    expect![[r"
        BILLION_GRAMS: 1000000000000000000
        MAX_COINS: 1329227995784915872903807060280344575
        MIN_COINS: -1329227995784915872903807060280344575
        OVERFLOW: overflow"]]
    .assert_eq(&evaluated);
}

#[test]
fn formats_constant_values() {
    let values = [
        ConstantValue::Int(BigInt::from(0)),
        ConstantValue::Int(BigInt::from(31)),
        ConstantValue::Int(BigInt::from(1_u64 << 32)),
        ConstantValue::Bool(true),
        ConstantValue::String("acton".to_owned()),
        ConstantValue::Overflow,
        ConstantValue::Unknown,
    ];
    let formatted = values
        .iter()
        .map(ConstantValue::format)
        .collect::<Vec<_>>()
        .join("\n");

    expect![[r#"
        0
        31 (0x1F)
        0x100000000
        true
        "acton"
        overflow
        unknown"#]]
    .assert_eq(&formatted);
}

#[test]
fn computes_tolk_hashes() {
    let hashes = format!(
        "seqno method id: {}\nTransfer struct opcode: 0x{:08X}",
        compute_get_method_id("seqno"),
        compute_struct_opcode("Transfer"),
    );

    expect![[r"
        seqno method id: 85143
        Transfer struct opcode: 0xB942C196"]]
    .assert_eq(&hashes);
}

#[test]
fn presents_serialization_ranges() {
    let sizes = [
        SerializationSize::exact(32),
        SerializationSize::range(1, 2, 0, 1),
        SerializationSize::range(0, 0, 1, 1),
        SerializationSize::unpredictable(),
        SerializationSize::invalid(),
    ];
    let presentations = sizes
        .iter()
        .map(|size| size.presentation())
        .collect::<Vec<_>>()
        .join("\n");

    expect![[r"
        32 bits
        1..2 bits, 0..1 refs
        0 bits, 1 refs
        0..9999 bits, 0..4 refs
        unknown or invalid size"]]
    .assert_eq(&presentations);
}
