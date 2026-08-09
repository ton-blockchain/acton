use expect_test::expect;
use std::path::PathBuf;
use tolk_resolver::file_index::FileSource;
use tolk_resolver::{FileIndex, Symbol, SymbolKind};

#[test]
fn indexes_declaration_and_contract_documentation() {
    let source = r"
/// Function docs.
fun answer(): int { return 42; }

/// Struct docs.
struct State {
    /// Count docs.
    count: int
    owner: address // Owner docs.
}

enum Error {
    /// Unauthorized docs.
    Unauthorized = 401
}

/// Contract docs.
contract Counter {
    /// Storage docs.
    storage: State
}
";
    let parsed = tolk_syntax::parse(source).expect("fixture should parse");
    let index = FileIndex::build(
        source,
        0,
        PathBuf::from("/fixture/main.tolk"),
        &parsed,
        FileSource::Workspace,
    );

    let mut actual = index
        .decls
        .iter()
        .flat_map(symbol_documentation)
        .collect::<Vec<_>>();
    let contract = index.contract.expect("contract should be indexed");
    actual.push(format!("contract {}: {}", contract.name, contract.doc));
    actual.extend(
        contract
            .fields
            .iter()
            .map(|field| format!("contract field {}: {}", field.name, field.doc)),
    );

    expect![[r"
        answer: Function docs.
        State: Struct docs.
        State.count: Count docs.
        State.owner: Owner docs.
        Error: 
        Error.Unauthorized: Unauthorized docs.
        contract Counter: Contract docs.
        contract field storage: Storage docs."]]
    .assert_eq(&actual.join("\n"));
}

fn symbol_documentation(symbol: &Symbol) -> Vec<String> {
    let mut result = vec![format!("{}: {}", symbol.fqn, symbol.doc)];
    match &symbol.kind {
        SymbolKind::Struct { fields, .. } => {
            result.extend(fields.iter().flat_map(symbol_documentation));
        }
        SymbolKind::Enum { members } => {
            result.extend(members.iter().flat_map(symbol_documentation));
        }
        _ => {}
    }
    result
}
