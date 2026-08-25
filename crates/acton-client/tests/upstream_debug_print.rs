#[allow(unreachable_pub, clippy::significant_drop_tightening)]
mod support;

#[acton_client::contract(abi = "tests/fixtures/upstream/debug-print-demos.abi.json")]
mod generated {}

use acton_client::{BigInt, ContractProvider, DynamicAbi, SendOptions, Tuple};
use expect_test::expect;
use serde_json::Value;
use support::{TvmContractProvider, TvmSender};

const ABI_JSON: &str = include_str!("fixtures/upstream/debug-print-demos.abi.json");

async fn call_and_print(get_method_name: &str) -> String {
    let fixture = serde_json::from_str::<Value>(ABI_JSON).expect("upstream ABI JSON must parse");
    let get_method = fixture["get_methods"]
        .as_array()
        .expect("upstream ABI must contain get methods")
        .iter()
        .find(|method| method["name"].as_str() == Some(get_method_name))
        .unwrap_or_else(|| panic!("get method '{get_method_name}' not found in ABI"));
    let method_id = get_method["tvm_method_id"]
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .expect("upstream getter method id must fit i32");
    let return_ty_idx = get_method["return_ty_idx"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .expect("upstream getter return type index must fit usize");

    let contract = generated::DebugPrintDemos::from_storage(&generated::DebugStorage {
        dummy: BigInt::from(0),
    })
    .expect("upstream deployment init must build");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("TVM transaction provider must initialize");
    let contract = contract.with_provider(provider);
    let deployer = TvmSender::new("deployer", 0x11);
    contract
        .send_deploy(
            &deployer,
            BigInt::from(50_000_000_u64),
            SendOptions::default(),
        )
        .await
        .expect("upstream deployment message must execute");
    let stack = contract
        .provider()
        .run_get_method(contract.address(), method_id, Tuple::empty())
        .await
        .expect("upstream getter must execute in TVM");

    DynamicAbi::from_json(ABI_JSON)
        .expect("upstream ABI must parse")
        .debug_print_from_stack(stack, return_ty_idx)
        .expect("upstream getter result must print")
}

// Upstream title: primitives
#[tokio::test]
async fn primitives() {
    expect!("(42, 255, true, 1000000000)").assert_eq(&call_and_print("primitives").await);
}

// Upstream title: strings
#[tokio::test]
async fn strings() {
    expect!(r#"("hello", "", "int32")"#).assert_eq(&call_and_print("justStrings").await);
}

// Upstream title: structures1
#[tokio::test]
async fn structures1() {
    expect!("(Point { x: -10, y: 20 }, Empty {})").assert_eq(&call_and_print("structures1").await);
}

// Upstream title: structures2
#[tokio::test]
async fn structures2() {
    expect!("Nested { p: Point { x: 5, y: 6 }, w: Wrapper { item: 99 } }")
        .assert_eq(&call_and_print("structures2").await);
}

// Upstream title: enum values
#[tokio::test]
async fn enum_values() {
    expect!("(Color.Red, Color.Blue, Sign.Negative)")
        .assert_eq(&call_and_print("enumValues").await);
}

// Upstream title: simple nullables
#[tokio::test]
async fn simple_nullables() {
    expect!("(null, 42, null, true)").assert_eq(&call_and_print("nullables1").await);
}

// Upstream title: simple arrays
#[tokio::test]
async fn simple_arrays() {
    expect!("([1, 2, 3], [Point { x: 10, y: 20 }, Point { x: 30, y: 40 }], [])")
        .assert_eq(&call_and_print("arrays1").await);
}

// Upstream title: shaped tuple
#[tokio::test]
async fn shaped_tuple() {
    expect!("[10, Wrapper { item: false }, Point { x: 1, y: 2 }]")
        .assert_eq(&call_and_print("shapedTuple1").await);
}

// Upstream title: union: int variant
#[tokio::test]
async fn union_int_variant() {
    expect!("#int 42").assert_eq(&call_and_print("union1").await);
}

// Upstream title: union: bool and tensor
#[tokio::test]
async fn union_bool_and_tensor() {
    expect!("(#bool true, #tensor (8, 16))").assert_eq(&call_and_print("union2").await);
}

// Upstream title: union with null
#[tokio::test]
async fn union_with_null() {
    expect!("(Point { x: 7, y: 8 }, null)").assert_eq(&call_and_print("unionNull").await);
}

// Upstream title: union with null in shape
#[tokio::test]
async fn union_with_null_in_shape() {
    expect!("[Point { x: 7, y: 8 }, null]").assert_eq(&call_and_print("unionNullInShape").await);
}

// Upstream title: cellRefs
#[tokio::test]
async fn cell_refs() {
    expect!("(ref{8}, ref{Wrapper { item: #int16 10 }})")
        .assert_eq(&call_and_print("cellRefs").await);
}

// Upstream title: cellOfCells
#[tokio::test]
async fn cell_of_cells() {
    expect!("WithCells { r1: ref{5}, r2: null, r3: ref{ref{true}}, r4: [ref{addr_none}, ref{EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c}] }")
        .assert_eq(&call_and_print("cellOfCells").await);
}

// Upstream title: maps
#[tokio::test]
async fn maps() {
    expect!("(map{1: 100, 2: null}, map{})").assert_eq(&call_and_print("maps").await);
}

// Upstream title: nullable maps
#[tokio::test]
async fn nullable_maps() {
    expect!("(map{}, null)").assert_eq(&call_and_print("nullableMaps").await);
}

// Upstream title: lisp list
#[tokio::test]
async fn lisp_list() {
    expect!("[10, 20, 30]").assert_eq(&call_and_print("lispList1").await);
}

// Upstream title: address
#[tokio::test]
async fn address() {
    expect!("EQBSeWTVXPputzH0v8B-nQJQmAl--FBVGehTmGJ5vYQA2Hyk")
        .assert_eq(&call_and_print("address1").await);
}

// Upstream title: nullable address
#[tokio::test]
async fn nullable_address() {
    expect!("(null, EQBSeWTVXPputzH0v8B-nQJQmAl--FBVGehTmGJ5vYQA2Hyk)")
        .assert_eq(&call_and_print("addressOpt").await);
}

// Upstream title: Wrapper<Point>
#[tokio::test]
async fn wrapper_point() {
    expect!("Wrapper { item: Point { x: -1, y: 1 } }")
        .assert_eq(&call_and_print("wrapperPoint").await);
}

// Upstream title: void return
#[tokio::test]
async fn void_return() {
    expect!("(void)").assert_eq(&call_and_print("voidReturn").await);
}

// Upstream title: wide nullables
#[tokio::test]
async fn wide_nullables() {
    expect!("(Point { x: 3, y: 4 }, null, (3, 4), null)")
        .assert_eq(&call_and_print("wideNullables").await);
}

// Upstream title: combo1
#[tokio::test]
async fn combo1() {
    expect!(r#"(777, Point { x: -100, y: 200 }, Color.Green, ["test"])"#)
        .assert_eq(&call_and_print("combo1").await);
}

// Upstream title: weirdUnion1
#[tokio::test]
async fn weird_union1() {
    expect!("WithWeirdUnions { u1: Point { x: 10, y: 20 }, u2: #tensor (), u3: #() () }")
        .assert_eq(&call_and_print("weirdUnion1").await);
}

// Upstream title: weirdUnion2
#[tokio::test]
async fn weird_union2() {
    expect!("WithWeirdUnions { u1: #tensor ((), Point { x: 10, y: 70 }), u2: #int 3, u3: null }")
        .assert_eq(&call_and_print("weirdUnion2").await);
}

// Upstream title: raw cell
#[tokio::test]
async fn raw_cell() {
    expect!("cell{x{}}").assert_eq(&call_and_print("rawCell").await);
}

// Upstream title: raw slices
#[tokio::test]
async fn raw_slices() {
    expect!("(builder{x{}\n x{61626364}}, slice{x{6162}})")
        .assert_eq(&call_and_print("rawSlices").await);
}

// Upstream title: array of unknown
#[tokio::test]
async fn array_of_unknown() {
    expect!("[1, cell{x{616261}}, slice{x{6162}}, (10, 20), null]")
        .assert_eq(&call_and_print("unknowns").await);
}
