#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use std::fmt::Write as _;
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Location, Position, WorkspaceConfig,
};

fn case_tolk_definition(
    uri: &str,
    source: &str,
    configure: impl FnOnce(&mut LanguageService),
    expect: Expect,
) {
    let marked = MarkedSource::parse(source);
    let carets = marked
        .markers()
        .iter()
        .filter(|marker| marker.name == "caret" || marker.name.starts_with("caret:"))
        .collect::<Vec<_>>();
    assert!(
        !carets.is_empty(),
        "Tolk definition test must contain a caret marker"
    );
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    configure(&mut service);
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let mut rendered = String::new();
    for caret in carets {
        let locations = service
            .definition(&uri, caret.position)
            .expect("definition request should succeed");
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&render_definition(caret.position, &locations));
    }
    expect.assert_eq(&rendered);
}

#[test]
fn resolves_function_in_same_file() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            fun helper(): int { return 1; }
            fun main(): int { return <caret>helper(); }
        ",
        |_| {},
        expect![[r"
            1:25 -> file:///fixture/main.tolk 0:4 resolved"]],
    );
}

#[test]
fn accepts_file_uri_with_authority() {
    case_tolk_definition(
        "file://fixture/main.tolk",
        r"
            fun helper(): int { return 1; }
            fun main(): int { return <caret>helper(); }
        ",
        |_| {},
        expect![[r"
            1:25 -> file://fixture/main.tolk 0:4 resolved"]],
    );
}

#[test]
fn accepts_plain_virtual_uri() {
    case_tolk_definition(
        "workspace/main.tolk",
        r"
            fun helper(): int { return 1; }
            fun main(): int { return <caret>helper(); }
        ",
        |_| {},
        expect![[r"
            1:25 -> workspace/main.tolk 0:4 resolved"]],
    );
}

#[test]
fn resolves_imported_function() {
    case_tolk_definition(
        "acton://fixture/main.tolk",
        r#"
            import "lib"
            fun main(): int { return <caret>helper(); }
        "#,
        |service| {
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "acton://fixture/lib.tolk",
                    "fun helper(): int { return 1; }\n",
                )
                .expect("provider file should be added");
        },
        expect![[r"
            1:25 -> acton://fixture/lib.tolk 0:4 resolved"]],
    );
}

#[test]
fn removes_provider_file_from_workspace() {
    let main_uri = DocumentUri::from("acton://fixture/main.tolk");
    let lib_uri = DocumentUri::from("acton://fixture/lib.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(
            LANGUAGE_ID,
            lib_uri.clone(),
            "fun helper(): int { return 1; }\n",
        )
        .expect("provider file should be added");
    service
        .open_document(
            main_uri.clone(),
            LANGUAGE_ID,
            1,
            "import \"lib\"\nfun main(): int { return helper(); }\n",
        )
        .expect("main file should open");

    let locations = service
        .definition(&main_uri, Position::new(1, 25))
        .expect("definition should resolve before removal");
    assert_eq!(locations.len(), 1);

    service
        .remove_source_file(LANGUAGE_ID, &lib_uri)
        .expect("provider file should be removed");
    let locations = service
        .definition(&main_uri, Position::new(1, 25))
        .expect("definition should not fail after removal");
    assert!(locations.is_empty());
}

#[test]
fn resolves_imported_function_through_acton_toml_mapping() {
    case_tolk_definition(
        "file:///workspace/main.tolk",
        r#"
            import "@lib/helper"
            fun main(): int { return <caret>helper(); }
        "#,
        |service| {
            service
                .set_workspace_config(
                    LANGUAGE_ID,
                    WorkspaceConfig::new(
                        "file:///workspace",
                        Some(DocumentUri::from("file:///workspace/Acton.toml")),
                        r#"
                            [package]
                            name = "fixture"
                            version = "0.1.0"

                            [import-mappings]
                            lib = "./src/lib"
                        "#,
                    ),
                )
                .expect("workspace config should be applied");
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "file:///workspace/src/lib/helper.tolk",
                    "fun helper(): int { return 1; }\n",
                )
                .expect("provider file should be added");
        },
        expect![[r"
            1:25 -> file:///workspace/src/lib/helper.tolk 0:4 resolved"]],
    );
}

#[test]
fn resolves_stdlib_import_to_external_root() {
    case_tolk_definition(
        "file:///workspace/main.tolk",
        r#"
            import "@stdlib/common"
            fun main(): int { return <caret>stdlibHelper(); }
        "#,
        |service| {
            service
                .set_workspace_config(
                    LANGUAGE_ID,
                    WorkspaceConfig::new("file:///workspace", None, "")
                        .with_tolk_stdlib_root_uri("file:///workspace/.acton/tolk-stdlib"),
                )
                .expect("workspace config should be applied");
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "file:///workspace/.acton/tolk-stdlib/common.tolk",
                    "fun stdlibHelper(): int { return 1; }\n",
                )
                .expect("provider stdlib file should be added");
        },
        expect![[r"
            1:25 -> file:///workspace/.acton/tolk-stdlib/common.tolk 0:4 resolved"]],
    );
}

#[test]
fn resolves_local_variable() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            fun main(): int {
                var value = 1;
                return <caret>value;
            }
        ",
        |_| {},
        expect![[r"
            2:11 -> file:///fixture/main.tolk 1:8 resolved"]],
    );
}

#[test]
fn resolves_instance_method_and_field_with_type_inference() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            struct Storage {
                counter: int
            }
            fun Storage.save(self) {}
            fun main() {
                var storage = Storage { counter: 1 };
                storage.<caret:method>save();
                storage.<caret:field>counter;
            }
        ",
        |_| {},
        expect![[r"
            6:12 -> file:///fixture/main.tolk 3:12 resolved
            7:12 -> file:///fixture/main.tolk 1:4 resolved"]],
    );
}

#[test]
fn resolves_stdlib_static_and_generic_methods() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            struct Storage {
                counter: int
            }
            fun main() {
                Storage.<caret:from_cell>fromCell(contract.getData());
                contract.<caret:get_data>getData();
            }
        ",
        |_| {},
        expect![[r"
            4:12 -> file:///__tolk_stdlib__/common.tolk 483:6 resolved
            5:13 -> file:///__tolk_stdlib__/common.tolk 378:13 resolved"]],
    );
}

#[test]
fn resolves_fields_in_counter_contract_body() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            tolk 1.0

            struct Storage {
                id: uint32
                counter: uint32
            }

            fun Storage.load() {
                return Storage.fromCell(contract.getData())
            }

            fun Storage.save(mutate self) {
                contract.setData(self.toCell())
            }

            struct (0x7e8764ef) IncreaseCounter {
                queryId: uint64 = 0
                increaseBy: uint32
            }

            struct (0x3a752f06) ResetCounter {
                queryId: uint64
            }

            type AllowedMessage = IncreaseCounter | ResetCounter

            fun onInternalMessage(in: InMessage) {
                val msg = lazy AllowedMessage.fromSlice(in.body);

                match (msg) {
                    IncreaseCounter => {
                        var storage = lazy Storage.load();

                        storage.<caret:counter>counter += msg.<caret:increase_by>increaseBy;
                        storage.<caret:save>save();
                    }

                    ResetCounter => {
                        var storage = lazy Storage.load();

                        storage.counter = 0;
                        storage.save();
                    }

                    else => {
                        assert (in.body.isEmpty()) throw 0xFFFF
                    }
                }
            }
        ",
        |_| {},
        expect![[r"
            33:20 -> file:///fixture/main.tolk 4:4 resolved
            33:35 -> file:///fixture/main.tolk 17:4 resolved
            34:20 -> file:///fixture/main.tolk 11:12 resolved"]],
    );
}

#[test]
fn unresolved_reference_is_rendered() {
    case_tolk_definition(
        "file:///fixture/main.tolk",
        r"
            fun main(): int { return <caret>missing(); }
        ",
        |_| {},
        expect![[r"
            0:25 unresolved"]],
    );
}

#[test]
fn open_document_overrides_provider_file() {
    let main = MarkedSource::parse(
        r#"
            import "lib"
            fun main(): int { return <caret>helper(); }
        "#,
    );
    let main_uri = DocumentUri::from("acton://fixture/main.tolk");
    let lib_uri = DocumentUri::from("acton://fixture/lib.tolk");

    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(
            LANGUAGE_ID,
            lib_uri.clone(),
            "fun helper(): int { return 1; }\n",
        )
        .expect("provider file should be added");
    service
        .open_document(main_uri.clone(), LANGUAGE_ID, 1, main.source().to_owned())
        .expect("main document should open");
    service
        .open_document(
            lib_uri,
            LANGUAGE_ID,
            1,
            "\nfun helper(): int { return 2; }\n",
        )
        .expect("open lib document should override provider file");

    let caret = main.marker("caret");
    let locations = service
        .definition(&main_uri, caret.position)
        .expect("definition request should succeed");
    expect![[r"
        1:25 -> acton://fixture/lib.tolk 1:4 resolved"]]
    .assert_eq(&render_definition(caret.position, &locations));
}

fn render_definition(caret_position: Position, locations: &[Location]) -> String {
    if locations.is_empty() {
        return format!("{} unresolved", format_position(caret_position));
    }

    let mut locations = locations.to_vec();
    locations.sort_by(|left, right| {
        (
            left.uri.as_str(),
            left.range.start.line,
            left.range.start.character,
        )
            .cmp(&(
                right.uri.as_str(),
                right.range.start.line,
                right.range.start.character,
            ))
    });
    locations.dedup_by(|left, right| left.uri == right.uri && left.range == right.range);

    let mut output = String::new();
    for location in locations {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{} -> {} {} resolved",
            format_position(caret_position),
            location.uri,
            format_position(location.range.start)
        );
    }
    output
}

fn format_position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}
