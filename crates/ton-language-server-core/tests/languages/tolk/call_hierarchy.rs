#[path = "../../support.rs"]
mod support;

use expect_test::{Expect, expect};
use std::fmt::Write as _;
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, DocumentUri,
    LanguageId, LanguageService, LanguageServiceConfig,
};

fn service_with_document(
    source: &str,
    configure: impl FnOnce(&mut LanguageService),
) -> (LanguageService, DocumentUri, MarkedSource) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    configure(&mut service);
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    (service, uri, marked)
}

fn prepare(
    service: &mut LanguageService,
    uri: &DocumentUri,
    marked: &MarkedSource,
) -> CallHierarchyItem {
    service
        .prepare_call_hierarchy(uri, marked.marker("caret").position)
        .expect("prepare call hierarchy should succeed")
        .expect("callable symbol should be prepared")
}

fn render_item(item: &CallHierarchyItem) -> String {
    format!(
        "{} {:?} {} {}:{}-{}:{} detail={}",
        item.name,
        item.kind,
        item.uri,
        item.selection_range.start.line,
        item.selection_range.start.character,
        item.selection_range.end.line,
        item.selection_range.end.character,
        item.detail.as_deref().unwrap_or("<none>"),
    )
}

fn render_outgoing(calls: &[CallHierarchyOutgoingCall]) -> String {
    let mut output = String::new();
    for call in calls {
        if !output.is_empty() {
            output.push('\n');
        }
        let ranges = call
            .from_ranges
            .iter()
            .map(|range| format!("{}:{}", range.start.line, range.start.character))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(output, "{} <- {ranges}", render_item(&call.to));
    }
    output
}

fn render_incoming(calls: &[CallHierarchyIncomingCall]) -> String {
    let mut output = String::new();
    for call in calls {
        if !output.is_empty() {
            output.push('\n');
        }
        let ranges = call
            .from_ranges
            .iter()
            .map(|range| format!("{}:{}", range.start.line, range.start.character))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(output, "{} -> {ranges}", render_item(&call.from));
    }
    output
}

fn check(actual: &str, expected: Expect) {
    expected.assert_eq(actual);
}

#[test]
fn prepares_function_method_and_get_method_items() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun <caret>helper(value: int): int { return value; }
        ",
        |_| {},
    );
    check(
        &render_item(&prepare(&mut service, &uri, &marked)),
        expect!["helper Function file:///workspace/main.tolk 0:4-0:10 detail=(value: int): int"],
    );

    let method = MarkedSource::parse(
        r"
            fun int.<caret>twice(): int { return self * 2; }
        ",
    );
    service
        .change_document(&uri, 2, method.source().to_owned())
        .expect("method document should update");
    check(
        &render_item(&prepare(&mut service, &uri, &method)),
        expect!["int.twice Method file:///workspace/main.tolk 0:8-0:13 detail=(): int"],
    );

    let get_method = MarkedSource::parse(
        r"
            get fun <caret>balance(): int { return 0; }
        ",
    );
    service
        .change_document(&uri, 3, get_method.source().to_owned())
        .expect("get method document should update");
    check(
        &render_item(&prepare(&mut service, &uri, &get_method)),
        expect!["get balance Event file:///workspace/main.tolk 0:8-0:15 detail=(): int"],
    );
}

#[test]
fn prepare_rejects_non_callable_symbols() {
    let (mut service, uri, marked) = service_with_document(
        r"
            const <caret>ANSWER = 42
        ",
        |_| {},
    );
    let item = service
        .prepare_call_hierarchy(&uri, marked.marker("caret").position)
        .expect("prepare call hierarchy should succeed");
    check(
        if item.is_none() { "none" } else { "item" },
        expect!["none"],
    );
}

#[test]
fn outgoing_calls_group_call_sites_and_ignore_function_values() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun leaf(): int { return 1; }
            fun <caret>caller(): int {
                leaf();
                leaf();
                val callback = leaf;
                return 0;
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect!["leaf Function file:///workspace/main.tolk 0:4-0:8 detail=(): int <- 2:4,3:4"],
    );
}

