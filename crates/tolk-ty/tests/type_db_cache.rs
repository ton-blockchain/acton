#![cfg(test)]

use expect_test::expect;
use std::fs;
use std::path::PathBuf;
use tolk_resolver::{FileDb, ProjectIndex, SymbolId, SymbolKind, resolve};
use tolk_ty::{TyData, TypeDb, TypeInterner, infer};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn method_receiver_loading_preserves_inferred_auto_return_type() -> TestResult {
    let temp_dir = tempfile::tempdir()?;
    let root_path = temp_dir.path().join("main.tolk");
    fs::write(
        &root_path,
        r"
            struct Foo {}

            fun Foo.value(self) {
                return 1;
            }
        ",
    )?;
    let root_path = dunce::canonicalize(root_path)?;

    let stdlib_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolk-compiler/assets/tolk-stdlib");
    let file_db = FileDb::new(stdlib_path.clone(), None);
    let common_tolk = dunce::canonicalize(stdlib_path)?.join("common.tolk");
    if common_tolk.exists() {
        file_db.process(&common_tolk)?;
    }

    let mut project_index = ProjectIndex::builder(&file_db, root_path.clone())
        .with_stdlib(file_db.stdlib_path().to_owned())
        .build()?;
    resolve(&file_db, &mut project_index);

    let file_info = file_db
        .get_by_path(&root_path)
        .expect("root source file should be processed");
    let method = file_info
        .index()
        .decls
        .iter()
        .find(|symbol| matches!(symbol.kind, SymbolKind::Method { .. }))
        .expect("fixture should declare a method");
    let method_decl = file_info
        .find_syntax_declaration(method.id)
        .expect("method syntax should be available");

    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, &file_db, &project_index);
    infer(&mut type_db, file_info.id(), method.id, &method_decl);

    let inferred_before = type_db
        .top_level_types
        .get(&method.id)
        .map(|ty| type_db.intrn.display(*ty).to_string());
    type_db.ensure_method_receivers_loaded();
    let inferred_after = type_db
        .top_level_types
        .get(&method.id)
        .map(|ty| type_db.intrn.display(*ty).to_string());

    assert_eq!(inferred_before.as_deref(), Some("(Foo) -> int"));
    assert_eq!(inferred_after, inferred_before);

    Ok(())
}

#[test]
fn incremental_cache_uses_updated_type_parameter_defaults() -> TestResult {
    let temp_dir = tempfile::tempdir()?;
    let root_path = temp_dir.path().join("main.tolk");
    fs::write(
        &root_path,
        "fun identity<T = int>(value: T): T { return value; }",
    )?;
    let root_path = dunce::canonicalize(root_path)?;
    let stdlib_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolk-compiler/assets/tolk-stdlib");
    let file_db = FileDb::new(stdlib_path, None);

    let mut project_index = ProjectIndex::builder(&file_db, root_path.clone()).build()?;
    resolve(&file_db, &mut project_index);
    let file_id = file_db
        .get_by_path(&root_path)
        .expect("root source file should be processed")
        .id();
    let symbol_id = project_index.global_symbols()["identity"][0];
    let mut interner = TypeInterner::new();
    let type_db = TypeDb::new(&mut interner, &file_db, &project_index);
    let before = type_parameter_default(&type_db, symbol_id);
    let cache = type_db.into_cache();

    let updated_source = "fun identity<T = slice>(value: T): T { return value; }";
    fs::write(&root_path, updated_source)?;
    file_db.process_content(root_path.clone(), updated_source)?;
    let mut project_index = ProjectIndex::builder(&file_db, root_path).build()?;
    resolve(&file_db, &mut project_index);
    let symbol_id = project_index.global_symbols()["identity"][0];
    let type_db = TypeDb::new_with_cache(&mut interner, &file_db, &project_index, cache, [file_id]);
    let after = type_parameter_default(&type_db, symbol_id);

    expect![[r"
        before: int
        after: slice"]]
    .assert_eq(&format!("before: {before}\nafter: {after}"));

    Ok(())
}

fn type_parameter_default(type_db: &TypeDb<'_>, symbol_id: SymbolId) -> String {
    let function_ty = type_db.top_level_types[&symbol_id];
    let TyData::Func { params, .. } = type_db.intrn.data(function_ty) else {
        panic!("fixture should produce a function type");
    };
    let TyData::TypeParameter {
        default_type: Some(default_type),
        ..
    } = type_db.intrn.data(params[0])
    else {
        panic!("fixture should produce a type parameter with a default");
    };
    type_db.intrn.format(*default_type)
}
