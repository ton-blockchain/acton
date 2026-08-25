#![allow(
    clippy::let_unit_value,
    clippy::missing_const_for_fn,
    clippy::needless_question_mark,
    clippy::ptr_arg,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

use std::future::Future;
use std::sync::Mutex;

use acton_client::__private::tycho_types::cell::CellBuilder;
use acton_client::__private::tycho_types::models::{AnyAddr, StdAddr};
use acton_client::{
    BitString, Cell, CellRef, ContractProvider, Dictionary, DynamicAbi, DynamicValue, OwnedSlice,
    Tuple, TupleItem,
};
use expect_test::expect;
use num_bigint::BigInt;

#[allow(clippy::significant_drop_tightening)]
mod support;

use support::TvmGetterProvider;

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-getters.abi.json")]
mod generated {}

const STR_128: &str = "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678";

#[derive(Debug, PartialEq, Eq)]
struct ObservedCall {
    method_id: i32,
    arguments: Vec<String>,
}

#[derive(Debug)]
struct RecordingProvider {
    inner: TvmGetterProvider,
    call: Mutex<Option<ObservedCall>>,
}

impl RecordingProvider {
    fn take_call(&self) -> ObservedCall {
        self.call
            .lock()
            .expect("call lock must not be poisoned")
            .take()
            .expect("getter must call the provider")
    }

    fn take_optional_call(&self) -> Option<ObservedCall> {
        self.call
            .lock()
            .expect("call lock must not be poisoned")
            .take()
    }
}

impl ContractProvider for RecordingProvider {
    type Error = String;

    fn run_get_method(
        &self,
        address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send {
        *self.call.lock().expect("call lock must not be poisoned") = Some(ObservedCall {
            method_id,
            arguments: arguments.0.iter().map(stack_item).collect(),
        });
        self.inner.run_get_method(address, method_id, arguments)
    }
}

fn stack_item(item: &TupleItem) -> String {
    match item {
        TupleItem::Null => "null".to_owned(),
        TupleItem::Int(value) => value.to_string(),
        TupleItem::Nan => "nan".to_owned(),
        TupleItem::Cont(_) => "cont".to_owned(),
        TupleItem::Cell(cell) => format!("cell:{}", cell_hex(cell)),
        TupleItem::Slice(cell) => format!("slice:{}", cell_hex(cell)),
        TupleItem::Builder(cell) => format!("builder:{}", cell_hex(cell)),
        TupleItem::Tuple(tuple) => format!(
            "tuple:[{}]",
            tuple
                .0
                .iter()
                .map(stack_item)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn cell_hex(cell: &Cell) -> String {
    format!(
        "x{{{:X}}}",
        cell.as_slice()
            .expect("cell must be readable")
            .display_data()
    )
}

fn bi(value: i64) -> BigInt {
    BigInt::from(value)
}

fn real_provider() -> RecordingProvider {
    let address = StdAddr::default();
    let code = generated::LotsOfGetters::<RecordingProvider>::code_cell()
        .expect("LotsOfGetters code BoC must decode");
    let data =
        generated::LotsOfGetters::<RecordingProvider>::storage_to_cell(&generated::StorageMe {
            id: bi(0),
            counter: bi(0),
        })
        .expect("LotsOfGetters storage must encode");
    RecordingProvider {
        inner: TvmGetterProvider::new(address, code, data),
        call: Mutex::new(None),
    }
}

fn real_contract() -> generated::LotsOfGetters<RecordingProvider> {
    let provider = real_provider();
    generated::LotsOfGetters::from_address(provider.inner.address().clone(), provider)
}

fn raw_address() -> StdAddr {
    "9:527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8"
        .parse()
        .expect("raw address must parse")
}

fn cell_uint(value: u64, bits: u16) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_uint(value, bits)
        .expect("integer must fit into test cell");
    builder.build().expect("test cell must build")
}

fn empty_cell() -> Cell {
    CellBuilder::new().build().expect("empty cell must build")
}

fn builder_uint(value: u64, bits: u16) -> CellBuilder {
    let mut builder = CellBuilder::new();
    builder
        .store_uint(value, bits)
        .expect("integer must fit into test builder");
    builder
}

fn slice_uint(value: u64, bits: u16) -> OwnedSlice {
    OwnedSlice::full(cell_uint(value, bits))
}

fn address_stack_item(address: &AnyAddr) -> TupleItem {
    let mut items = Vec::new();
    acton_client::stack::write_tlb_slice(address, &mut items)
        .expect("address must serialize to stack");
    items.pop().expect("address must produce one stack item")
}

async fn call_dynamic(
    method_name: &str,
    arguments: &[DynamicValue],
) -> (ObservedCall, DynamicValue) {
    let provider = real_provider();
    let result = DynamicAbi::from_json(generated::ABI_JSON)
        .expect("LotsOfGetters ABI must parse dynamically")
        .call_get_method(&provider, &StdAddr::default(), method_name, arguments)
        .await
        .expect("dynamic getter must succeed");
    (provider.take_call(), result)
}

async fn call_dynamic_error(
    method_name: &str,
    arguments: &[DynamicValue],
) -> (Option<ObservedCall>, String) {
    let provider = real_provider();
    let error = DynamicAbi::from_json(generated::ABI_JSON)
        .expect("LotsOfGetters ABI must parse dynamically")
        .call_get_method(&provider, &StdAddr::default(), method_name, arguments)
        .await
        .expect_err("dynamic getter must fail")
        .to_string();
    (provider.take_optional_call(), error)
}

// Upstream runs each parameterized case through the generated wrapper and its
// dynamic dispatcher. The dynamic entrypoint is connected by the shared test
// harness; these two concrete tests deliberately stay separate.
macro_rules! generated_and_dynamic_tests {
    ($generated:ident, $dynamic:ident, $case:ident, $dynamic_case:ident) => {
        #[tokio::test]
        async fn $generated() {
            $case().await;
        }

        #[tokio::test]
        async fn $dynamic() {
            $dynamic_case().await;
        }
    };
}

async fn complex_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_complex_params1(
            &bi(1),
            &bi(2),
            &(bi(3), bi(4)),
            &AnyAddr::None,
            &slice_uint(123, 16),
            &cell_uint(123, 16),
            &builder_uint(123, 16),
            &vec![TupleItem::Null],
            &"hello".to_owned(),
            &STR_128.to_owned(),
        )
        .await
        .expect("complexParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 80154,
                arguments: [
                    "1",
                    "2",
                    "3",
                    "4",
                    "slice:x{2_}",
                    "slice:x{007B}",
                    "cell:x{007B}",
                    "builder:x{007B}",
                    "tuple:[null]",
                    "cell:x{68656C6C6F}",
                    "cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}",
                ],
            },
            Wrapper {
                item: 1,
            },
        )
    "#]].assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    complex_params1_generated,
    complex_params1_dynamic,
    complex_params1_case,
    complex_params1_dynamic_case
);

async fn complex_params2_case() {
    let address = raw_address();
    let contract = real_contract();
    let result = contract
        .get_complex_params2(
            &generated::WithWrapper {
                nested_w: generated::Wrapper {
                    item: (bi(1), bi(2)),
                },
            },
            &generated::PartReply {
                f1: bi(0),
                f2: vec![TupleItem::Null, TupleItem::Null],
                f3: (
                    CellBuilder::new(),
                    generated::NestedPartReply {
                        n1: true,
                        n2: slice_uint(0x0102, 16),
                    },
                ),
            },
            &(OwnedSlice::full(empty_cell()), bi(123)),
            &(AnyAddr::Std(address),),
            &CellRef::new(bi(123)),
            &generated::Empty {},
            &(),
            &(generated::Color::blue(), generated::E0Max::max_int()),
            &generated::PackOptions {
                skip_bits_n_validation: true,
            },
        )
        .await
        .expect("complexParams2 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 67961,
                arguments: [
                    "1",
                    "2",
                    "0",
                    "tuple:[null, null]",
                    "builder:x{}",
                    "-1",
                    "slice:x{0102}",
                    "slice:x{}",
                    "123",
                    "tuple:[slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}]",
                    "cell:x{7B}",
                    "tuple:[]",
                    "tuple:[2, 115792089237316195423570985008687907853269984665640564039457584007913129639935]",
                    "-1",
                ],
            },
            (
                Std(
                    StdAddr {
                        anycast: None,
                        workchain: 9,
                        address: 527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8,
                    },
                ),
            ),
        )
    "#]].assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    complex_params2_generated,
    complex_params2_dynamic,
    complex_params2_case,
    complex_params2_dynamic_case
);

