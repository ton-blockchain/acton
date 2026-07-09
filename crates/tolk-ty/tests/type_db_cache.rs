#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use tolk_resolver::{FileDb, ProjectIndex, SymbolKind, resolve};
use tolk_ty::{TypeDb, TypeInterner, infer};

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
