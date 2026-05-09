use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const IO_IMPORTS: &str = r#"
import "../../lib/io"
import "../contracts/ar_shared_types"
"#;

const SHARED_TYPES: &str = r"
struct Point {
    x: int,
    y: int,
}

struct Frame {
    name: string,
    point: Point,
    pair: (int, int),
    numbers: [int, int, int],
}
";

const FORMATTER_TYPES_CONTRACT: &str = r#"
import "ar_shared_types"

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun sampleFrame(): Frame {
    return Frame {
        name: "sample",
        point: Point { x: 1, y: 2 },
        pair: (3, 4),
        numbers: [5, 6, 7],
    };
}
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{IO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/ar_shared_types", SHARED_TYPES)
        .contract("ar_formatter_types", FORMATTER_TYPES_CONTRACT)
        .test_file("io_nested_formatter", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn println_formats_nested_tuple_and_struct_values() {
    run_success_case(
        "ar-stdlib-println-nested-tuple-and-struct-values",
        r#"
get fun `test ar stdlib println nested tuple and struct values`() {
    val frame = Frame {
        name: "root",
        point: Point { x: 7, y: 9 },
        pair: (11, 13),
        numbers: [17, 19, 23],
    };

    val nested = (
        frame,
        ([29, 31], Point { x: 37, y: 41 }),
    );

    println(nested);
}
"#,
        "integration/snapshots/test-runner/println_formats_nested_tuple_and_struct_values/println_formats_nested_tuple_and_struct_values.stdout.txt",
    );
}

#[test]
fn println1_formats_nested_tuple_and_struct_values_via_placeholder_pipeline() {
    run_success_case(
        "ar-stdlib-println1-nested-tuple-and-struct-values",
        r#"
get fun `test ar stdlib println1 nested tuple and struct values`() {
    val frame = Frame {
        name: "root",
        point: Point { x: 7, y: 9 },
        pair: (11, 13),
        numbers: [17, 19, 23],
    };

    val nested = (
        frame,
        ([29, 31], Point { x: 37, y: 41 }),
    );

    println("nested={}", nested);
}
"#,
        "integration/snapshots/test-runner/println_formats_nested_tuple_and_struct_values/println1_formats_nested_tuple_and_struct_values_via_placeholder_pipeline.stdout.txt",
    );
}

#[test]
fn println_formats_struct_fields_with_abi_client_type() {
    run_success_case(
        "ar-stdlib-println-abi-client-type-fields",
        r#"
struct (0b0) PrintablePayloadInline {
    value: RemainingBitsAndRefs
}

struct (0b1) PrintablePayloadInRef {
    value: Cell<RemainingBitsAndRefs>
}

type PrintablePayload = PrintablePayloadInline | PrintablePayloadInRef

struct ClientTypedPayloadHolder {
    @abi.clientType(PrintablePayload)
    payload: RemainingBitsAndRefs
}

get fun `test ar stdlib println abi client type fields`() {
    val payload = beginCell()
        .storeUint(0xCAFE, 16)
        .endCell()
        .beginParse() as RemainingBitsAndRefs;
    val holder = ClientTypedPayloadHolder { payload };

    println(holder);
    println("holder={}", holder);
}
"#,
        "integration/snapshots/test-runner/println_formats_nested_tuple_and_struct_values/println_formats_struct_fields_with_abi_client_type.stdout.txt",
    );
}