async fn complex_params3_case() {
    let null_contract = real_contract();
    let null_result = null_contract
        .get_complex_params3(
            &(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                generated::Wrapper { item: None },
                generated::Wrapper {
                    item: generated::Wrapper { item: None },
                },
                None,
                None,
                (),
            ),
            &true,
        )
        .await
        .expect("null complexParams3 must succeed");

    let value_contract = real_contract();
    let value_result = value_contract
        .get_complex_params3(
            &(
                Some(bi(8)),
                Some(bi(50_000_000)),
                Some(empty_cell()),
                Some(slice_uint(0x0102, 16)),
                Some(builder_uint(0xff, 32)),
                Some(true),
                Some(raw_address()),
                Some(vec![TupleItem::Null, TupleItem::Nan]),
                Some(BitString(slice_uint(0x0102, 16))),
                Some(CellRef::new(bi(8))),
                Some(CellRef::new(CellRef::new(bi(8)))),
                generated::Wrapper {
                    item: Some(bi(123)),
                },
                generated::Wrapper {
                    item: generated::Wrapper { item: Some(true) },
                },
                Some((bi(1), (bi(2),))),
                Some("spoon".to_owned()),
                (),
            ),
            &false,
        )
        .await
        .expect("value complexParams3 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 72024,
                arguments: [
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "-1",
                ],
            },
            (
                None,
                None,
            ),
            ObservedCall {
                method_id: 72024,
                arguments: [
                    "8",
                    "50000000",
                    "cell:x{}",
                    "slice:x{0102}",
                    "builder:x{000000FF}",
                    "-1",
                    "slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
                    "tuple:[null, nan]",
                    "slice:x{0102}",
                    "cell:x{08}",
                    "cell:x{}",
                    "123",
                    "-1",
                    "tuple:[1, tuple:[2]]",
                    "cell:x{73706F6F6E}",
                    "null",
                    "0",
                ],
            },
            (
                Some(
                    8,
                ),
                Some(
                    50000000,
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        null_contract.provider().take_call(),
        null_result,
        value_contract.provider().take_call(),
        value_result,
    ));
}

generated_and_dynamic_tests!(
    complex_params3_generated,
    complex_params3_dynamic,
    complex_params3_case,
    complex_params3_dynamic_case
);

async fn complex_params4_case() {
    let mut m1 = Dictionary::new();
    m1.insert(bi(8), bi(50_000_000));
    let mut m3 = Dictionary::new();
    m3.insert(raw_address(), generated::Color::blue());
    let contract = real_contract();
    let result = contract
        .get_complex_params4(
            &generated::WithCells {
                c1: CellRef::new(bi(1)),
                c2: None,
                c3: Some(CellRef::new(generated::WithWrapper {
                    nested_w: generated::Wrapper { item: bi(3) },
                })),
                c4: CellRef::new(generated::UnionTy90::Variant1(bi(4))),
                c5: CellRef::new(None),
                c6: CellRef::new(generated::Color::green()),
            },
            &CellRef::new(generated::PackOptions {
                skip_bits_n_validation: true,
            }),
            &m1,
            &Dictionary::new(),
            &Some(m3),
            &None,
        )
        .await
        .expect("complexParams4 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 92607,
                arguments: [
                    "cell:x{00000001}",
                    "null",
                    "cell:x{0000000000000003}",
                    "cell:x{80000000000000024_}",
                    "cell:x{4_}",
                    "cell:x{6_}",
                    "cell:x{C_}",
                    "cell:x{A02100BEBC202_}",
                    "null",
                    "cell:x{A1702549E5935573E9BADCC7D2FF01FA7409426025FBE1415467A14E6189E6F6100362}",
                    "129",
                    "null",
                    "0",
                ],
            },
            (),
        )
    "#]].assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    complex_params4_generated,
    complex_params4_dynamic,
    complex_params4_case,
    complex_params4_dynamic_case
);

async fn complex_params5_case() {
    let contract = real_contract();
    let result = contract
        .get_complex_params5(
            &generated::WrapperN::<BigInt> { item: None },
            &generated::WrapperN {
                item: Some(bi(123)),
            },
            &generated::WrapperN::<OwnedSlice> { item: None },
            &generated::WrapperN {
                item: Some(slice_uint(1, 8)),
            },
            &generated::WrapperN::<StdAddr> { item: None },
            &generated::WrapperN::<AnyAddr> { item: None },
            &generated::WrapperN {
                item: Some(AnyAddr::None),
            },
            &generated::WrapperN {
                item: Some(AnyAddr::Std(raw_address())),
            },
        )
        .await
        .expect("complexParams5 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 96670,
                arguments: [
                    "null",
                    "123",
                    "null",
                    "slice:x{01}",
                    "null",
                    "null",
                    "slice:x{2_}",
                    "slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
                ],
            },
            (),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    complex_params5_generated,
    complex_params5_dynamic,
    complex_params5_case,
    complex_params5_dynamic_case
);

fn unknown_values() -> Tuple {
    Tuple(vec![
        TupleItem::Int(bi(1)),
        TupleItem::Int(bi(-1)),
        address_stack_item(&AnyAddr::Std(raw_address())),
        address_stack_item(&AnyAddr::None),
        TupleItem::Tuple(Tuple(vec![TupleItem::Int(bi(10)), TupleItem::Int(bi(20))])),
    ])
}