#[test]
fn outgoing_calls_resolve_instance_methods_through_inference() {
    let (mut service, uri, marked) = service_with_document(
        r"
            struct Box { value: int }
            fun Box.read(): int { return self.value; }
            fun <caret>caller(box: Box): int {
                return box.read();
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect!["Box.read Method file:///workspace/main.tolk 1:8-1:12 detail=(): int <- 3:15"],
    );
}

#[test]
fn outgoing_calls_distinguish_overloaded_methods() {
    let (mut service, uri, marked) = service_with_document(
        r"
            struct First { value: int }
            struct Second { value: int }
            fun First.read(): int { return self.value; }
            fun Second.read(): int { return self.value; }
            fun <caret>caller(first: First, second: Second): int {
                return first.read() + second.read();
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect![[r"
            First.read Method file:///workspace/main.tolk 2:10-2:14 detail=(): int <- 5:17
            Second.read Method file:///workspace/main.tolk 3:11-3:15 detail=(): int <- 5:33"]],
    );
}

#[test]
fn incoming_calls_are_grouped_by_caller_across_files() {
    let (mut service, uri, marked) = service_with_document(
        r#"
            import "lib"
            fun first(): int { return helper() + helper(); }
            fun second(): int { return <caret>helper(); }
        "#,
        |service| {
            service
                .add_source_file(
                    LANGUAGE_ID,
                    "file:///workspace/lib.tolk",
                    "fun helper(): int { return 1; }\nfun fromLib(): int { return helper(); }\n",
                )
                .expect("provider file should be added");
        },
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .incoming_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("incoming calls should succeed");
    check(
        &render_incoming(&calls),
        expect![[r"
            fromLib Function file:///workspace/lib.tolk 1:4-1:11 detail=(): int -> 1:28
            first Function file:///workspace/main.tolk 1:4-1:9 detail=(): int -> 1:26,1:37
            second Function file:///workspace/main.tolk 2:4-2:10 detail=(): int -> 2:27"]],
    );
}

#[test]
fn recursive_calls_appear_in_both_directions() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun <caret>recurse(value: int): int {
                return value <= 0 ? 0 : recurse(value - 1);
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let language_id = LanguageId::from(LANGUAGE_ID);
    let incoming = service
        .incoming_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("incoming calls should succeed");
    let outgoing = service
        .outgoing_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("outgoing calls should succeed");
    check(
        &format!(
            "incoming:\n{}\noutgoing:\n{}",
            render_incoming(&incoming),
            render_outgoing(&outgoing)
        ),
        expect![[r"
            incoming:
            recurse Function file:///workspace/main.tolk 0:4-0:11 detail=(value: int): int -> 1:28
            outgoing:
            recurse Function file:///workspace/main.tolk 0:4-0:11 detail=(value: int): int <- 1:28"]],
    );
}

#[test]
fn prepare_on_call_site_returns_the_called_function() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun helper(): int { return 1; }
            fun caller(): int { return <caret>helper(); }
        ",
        |_| {},
    );
    check(
        &render_item(&prepare(&mut service, &uri, &marked)),
        expect!["helper Function file:///workspace/main.tolk 0:4-0:10 detail=(): int"],
    );
}

#[test]
fn outgoing_calls_recognize_generic_instantiations() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun identity<T>(value: T): T { return value; }
            fun <caret>caller(): int { return identity<int>(42); }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect![
            "identity Function file:///workspace/main.tolk 0:4-0:12 detail=<T>(value: T): T <- 1:27"
        ],
    );
}

#[test]
fn outgoing_calls_include_calls_nested_in_lambdas() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun helper(): int { return 1; }
            fun <caret>caller() {
                val callback = fun(): int { return helper(); };
                callback();
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect!["helper Function file:///workspace/main.tolk 0:4-0:10 detail=(): int <- 2:39"],
    );
}

#[test]
fn incoming_calls_ignore_non_call_references() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun <caret>target(): int { return 1; }
            fun caller(): int {
                val callback = target;
                return target();
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .incoming_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("incoming calls should succeed");
    check(
        &render_incoming(&calls),
        expect!["caller Function file:///workspace/main.tolk 1:4-1:10 detail=(): int -> 3:11"],
    );
}

#[test]
fn leaf_and_unresolved_calls_produce_empty_results() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun <caret>leaf(): int { return 1; }
            fun unresolved(): int { return missing(); }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let language_id = LanguageId::from(LANGUAGE_ID);
    let incoming = service
        .incoming_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("incoming calls should succeed");
    let outgoing = service
        .outgoing_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("outgoing calls should succeed");
    check(
        &format!(
            "leaf incoming={} outgoing={}",
            incoming.len(),
            outgoing.len()
        ),
        expect!["leaf incoming=0 outgoing=0"],
    );

    let unresolved = MarkedSource::parse(
        r"
            fun leaf(): int { return 1; }
            fun <caret>unresolved(): int { return missing(); }
        ",
    );
    let unresolved_item = prepare(&mut service, &uri, &unresolved);
    let unresolved_outgoing = service
        .outgoing_calls(
            &language_id,
            &unresolved_item.uri,
            unresolved_item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &format!("unresolved outgoing={}", unresolved_outgoing.len()),
        expect!["unresolved outgoing=0"],
    );
}

#[test]
fn incoming_calls_distinguish_overloaded_methods() {
    let (mut service, uri, marked) = service_with_document(
        r"
            struct First { value: int }
            struct Second { value: int }
            fun First.<caret>read(): int { return self.value; }
            fun Second.read(): int { return self.value; }
            fun caller(first: First, second: Second): int {
                return first.read() + second.read();
            }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .incoming_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("incoming calls should succeed");
    check(
        &render_incoming(&calls),
        expect![
            "caller Function file:///workspace/main.tolk 4:4-4:10 detail=(first: First, second: Second): int -> 5:17"
        ],
    );
}

#[test]
fn outgoing_calls_follow_incremental_document_updates() {
    let (mut service, uri, marked) = service_with_document(
        r"
            fun first(): int { return 1; }
            fun second(): int { return 2; }
            fun <caret>caller(): int { return first(); }
        ",
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let language_id = LanguageId::from(LANGUAGE_ID);
    let before = service
        .outgoing_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("outgoing calls should succeed");

    service
        .change_document(
            &uri,
            2,
            "fun first(): int { return 1; }\nfun second(): int { return 2; }\nfun caller(): int { return second(); }\n",
        )
        .expect("document should update");
    let after = service
        .outgoing_calls(&language_id, &item.uri, item.selection_range.start)
        .expect("outgoing calls should succeed");
    check(
        &format!(
            "before:\n{}\nafter:\n{}",
            render_outgoing(&before),
            render_outgoing(&after)
        ),
        expect![[r"
            before:
            first Function file:///workspace/main.tolk 0:4-0:9 detail=(): int <- 2:27
            after:
            second Function file:///workspace/main.tolk 1:4-1:10 detail=(): int <- 2:27"]],
    );
}

#[test]
fn call_ranges_use_utf16_columns() {
    let (mut service, uri, marked) = service_with_document(
        r#"
            fun helper(): int { return 1; }
            fun <caret>caller(): int { val label = "😀"; return helper(); }
        "#,
        |_| {},
    );
    let item = prepare(&mut service, &uri, &marked);
    let calls = service
        .outgoing_calls(
            &LanguageId::from(LANGUAGE_ID),
            &item.uri,
            item.selection_range.start,
        )
        .expect("outgoing calls should succeed");
    check(
        &render_outgoing(&calls),
        expect!["helper Function file:///workspace/main.tolk 0:4-0:10 detail=(): int <- 1:45"],
    );
}