async fn array_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_array_params1(
            &unknown_values(),
            &vec![bi(1), bi(2), bi(3)],
            &vec![Some(bi(1)), None, Some(bi(3))],
            &vec![
                generated::Point::create(),
                generated::Point {
                    x: bi(100),
                    y: bi(200),
                },
            ],
            &vec![(), ()],
            &vec![(bi(10), bi(20)), (bi(30), bi(40))],
            &vec!["one".to_owned(), "two".to_owned()],
        )
        .await
        .expect("arrayParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 95410,
                arguments: [
                    "tuple:[1, -1, slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}, slice:x{2_}, tuple:[10, 20]]",
                    "tuple:[1, 2, 3]",
                    "tuple:[1, null, 3]",
                    "tuple:[tuple:[10, 20], tuple:[100, 200]]",
                    "tuple:[tuple:[], tuple:[]]",
                    "tuple:[tuple:[10, 20], tuple:[30, 40]]",
                    "tuple:[cell:x{6F6E65}, cell:x{74776F}]",
                ],
            },
            (),
        )
    "#]].assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    array_params1_generated,
    array_params1_dynamic,
    array_params1_case,
    array_params1_dynamic_case
);

async fn array_params2_case() {
    let contract = real_contract();
    let result = contract
        .get_array_params2(
            &vec![],
            &vec![],
            &vec![],
            &vec![],
            &vec![],
            &vec![],
            &vec![],
        )
        .await
        .expect("arrayParams2 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 83153,
                arguments: [
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                ],
            },
            (),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    array_params2_generated,
    array_params2_dynamic,
    array_params2_case,
    array_params2_dynamic_case
);

async fn array_params3_case() {
    let contract = real_contract();
    let result = contract
        .get_array_params3(
            &vec![bi(1), bi(2), bi(3)],
            &vec![vec![bi(1), bi(2)], vec![], vec![bi(5), bi(6)]],
            &CellRef::new(vec![bi(1), bi(2), bi(3)]),
            &vec![
                CellRef::new(bi(1)),
                CellRef::new(bi(2)),
                CellRef::new(bi(3)),
            ],
            &Some(vec![bi(1), bi(2), bi(3)]),
            &Some(vec![Some(bi(1)), None, Some(bi(3))]),
            &Some(vec![
                generated::Point::create(),
                generated::Point {
                    x: bi(-100),
                    y: bi(100),
                },
            ]),
            &CellRef::new(vec![Some((bi(1), bi(2))), None, None, Some((bi(7), bi(8)))]),
        )
        .await
        .expect("arrayParams3 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 87280,
                arguments: [
                    "tuple:[1, 2, 3]",
                    "tuple:[tuple:[1, 2], tuple:[], tuple:[5, 6]]",
                    "cell:x{03C_}",
                    "tuple:[cell:x{01}, cell:x{02}, cell:x{03}]",
                    "tuple:[1, 2, 3]",
                    "tuple:[1, null, 3]",
                    "tuple:[tuple:[10, 20], tuple:[-100, 100]]",
                    "cell:x{04C_}",
                ],
            },
            (),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    array_params3_generated,
    array_params3_dynamic,
    array_params3_case,
    array_params3_dynamic_case
);

async fn array_params4_case() {
    let contract = real_contract();
    let result = contract
        .get_array_params4(
            &vec![],
            &vec![],
            &CellRef::new(vec![]),
            &vec![],
            &Some(vec![]),
            &None,
            &None,
            &CellRef::new(vec![]),
        )
        .await
        .expect("arrayParams4 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 74775,
                arguments: [
                    "tuple:[]",
                    "tuple:[]",
                    "cell:x{004_}",
                    "tuple:[]",
                    "tuple:[]",
                    "null",
                    "null",
                    "cell:x{004_}",
                ],
            },
            (),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    array_params4_generated,
    array_params4_dynamic,
    array_params4_case,
    array_params4_dynamic_case
);

async fn shape_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_shape_params1(
            &(
                bi(10),
                generated::Point::create(),
                (),
                (generated::Point::create(),),
                generated::Wrapper {
                    item: generated::Point::create(),
                },
                vec![TupleItem::Int(bi(1)), address_stack_item(&AnyAddr::None)],
                STR_128.to_owned(),
            ),
            &(
                (bi(1), None),
                CellRef::new((bi(8), None)),
                None,
                vec![],
                vec![bi(1), bi(2), bi(3)],
            ),
        )
        .await
        .expect("shapeParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 107407,
                arguments: [
                    "tuple:[10, tuple:[10, 20], tuple:[], tuple:[tuple:[10, 20]], tuple:[10, 20], tuple:[1, slice:x{2_}], cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}]",
                    "tuple:[tuple:[1, null], cell:x{084_}, null, tuple:[], tuple:[1, 2, 3]]",
                ],
            },
            (),
        )
    "#]].assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    shape_params1_generated,
    shape_params1_dynamic,
    shape_params1_case,
    shape_params1_dynamic_case
);

async fn lisp_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_lisp_params1(
            &vec![bi(1), bi(2), bi(3)],
            &vec![],
            &vec![
                generated::Point {
                    x: bi(-60),
                    y: bi(60),
                },
                generated::Point::create(),
            ],
            &vec![Some(bi(1)), None, Some(bi(3))],
            &vec![()],
            &vec![
                generated::Wrapper {
                    item: vec![bi(1), bi(2)],
                },
                generated::Wrapper { item: vec![] },
                generated::Wrapper {
                    item: vec![bi(5), bi(6)],
                },
            ],
            &CellRef::new(vec![(bi(32), bi(64)), (bi(320), bi(640))]),
            &vec!["one".to_owned(), "two".to_owned()],
        )
        .await
        .expect("lispParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 90594,
                arguments: [
                    "tuple:[1, tuple:[2, tuple:[3, null]]]",
                    "null",
                    "tuple:[tuple:[-60, 60], tuple:[tuple:[10, 20], null]]",
                    "tuple:[1, tuple:[null, tuple:[3, null]]]",
                    "tuple:[tuple:[], null]",
                    "tuple:[tuple:[1, 2], tuple:[tuple:[], tuple:[tuple:[5, 6], null]]]",
                    "cell:x{}",
                    "tuple:[cell:x{6F6E65}, tuple:[cell:x{74776F}, null]]",
                ],
            },
            (),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    lisp_params1_generated,
    lisp_params1_dynamic,
    lisp_params1_case,
    lisp_params1_dynamic_case
);

async fn wide_nullable_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_wide_nullable_params1(
            &generated::OnlyIntN { i: Some(bi(0)) },
            &generated::OnlyIntN { i: None },
            &Some(generated::OnlyIntN { i: Some(bi(2)) }),
            &Some(generated::OnlyIntN { i: None }),
            &None,
            &Some(generated::Wrapper {
                item: generated::OnlyIntN { i: None },
            }),
            &None,
            &Some((bi(7), ())),
            &None,
            &bi(777),
        )
        .await
        .expect("wideNullableParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 114471,
                arguments: [
                    "0",
                    "null",
                    "2",
                    "131",
                    "null",
                    "131",
                    "null",
                    "0",
                    "null",
                    "132",
                    "null",
                    "0",
                    "7",
                    "null",
                    "777",
                ],
            },
            777,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    wide_nullable_params1_generated,
    wide_nullable_params1_dynamic,
    wide_nullable_params1_case,
    wide_nullable_params1_dynamic_case
);

async fn wide_nullable_params2_case() {
    let contract = real_contract();
    let result = contract
        .get_wide_nullable_params2(
            &Some((bi(1), bi(2))),
            &Some(generated::Point::create()),
            &generated::WithWideNullables {
                pair_n: Some((bi(5), bi(6))),
                point_n: Some(generated::Point { x: bi(7), y: bi(8) }),
            },
            &generated::WithWideNullables {
                pair_n: None,
                point_n: None,
            },
            &generated::WithWideNullables {
                pair_n: Some((bi(5), bi(6))),
                point_n: None,
            },
            &Some(generated::WithWideNullables {
                pair_n: None,
                point_n: Some(generated::Point::create()),
            }),
            &None,
            &bi(777),
        )
        .await
        .expect("wideNullableParams2 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 102212,
                arguments: [
                    "1",
                    "2",
                    "133",
                    "10",
                    "20",
                    "134",
                    "5",
                    "6",
                    "133",
                    "7",
                    "8",
                    "134",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "0",
                    "5",
                    "6",
                    "133",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "0",
                    "10",
                    "20",
                    "134",
                    "135",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            777,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    wide_nullable_params2_generated,
    wide_nullable_params2_dynamic,
    wide_nullable_params2_case,
    wide_nullable_params2_dynamic_case
);

async fn union_params1_case() {
    let contract = real_contract();
    let result = contract
        .get_union_params1(
            &generated::UnionTy167::Variant0(bi(1)),
            &generated::UnionTy167::Variant1(true),
            &generated::UnionTy168::Variant0(bi(3)),
            &generated::UnionTy170::Variant1((bi(4), bi(4))),
            &generated::UnionTy171::Variant0(bi(5)),
            &generated::UnionTy172::Variant0(generated::Point { x: bi(6), y: bi(6) }),
            &bi(777),
        )
        .await
        .expect("unionParams1 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 69131,
                arguments: [
                    "1",
                    "1",
                    "-1",
                    "2",
                    "null",
                    "3",
                    "1",
                    "4",
                    "4",
                    "133",
                    "null",
                    "5",
                    "1",
                    "6",
                    "6",
                    "134",
                    "777",
                ],
            },
            777,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    union_params1_generated,
    union_params1_dynamic,
    union_params1_case,
    union_params1_dynamic_case
);

async fn union_params2_case() {
    let contract = real_contract();
    let result = contract
        .get_union_params2(
            &generated::UnionTy175::Variant1(generated::Point { x: bi(0), y: bi(0) }),
            &generated::UnionTy178::Variant0(CellRef::new(bi(2))),
            &generated::UnionTy178::Variant1((bi(3), bi(3), bi(3))),
            &generated::UnionTy178::Variant2(()),
            &generated::UnionTy180::Variant1(generated::TransferNotification { payload: () }),
            &Some(generated::Wrapper {
                item: generated::UnionTy181::Variant1(bi(6)),
            }),
            &Some(generated::Wrapper {
                item: generated::UnionTy181::Variant2(generated::IncreaseCounter {
                    query_id: bi(7),
                    increase_by: bi(7),
                }),
            }),
            &None,
            &bi(777),
        )
        .await
        .expect("unionParams2 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 81512,
                arguments: [
                    "null",
                    "null",
                    "0",
                    "0",
                    "134",
                    "null",
                    "null",
                    "cell:x{02}",
                    "137",
                    "3",
                    "3",
                    "3",
                    "138",
                    "null",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "139",
                    "null",
                    "6",
                    "46",
                    "140",
                    "7",
                    "7",
                    "136",
                    "140",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            777,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    union_params2_generated,
    union_params2_dynamic,
    union_params2_case,
    union_params2_dynamic_case
);

async fn union_params3_case() {
    let contract = real_contract();
    let result = contract
        .get_union_params3(
            &generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant0(generated::Point::create()),
                u2: generated::UnionTy188::Variant0(()),
                u3: generated::UnionTy189::Variant0(()),
            },
            &generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant1(((), generated::Point::create())),
                u2: generated::UnionTy188::Variant1(bi(2)),
                u3: generated::UnionTy189::Variant1((bi(2), bi(2))),
            },
            &Some(generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant1((
                    (),
                    generated::Point {
                        y: bi(70),
                        ..generated::Point::default()
                    },
                )),
                u2: generated::UnionTy188::Variant1(bi(3)),
                u3: generated::UnionTy189::Variant2(()),
            }),
            &None,
            &bi(777),
        )
        .await
        .expect("unionParams3 must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 77385,
                arguments: [
                    "10",
                    "20",
                    "134",
                    "null",
                    "141",
                    "null",
                    "null",
                    "141",
                    "10",
                    "20",
                    "142",
                    "2",
                    "1",
                    "2",
                    "2",
                    "133",
                    "10",
                    "70",
                    "142",
                    "3",
                    "1",
                    "null",
                    "null",
                    "0",
                    "143",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            777,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

generated_and_dynamic_tests!(
    union_params3_generated,
    union_params3_dynamic,
    union_params3_case,
    union_params3_dynamic_case
);

fn dynamic_number(value: i64) -> DynamicValue {
    DynamicValue::Number(bi(value))
}

fn dynamic_array(values: impl IntoIterator<Item = DynamicValue>) -> DynamicValue {
    DynamicValue::Array(values.into_iter().collect())
}

fn dynamic_point(x: i64, y: i64) -> DynamicValue {
    DynamicValue::structure(
        "Point",
        [("x", dynamic_number(x)), ("y", dynamic_number(y))],
    )
}

fn dynamic_wrapper(item: DynamicValue) -> DynamicValue {
    DynamicValue::structure("Wrapper", [("item", item)])
}

fn dynamic_builder(builder: CellBuilder) -> DynamicValue {
    DynamicValue::Builder(builder.build().expect("dynamic test builder must build"))
}

async fn complex_params1_dynamic_case() {
    let actual = call_dynamic(
        "complexParams1",
        &[
            dynamic_number(1),
            dynamic_number(2),
            dynamic_array([dynamic_number(3), dynamic_number(4)]),
            DynamicValue::AddressNone,
            DynamicValue::Slice(slice_uint(123, 16)),
            DynamicValue::Cell(cell_uint(123, 16)),
            dynamic_builder(builder_uint(123, 16)),
            dynamic_array([DynamicValue::Unknown(TupleItem::Null)]),
            DynamicValue::from("hello"),
            DynamicValue::from(STR_128),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 80154,
                arguments: [
                    "1",
                    "2",
                    "3",
                    "4",
                    "slice:x{2_}",
                    "slice:x{007B}",
                    "cell:x{007B}",
                    "builder:x{007B}",
                    "tuple:[null]",
                    "cell:x{68656C6C6F}",
                    "cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}",
                ],
            },
            Object(
                [
                    (
                        "$",
                        String(
                            "Wrapper",
                        ),
                    ),
                    (
                        "item",
                        Number(
                            1,
                        ),
                    ),
                ],
            ),
        )
    "#]].assert_debug_eq(&actual);
}

async fn complex_params2_dynamic_case() {
    let address = raw_address();
    let actual = call_dynamic(
        "complexParams2",
        &[
            DynamicValue::structure(
                "WithWrapper",
                [(
                    "nestedW",
                    dynamic_wrapper(dynamic_array([dynamic_number(1), dynamic_number(2)])),
                )],
            ),
            DynamicValue::structure(
                "PartReply",
                [
                    ("f1", dynamic_number(0)),
                    (
                        "f2",
                        dynamic_array([
                            DynamicValue::Unknown(TupleItem::Null),
                            DynamicValue::Unknown(TupleItem::Null),
                        ]),
                    ),
                    (
                        "f3",
                        dynamic_array([
                            dynamic_builder(CellBuilder::new()),
                            DynamicValue::structure(
                                "NestedPartReply",
                                [
                                    ("n1", DynamicValue::Bool(true)),
                                    ("n2", DynamicValue::Slice(slice_uint(0x0102, 16))),
                                ],
                            ),
                        ]),
                    ),
                ],
            ),
            dynamic_array([
                DynamicValue::Slice(OwnedSlice::full(empty_cell())),
                dynamic_number(123),
            ]),
            dynamic_array([DynamicValue::from(address)]),
            DynamicValue::reference(dynamic_number(123)),
            DynamicValue::structure("Empty", Vec::<(&str, DynamicValue)>::new()),
            dynamic_array([]),
            dynamic_array([
                dynamic_number(2),
                DynamicValue::Number(generated::E0Max::max_int().0),
            ]),
            DynamicValue::structure(
                "PackOptions",
                [("skipBitsNValidation", DynamicValue::Bool(true))],
            ),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 67961,
                arguments: [
                    "1",
                    "2",
                    "0",
                    "tuple:[null, null]",
                    "builder:x{}",
                    "-1",
                    "slice:x{0102}",
                    "slice:x{}",
                    "123",
                    "tuple:[slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}]",
                    "cell:x{7B}",
                    "tuple:[]",
                    "tuple:[2, 115792089237316195423570985008687907853269984665640564039457584007913129639935]",
                    "-1",
                ],
            },
            Array(
                [
                    Address(
                        Std(
                            StdAddr {
                                anycast: None,
                                workchain: 9,
                                address: 527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8,
                            },
                        ),
                    ),
                ],
            ),
        )
    "#]].assert_debug_eq(&actual);
}

async fn complex_params3_dynamic_case() {
    let null_actual = call_dynamic(
        "complexParams3",
        &[
            dynamic_array([
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
                dynamic_wrapper(DynamicValue::Null),
                dynamic_wrapper(dynamic_wrapper(DynamicValue::Null)),
                DynamicValue::Null,
                DynamicValue::Null,
                DynamicValue::Null,
            ]),
            DynamicValue::Bool(true),
        ],
    )
    .await;

    let value_actual = call_dynamic(
        "complexParams3",
        &[
            dynamic_array([
                dynamic_number(8),
                dynamic_number(50_000_000),
                DynamicValue::Cell(empty_cell()),
                DynamicValue::Slice(slice_uint(0x0102, 16)),
                dynamic_builder(builder_uint(0xff, 32)),
                DynamicValue::Bool(true),
                DynamicValue::from(raw_address()),
                dynamic_array([
                    DynamicValue::Unknown(TupleItem::Null),
                    DynamicValue::Unknown(TupleItem::Nan),
                ]),
                DynamicValue::Bits(BitString(slice_uint(0x0102, 16))),
                DynamicValue::reference(dynamic_number(8)),
                DynamicValue::reference(DynamicValue::reference(dynamic_number(8))),
                dynamic_wrapper(dynamic_number(123)),
                dynamic_wrapper(dynamic_wrapper(DynamicValue::Bool(true))),
                dynamic_array([dynamic_number(1), dynamic_array([dynamic_number(2)])]),
                DynamicValue::from("spoon"),
                DynamicValue::Null,
            ]),
            DynamicValue::Bool(false),
        ],
    )
    .await;

    expect![[r#"
        (
            (
                ObservedCall {
                    method_id: 72024,
                    arguments: [
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "null",
                        "-1",
                    ],
                },
                Array(
                    [
                        Null,
                        Null,
                    ],
                ),
            ),
            (
                ObservedCall {
                    method_id: 72024,
                    arguments: [
                        "8",
                        "50000000",
                        "cell:x{}",
                        "slice:x{0102}",
                        "builder:x{000000FF}",
                        "-1",
                        "slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
                        "tuple:[null, nan]",
                        "slice:x{0102}",
                        "cell:x{08}",
                        "cell:x{}",
                        "123",
                        "-1",
                        "tuple:[1, tuple:[2]]",
                        "cell:x{73706F6F6E}",
                        "null",
                        "0",
                    ],
                },
                Array(
                    [
                        Number(
                            8,
                        ),
                        Number(
                            50000000,
                        ),
                    ],
                ),
            ),
        )
    "#]].assert_debug_eq(&(null_actual, value_actual));
}

async fn complex_params4_dynamic_case() {
    let actual = call_dynamic(
        "complexParams4",
        &[
            DynamicValue::structure(
                "WithCells",
                [
                    ("c1", DynamicValue::reference(dynamic_number(1))),
                    ("c2", DynamicValue::Null),
                    (
                        "c3",
                        DynamicValue::reference(DynamicValue::structure(
                            "WithWrapper",
                            [("nestedW", dynamic_wrapper(dynamic_number(3)))],
                        )),
                    ),
                    (
                        "c4",
                        DynamicValue::reference(DynamicValue::union("int64", dynamic_number(4))),
                    ),
                    ("c5", DynamicValue::reference(DynamicValue::Null)),
                    ("c6", DynamicValue::reference(dynamic_number(1))),
                ],
            ),
            DynamicValue::reference(DynamicValue::structure(
                "PackOptions",
                [("skipBitsNValidation", DynamicValue::Bool(true))],
            )),
            DynamicValue::Map(vec![(dynamic_number(8), dynamic_number(50_000_000))]),
            DynamicValue::Map(vec![]),
            DynamicValue::Map(vec![(DynamicValue::from(raw_address()), dynamic_number(2))]),
            DynamicValue::Null,
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 92607,
                arguments: [
                    "cell:x{00000001}",
                    "null",
                    "cell:x{0000000000000003}",
                    "cell:x{80000000000000024_}",
                    "cell:x{4_}",
                    "cell:x{6_}",
                    "cell:x{C_}",
                    "cell:x{A02100BEBC202_}",
                    "null",
                    "cell:x{A1702549E5935573E9BADCC7D2FF01FA7409426025FBE1415467A14E6189E6F6100362}",
                    "129",
                    "null",
                    "0",
                ],
            },
            Void,
        )
    "#]].assert_debug_eq(&actual);
}

async fn complex_params5_dynamic_case() {
    let actual = call_dynamic(
        "complexParams5",
        &[
            DynamicValue::structure("WrapperN", [("item", DynamicValue::Null)]),
            DynamicValue::structure("WrapperN", [("item", dynamic_number(123))]),
            DynamicValue::structure("WrapperN", [("item", DynamicValue::Null)]),
            DynamicValue::structure(
                "WrapperN",
                [("item", DynamicValue::Slice(slice_uint(1, 8)))],
            ),
            DynamicValue::structure("WrapperN", [("item", DynamicValue::Null)]),
            DynamicValue::structure("WrapperN", [("item", DynamicValue::Null)]),
            DynamicValue::structure("WrapperN", [("item", DynamicValue::AddressNone)]),
            DynamicValue::structure("WrapperN", [("item", DynamicValue::from(raw_address()))]),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 96670,
                arguments: [
                    "null",
                    "123",
                    "null",
                    "slice:x{01}",
                    "null",
                    "null",
                    "slice:x{2_}",
                    "slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
                ],
            },
            Void,
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn array_params1_dynamic_case() {
    let actual = call_dynamic(
        "arrayParams1",
        &[
            DynamicValue::Array(
                unknown_values()
                    .0
                    .into_iter()
                    .map(DynamicValue::Unknown)
                    .collect(),
            ),
            dynamic_array([dynamic_number(1), dynamic_number(2), dynamic_number(3)]),
            dynamic_array([dynamic_number(1), DynamicValue::Null, dynamic_number(3)]),
            dynamic_array([dynamic_point(10, 20), dynamic_point(100, 200)]),
            dynamic_array([dynamic_array([]), dynamic_array([])]),
            dynamic_array([
                dynamic_array([dynamic_number(10), dynamic_number(20)]),
                dynamic_array([dynamic_number(30), dynamic_number(40)]),
            ]),
            dynamic_array([DynamicValue::from("one"), DynamicValue::from("two")]),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 95410,
                arguments: [
                    "tuple:[1, -1, slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}, slice:x{2_}, tuple:[10, 20]]",
                    "tuple:[1, 2, 3]",
                    "tuple:[1, null, 3]",
                    "tuple:[tuple:[10, 20], tuple:[100, 200]]",
                    "tuple:[tuple:[], tuple:[]]",
                    "tuple:[tuple:[10, 20], tuple:[30, 40]]",
                    "tuple:[cell:x{6F6E65}, cell:x{74776F}]",
                ],
            },
            Void,
        )
    "#]].assert_debug_eq(&actual);
}

async fn array_params2_dynamic_case() {
    let actual = call_dynamic(
        "arrayParams2",
        &[
            dynamic_array([]),
            dynamic_array([]),
            dynamic_array([]),
            dynamic_array([]),
            dynamic_array([]),
            dynamic_array([]),
            dynamic_array([]),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 83153,
                arguments: [
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                    "tuple:[]",
                ],
            },
            Void,
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn array_params3_dynamic_case() {
    let actual = call_dynamic(
        "arrayParams3",
        &[
            dynamic_array([dynamic_number(1), dynamic_number(2), dynamic_number(3)]),
            dynamic_array([
                dynamic_array([dynamic_number(1), dynamic_number(2)]),
                dynamic_array([]),
                dynamic_array([dynamic_number(5), dynamic_number(6)]),
            ]),
            DynamicValue::reference(dynamic_array([
                dynamic_number(1),
                dynamic_number(2),
                dynamic_number(3),
            ])),
            dynamic_array([
                DynamicValue::reference(dynamic_number(1)),
                DynamicValue::reference(dynamic_number(2)),
                DynamicValue::reference(dynamic_number(3)),
            ]),
            dynamic_array([dynamic_number(1), dynamic_number(2), dynamic_number(3)]),
            dynamic_array([dynamic_number(1), DynamicValue::Null, dynamic_number(3)]),
            dynamic_array([dynamic_point(10, 20), dynamic_point(-100, 100)]),
            DynamicValue::reference(dynamic_array([
                dynamic_array([dynamic_number(1), dynamic_number(2)]),
                DynamicValue::Null,
                DynamicValue::Null,
                dynamic_array([dynamic_number(7), dynamic_number(8)]),
            ])),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 87280,
                arguments: [
                    "tuple:[1, 2, 3]",
                    "tuple:[tuple:[1, 2], tuple:[], tuple:[5, 6]]",
                    "cell:x{03C_}",
                    "tuple:[cell:x{01}, cell:x{02}, cell:x{03}]",
                    "tuple:[1, 2, 3]",
                    "tuple:[1, null, 3]",
                    "tuple:[tuple:[10, 20], tuple:[-100, 100]]",
                    "cell:x{04C_}",
                ],
            },
            Void,
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn array_params4_dynamic_case() {
    let actual = call_dynamic(
        "arrayParams4",
        &[
            dynamic_array([]),
            dynamic_array([]),
            DynamicValue::reference(dynamic_array([])),
            dynamic_array([]),
            dynamic_array([]),
            DynamicValue::Null,
            DynamicValue::Null,
            DynamicValue::reference(dynamic_array([])),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 74775,
                arguments: [
                    "tuple:[]",
                    "tuple:[]",
                    "cell:x{004_}",
                    "tuple:[]",
                    "tuple:[]",
                    "null",
                    "null",
                    "cell:x{004_}",
                ],
            },
            Void,
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn shape_params1_dynamic_case() {
    let actual = call_dynamic(
        "shapeParams1",
        &[
            dynamic_array([
                dynamic_number(10),
                dynamic_point(10, 20),
                dynamic_array([]),
                dynamic_array([dynamic_point(10, 20)]),
                dynamic_wrapper(dynamic_point(10, 20)),
                dynamic_array([
                    DynamicValue::Unknown(TupleItem::Int(bi(1))),
                    DynamicValue::Unknown(address_stack_item(&AnyAddr::None)),
                ]),
                DynamicValue::from(STR_128),
            ]),
            dynamic_array([
                dynamic_array([dynamic_number(1), DynamicValue::Null]),
                DynamicValue::reference(dynamic_array([dynamic_number(8), DynamicValue::Null])),
                DynamicValue::Null,
                dynamic_array([]),
                dynamic_array([dynamic_number(1), dynamic_number(2), dynamic_number(3)]),
            ]),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 107407,
                arguments: [
                    "tuple:[10, tuple:[10, 20], tuple:[], tuple:[tuple:[10, 20]], tuple:[10, 20], tuple:[1, slice:x{2_}], cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}]",
                    "tuple:[tuple:[1, null], cell:x{084_}, null, tuple:[], tuple:[1, 2, 3]]",
                ],
            },
            Void,
        )
    "#]].assert_debug_eq(&actual);
}

async fn lisp_params1_dynamic_case() {
    let actual = call_dynamic(
        "lispParams1",
        &[
            dynamic_array([dynamic_number(1), dynamic_number(2), dynamic_number(3)]),
            dynamic_array([]),
            dynamic_array([dynamic_point(-60, 60), dynamic_point(10, 20)]),
            dynamic_array([dynamic_number(1), DynamicValue::Null, dynamic_number(3)]),
            dynamic_array([dynamic_array([])]),
            dynamic_array([
                dynamic_wrapper(dynamic_array([dynamic_number(1), dynamic_number(2)])),
                dynamic_wrapper(dynamic_array([])),
                dynamic_wrapper(dynamic_array([dynamic_number(5), dynamic_number(6)])),
            ]),
            DynamicValue::reference(dynamic_array([
                dynamic_array([dynamic_number(32), dynamic_number(64)]),
                dynamic_array([dynamic_number(320), dynamic_number(640)]),
            ])),
            dynamic_array([DynamicValue::from("one"), DynamicValue::from("two")]),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 90594,
                arguments: [
                    "tuple:[1, tuple:[2, tuple:[3, null]]]",
                    "null",
                    "tuple:[tuple:[-60, 60], tuple:[tuple:[10, 20], null]]",
                    "tuple:[1, tuple:[null, tuple:[3, null]]]",
                    "tuple:[tuple:[], null]",
                    "tuple:[tuple:[1, 2], tuple:[tuple:[], tuple:[tuple:[5, 6], null]]]",
                    "cell:x{}",
                    "tuple:[cell:x{6F6E65}, tuple:[cell:x{74776F}, null]]",
                ],
            },
            Void,
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn wide_nullable_params1_dynamic_case() {
    let actual = call_dynamic(
        "wideNullableParams1",
        &[
            DynamicValue::structure("OnlyIntN", [("i", dynamic_number(0))]),
            DynamicValue::structure("OnlyIntN", [("i", DynamicValue::Null)]),
            DynamicValue::structure("OnlyIntN", [("i", dynamic_number(2))]),
            DynamicValue::structure("OnlyIntN", [("i", DynamicValue::Null)]),
            DynamicValue::Null,
            dynamic_wrapper(DynamicValue::structure(
                "OnlyIntN",
                [("i", DynamicValue::Null)],
            )),
            DynamicValue::Null,
            dynamic_array([dynamic_number(7), dynamic_array([])]),
            DynamicValue::Null,
            dynamic_number(777),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 114471,
                arguments: [
                    "0",
                    "null",
                    "2",
                    "131",
                    "null",
                    "131",
                    "null",
                    "0",
                    "null",
                    "132",
                    "null",
                    "0",
                    "7",
                    "null",
                    "777",
                ],
            },
            Number(
                777,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn wide_nullable_params2_dynamic_case() {
    let actual = call_dynamic(
        "wideNullableParams2",
        &[
            dynamic_array([dynamic_number(1), dynamic_number(2)]),
            dynamic_point(10, 20),
            DynamicValue::structure(
                "WithWideNullables",
                [
                    (
                        "pairN",
                        dynamic_array([dynamic_number(5), dynamic_number(6)]),
                    ),
                    ("pointN", dynamic_point(7, 8)),
                ],
            ),
            DynamicValue::structure(
                "WithWideNullables",
                [
                    ("pairN", DynamicValue::Null),
                    ("pointN", DynamicValue::Null),
                ],
            ),
            DynamicValue::structure(
                "WithWideNullables",
                [
                    (
                        "pairN",
                        dynamic_array([dynamic_number(5), dynamic_number(6)]),
                    ),
                    ("pointN", DynamicValue::Null),
                ],
            ),
            DynamicValue::structure(
                "WithWideNullables",
                [
                    ("pairN", DynamicValue::Null),
                    ("pointN", dynamic_point(10, 20)),
                ],
            ),
            DynamicValue::Null,
            dynamic_number(777),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 102212,
                arguments: [
                    "1",
                    "2",
                    "133",
                    "10",
                    "20",
                    "134",
                    "5",
                    "6",
                    "133",
                    "7",
                    "8",
                    "134",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "0",
                    "5",
                    "6",
                    "133",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "0",
                    "10",
                    "20",
                    "134",
                    "135",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            Number(
                777,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn union_params1_dynamic_case() {
    let actual = call_dynamic(
        "unionParams1",
        &[
            DynamicValue::union("int", dynamic_number(1)),
            DynamicValue::union("bool", DynamicValue::Bool(true)),
            DynamicValue::union("int", dynamic_number(3)),
            DynamicValue::union(
                "tensor",
                dynamic_array([dynamic_number(4), dynamic_number(4)]),
            ),
            DynamicValue::union("int", dynamic_number(5)),
            dynamic_point(6, 6),
            dynamic_number(777),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 69131,
                arguments: [
                    "1",
                    "1",
                    "-1",
                    "2",
                    "null",
                    "3",
                    "1",
                    "4",
                    "4",
                    "133",
                    "null",
                    "5",
                    "1",
                    "6",
                    "6",
                    "134",
                    "777",
                ],
            },
            Number(
                777,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn union_params2_dynamic_case() {
    let actual = call_dynamic(
        "unionParams2",
        &[
            dynamic_point(0, 0),
            DynamicValue::union("Cell", DynamicValue::reference(dynamic_number(2))),
            DynamicValue::union(
                "tensor",
                dynamic_array([dynamic_number(3), dynamic_number(3), dynamic_number(3)]),
            ),
            DynamicValue::Null,
            DynamicValue::structure("TransferNotification", [("payload", DynamicValue::Void)]),
            dynamic_wrapper(DynamicValue::union("int32", dynamic_number(6))),
            dynamic_wrapper(DynamicValue::structure(
                "IncreaseCounter",
                [
                    ("queryId", dynamic_number(7)),
                    ("increaseBy", dynamic_number(7)),
                ],
            )),
            DynamicValue::Null,
            dynamic_number(777),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 81512,
                arguments: [
                    "null",
                    "null",
                    "0",
                    "0",
                    "134",
                    "null",
                    "null",
                    "cell:x{02}",
                    "137",
                    "3",
                    "3",
                    "3",
                    "138",
                    "null",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "139",
                    "null",
                    "6",
                    "46",
                    "140",
                    "7",
                    "7",
                    "136",
                    "140",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            Number(
                777,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

async fn union_params3_dynamic_case() {
    let actual = call_dynamic(
        "unionParams3",
        &[
            DynamicValue::structure(
                "WithWeirdUnions",
                [
                    ("u1", dynamic_point(10, 20)),
                    ("u2", DynamicValue::union("tensor", dynamic_array([]))),
                    ("u3", DynamicValue::union("()", dynamic_array([]))),
                ],
            ),
            DynamicValue::structure(
                "WithWeirdUnions",
                [
                    (
                        "u1",
                        DynamicValue::union(
                            "tensor",
                            dynamic_array([dynamic_array([]), dynamic_point(10, 20)]),
                        ),
                    ),
                    ("u2", DynamicValue::union("int", dynamic_number(2))),
                    (
                        "u3",
                        DynamicValue::union(
                            "(int, int)",
                            dynamic_array([dynamic_number(2), dynamic_number(2)]),
                        ),
                    ),
                ],
            ),
            DynamicValue::structure(
                "WithWeirdUnions",
                [
                    (
                        "u1",
                        DynamicValue::union(
                            "tensor",
                            dynamic_array([dynamic_array([]), dynamic_point(10, 70)]),
                        ),
                    ),
                    ("u2", DynamicValue::union("int", dynamic_number(3))),
                    ("u3", DynamicValue::Null),
                ],
            ),
            DynamicValue::Null,
            dynamic_number(777),
        ],
    )
    .await;

    expect![[r#"
        (
            ObservedCall {
                method_id: 77385,
                arguments: [
                    "10",
                    "20",
                    "134",
                    "null",
                    "141",
                    "null",
                    "null",
                    "141",
                    "10",
                    "20",
                    "142",
                    "2",
                    "1",
                    "2",
                    "2",
                    "133",
                    "10",
                    "70",
                    "142",
                    "3",
                    "1",
                    "null",
                    "null",
                    "0",
                    "143",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "null",
                    "0",
                    "777",
                ],
            },
            Number(
                777,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

#[tokio::test]
async fn default_params() {
    let first = real_contract();
    let first_result = first
        .get_default_params(&bi(1), generated::DefaultParamsArgs::default())
        .await
        .expect("defaultParams with only the required value must succeed");

    let second = real_contract();
    let second_result = second
        .get_default_params(
            &bi(2),
            generated::DefaultParamsArgs {
                d1: bi(50),
                d2: (bi(1), None),
                ..Default::default()
            },
        )
        .await
        .expect("defaultParams with two defaults overridden must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 66111,
                arguments: [
                    "1",
                    "50",
                    "1",
                    "null",
                    "tuple:[1, tuple:[2, null]]",
                    "10",
                    "20",
                    "10",
                    "20",
                    "134",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "null",
                    "0",
                ],
            },
            1,
            ObservedCall {
                method_id: 66111,
                arguments: [
                    "2",
                    "50",
                    "1",
                    "null",
                    "tuple:[1, tuple:[2, null]]",
                    "10",
                    "20",
                    "10",
                    "20",
                    "134",
                    "null",
                    "null",
                    "0",
                    "null",
                    "null",
                    "null",
                    "0",
                ],
            },
            2,
        )
    "#]]
    .assert_debug_eq(&(
        first.provider().take_call(),
        first_result,
        second.provider().take_call(),
        second_result,
    ));
}

#[tokio::test]
async fn collision_params() {
    let contract = real_contract();
    let result = contract
        .get_collision_params(
            &bi(123),
            &bi(888),
            &raw_address(),
            &CellRef::new(AnyAddr::None),
            &false,
            &false,
            &CellRef::new(OwnedSlice::full(empty_cell())),
            &"string".to_owned(),
        )
        .await
        .expect("collisionParams must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 82716,
                arguments: [
                    "123",
                    "888",
                    "slice:x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
                    "cell:x{2_}",
                    "0",
                    "0",
                    "cell:x{}",
                    "cell:x{737472696E67}",
                ],
            },
            (
                CellRef {
                    ref: None,
                },
                888,
            ),
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

#[tokio::test]
async fn processed() {
    let contract = real_contract();
    let result = contract
        .get_processed(&bi(123), &true)
        .await
        .expect("processed? must succeed");

    expect![[r#"
        (
            ObservedCall {
                method_id: 117746,
                arguments: [
                    "123",
                    "-1",
                ],
            },
            123,
        )
    "#]]
    .assert_debug_eq(&(contract.provider().take_call(), result));
}

async fn concat_strings_success_case() {
    let first = real_contract();
    let first_result = first
        .get_concat_strings(&vec!["1".to_owned(), "2".to_owned()])
        .await
        .expect("short concatStrings must succeed");

    let second = real_contract();
    let second_result = second
        .get_concat_strings(&vec![
            "1".to_owned(),
            "2".to_owned(),
            STR_128.to_owned(),
            STR_128.to_owned(),
            "klop".to_owned(),
        ])
        .await
        .expect("long concatStrings must succeed");

    let failing = real_contract();
    let error = failing
        .get_concat_strings(&vec![])
        .await
        .expect_err("empty concatStrings must fail")
        .to_string();

    expect![[r#"
        (
            ObservedCall {
                method_id: 77279,
                arguments: [
                    "tuple:[cell:x{31}, cell:x{32}]",
                ],
            },
            "12",
            ObservedCall {
                method_id: 77279,
                arguments: [
                    "tuple:[cell:x{31}, cell:x{32}, cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}, cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}, cell:x{6B6C6F70}]",
                ],
            },
            "121234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567812345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678klop",
            ObservedCall {
                method_id: 77279,
                arguments: [
                    "tuple:[]",
                ],
            },
            "contract provider failed: exit_code: 7",
        )
    "#]].assert_debug_eq(&(
        first.provider().take_call(),
        first_result,
        second.provider().take_call(),
        second_result,
        failing.provider().take_call(),
        error,
    ));
}

#[tokio::test]
async fn concat_strings() {
    concat_strings_success_case().await;
}

#[tokio::test]
async fn dynamic_concat_strings() {
    let first = call_dynamic(
        "concatStrings",
        &[dynamic_array([
            DynamicValue::from("1"),
            DynamicValue::from("2"),
        ])],
    )
    .await;

    let second = call_dynamic(
        "concatStrings",
        &[dynamic_array([
            DynamicValue::from("1"),
            DynamicValue::from("2"),
            DynamicValue::from(STR_128),
            DynamicValue::from(STR_128),
            DynamicValue::from("klop"),
        ])],
    )
    .await;

    let empty_array_error = call_dynamic_error("concatStrings", &[dynamic_array([])]).await;
    let missing_argument_error = call_dynamic_error("concatStrings", &[]).await;

    expect![[r#"
        (
            (
                ObservedCall {
                    method_id: 77279,
                    arguments: [
                        "tuple:[cell:x{31}, cell:x{32}]",
                    ],
                },
                String(
                    "12",
                ),
            ),
            (
                ObservedCall {
                    method_id: 77279,
                    arguments: [
                        "tuple:[cell:x{31}, cell:x{32}, cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}, cell:x{31323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637383930313233343536373839303132333435363738393031323334353637}, cell:x{6B6C6F70}]",
                    ],
                },
                String(
                    "121234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567812345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678klop",
                ),
            ),
            (
                Some(
                    ObservedCall {
                        method_id: 77279,
                        arguments: [
                            "tuple:[]",
                        ],
                    },
                ),
                "contract provider failed: exit_code: 7",
            ),
            (
                None,
                "cannot call get method 'concatStrings' dynamically: expected 1 arguments, got 0",
            ),
        )
    "#]].assert_debug_eq(&(
        first,
        second,
        empty_array_error,
        missing_argument_error,
    ));
}

#[tokio::test]
async fn errors_at_dynamic_call_get_methods() {
    let not_found = call_dynamic_error("adsf", &[]).await;
    let missing_arguments = call_dynamic_error("intAndPoint", &[]).await;
    let invalid_x = call_dynamic_error(
        "intAndPoint",
        &[
            dynamic_number(2),
            DynamicValue::object(Vec::<(&str, DynamicValue)>::new()),
        ],
    )
    .await;
    let invalid_y = call_dynamic_error(
        "intAndPoint",
        &[
            dynamic_number(2),
            DynamicValue::object([("x", dynamic_number(60)), ("y", DynamicValue::from("asdf"))]),
        ],
    )
    .await;

    expect![[r#"
        (
            (
                None,
                "cannot call get method 'adsf' dynamically: method not found in contract LotsOfGetters",
            ),
            (
                None,
                "cannot call get method 'intAndPoint' dynamically: expected 2 arguments, got 0",
            ),
            (
                None,
                "invalid value passed for 'p.x' of type 'int': not a number",
            ),
            (
                None,
                "invalid value passed for 'p.y' of type 'int': not a number",
            ),
        )
    "#]].assert_debug_eq(&(
        not_found,
        missing_arguments,
        invalid_x,
        invalid_y,
    ));
}
