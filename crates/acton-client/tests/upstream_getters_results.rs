#![allow(
    clippy::needless_raw_string_hashes,
    clippy::ptr_arg,
    clippy::too_many_arguments
)]

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-getters.abi.json")]
mod generated {}
#[allow(unreachable_pub, clippy::significant_drop_tightening)]
mod support;

use acton_client::__private::tycho_types::cell::{CellBuilder, Store};
use acton_client::__private::tycho_types::models::{AnyAddr, IntAddr};
use acton_client::{
    Cell, CellRef, ContractProvider, Dictionary, DynamicAbi, DynamicValue, OwnedSlice, SendOptions,
    StdAddr, Tuple, TupleItem,
};
use expect_test::expect;
use num_bigint::BigInt;
use std::convert::Infallible;
use std::fmt::Write as _;
use std::future::{Future, ready};
use std::sync::{Arc, Mutex};
use support::{TvmContractProvider, TvmGetterProvider, TvmSender, TvmTransaction};

const NANO_005: u64 = 50_000_000;
const STR_128: &str = "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678";

fn bi(value: impl Into<BigInt>) -> BigInt {
    value.into()
}

fn address() -> StdAddr {
    "9:527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8"
        .parse()
        .expect("upstream address must parse")
}

fn contract_address() -> StdAddr {
    StdAddr {
        anycast: None,
        workchain: 0,
        address: Default::default(),
    }
}

fn cell_uint(value: u64, bits: u16) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_uint(value, bits)
        .expect("fixture integer must fit");
    builder.build().expect("fixture cell must build")
}

fn cell_int(value: i64, bits: u16) -> Cell {
    let mut builder = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut builder, &bi(value), bits, true)
        .expect("fixture integer must fit");
    builder.build().expect("fixture cell must build")
}

fn cell_hex(cell: &Cell) -> String {
    format!(
        "x{{{:X}}}",
        cell.as_slice()
            .expect("fixture cell must be readable")
            .display_data()
    )
}

fn cell_tree(cell: &Cell) -> String {
    fn append(cell: &Cell, depth: usize, output: &mut String) {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&" ".repeat(depth));
        let slice = cell.as_slice().expect("cell must be readable");
        write!(output, "x{{{:X}}}", slice.display_data()).expect("writing to a String cannot fail");
        for index in 0..cell.reference_count() {
            append(
                &cell
                    .reference_cloned(index)
                    .expect("cell reference must exist"),
                depth + 1,
                output,
            );
        }
    }

    let mut output = String::new();
    append(cell, 0, &mut output);
    output
}

fn ref_cell(cell: &Cell) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_reference(cell.clone())
        .expect("fixture reference must fit");
    builder.build().expect("fixture cell must build")
}

const fn slice_item(cell: Cell) -> TupleItem {
    TupleItem::Slice(cell)
}

fn int_item(value: impl Into<BigInt>) -> TupleItem {
    TupleItem::Int(value.into())
}

const fn tuple_item(items: Vec<TupleItem>) -> TupleItem {
    TupleItem::Tuple(Tuple(items))
}

fn tlb_slice_item<T: Store>(value: &T) -> TupleItem {
    let mut items = Vec::new();
    acton_client::stack::write_tlb_slice(value, &mut items)
        .expect("fixture address must serialize");
    items.pop().expect("address writer must emit one item")
}

#[derive(Debug, Clone, Copy)]
enum ApiPath {
    Generated,
    Dynamic,
}

#[derive(Debug, Clone)]
struct ResultProvider {
    address: StdAddr,
    method_id: i32,
    arguments: Tuple,
    result: Tuple,
    path: ApiPath,
    inner: TvmGetterProvider,
}

impl ResultProvider {
    fn new(method_id: i32, arguments: Tuple, result: Tuple, path: ApiPath) -> Self {
        let (address, inner) = tvm_getter_provider();
        Self {
            address,
            method_id,
            arguments,
            result,
            path,
            inner,
        }
    }

    fn empty(method_id: i32, result: Tuple, path: ApiPath) -> Self {
        Self::new(method_id, Tuple::empty(), result, path)
    }
}

impl ContractProvider for ResultProvider {
    type Error = String;

    fn run_get_method(
        &self,
        address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send {
        assert_eq!(address, &self.address);
        assert_eq!(method_id, self.method_id);
        assert_eq!(arguments, self.arguments);

        async move {
            let actual = self
                .inner
                .run_get_method(address, method_id, arguments)
                .await?;
            assert_eq!(
                actual, self.result,
                "real TVM stack diverged for {:?} path",
                self.path
            );
            Ok(actual)
        }
    }
}

fn tvm_getter_provider() -> (StdAddr, TvmGetterProvider) {
    let storage = generated::StorageMe {
        id: bi(0),
        counter: bi(0),
    };
    let contract = generated::LotsOfGetters::from_storage(&storage)
        .expect("real TVM contract init must build");
    let address = contract.address().clone();
    let init = contract.init().expect("real TVM contract init must exist");
    let provider = TvmGetterProvider::new(address.clone(), init.code.clone(), init.data.clone());
    (address, provider)
}

fn result_contract(provider: ResultProvider) -> generated::LotsOfGetters<ResultProvider> {
    let address = provider.address.clone();
    generated::LotsOfGetters::from_address(address, provider)
}

fn normalize_tuple_item(value: &TupleItem) -> String {
    match value {
        TupleItem::Null => "null".to_owned(),
        TupleItem::Int(value) => value.to_string(),
        TupleItem::Nan => "nan".to_owned(),
        TupleItem::Cell(cell) => format!("cell({})", cell_hex(cell)),
        TupleItem::Slice(cell) => format!("slice({})", cell_hex(cell)),
        TupleItem::Builder(cell) => format!("builder({})", cell_hex(cell)),
        TupleItem::Tuple(tuple) => {
            format_normalized("[", "]", tuple.iter().map(normalize_tuple_item))
        }
        TupleItem::Cont(value) => format!("cont({value:?})"),
    }
}

fn format_normalized(open: &str, close: &str, values: impl IntoIterator<Item = String>) -> String {
    format!(
        "{open}{}{close}",
        values.into_iter().collect::<Vec<_>>().join(",")
    )
}

fn normalize_dynamic(value: &DynamicValue) -> String {
    match value {
        DynamicValue::Null => "null".to_owned(),
        DynamicValue::Void => "void".to_owned(),
        DynamicValue::Number(value) => value.to_string(),
        DynamicValue::Bool(value) => value.to_string(),
        DynamicValue::String(value) => format!("{value:?}"),
        DynamicValue::Cell(cell) => format!("cell({})", cell_hex(cell)),
        DynamicValue::Builder(cell) => format!("builder({})", cell_hex(cell)),
        DynamicValue::Slice(slice) => format!("slice({})", cell_hex(&slice.cell)),
        DynamicValue::Bits(bits) => format!("bits({})", cell_hex(&bits.0.cell)),
        DynamicValue::Address(IntAddr::Std(address)) => {
            format!("address(wc={})", address.workchain)
        }
        DynamicValue::Address(address) => format!("address({address:?})"),
        DynamicValue::ExtAddress(address) => format!("ext_address({address:?})"),
        DynamicValue::AddressNone => "none".to_owned(),
        DynamicValue::Array(values) => {
            format_normalized("[", "]", values.iter().map(normalize_dynamic))
        }
        DynamicValue::Map(values) => format_normalized(
            "map{",
            "}",
            values.iter().map(|(key, value)| {
                format!("{}:{}", normalize_dynamic(key), normalize_dynamic(value))
            }),
        ),
        DynamicValue::Object(fields) => format_normalized(
            "{",
            "}",
            fields
                .iter()
                .map(|(name, value)| format!("{name}:{}", normalize_dynamic(value))),
        ),
        DynamicValue::Unknown(value) => format!("unknown({})", normalize_tuple_item(value)),
    }
}

async fn call_dynamic(
    method_name: &str,
    method_id: i32,
    expected_arguments: Tuple,
    result: Tuple,
    arguments: &[DynamicValue],
) -> DynamicValue {
    let provider = ResultProvider::new(method_id, expected_arguments, result, ApiPath::Dynamic);
    let address = provider.address.clone();
    DynamicAbi::from_json(generated::ABI_JSON)
        .expect("upstream ABI must parse dynamically")
        .call_get_method(&provider, &address, method_name, arguments)
        .await
        .expect("dynamic getter must decode")
}

#[derive(Debug, Clone, Default)]
struct CaptureProvider {
    captured: Arc<Mutex<Option<(i32, Tuple)>>>,
}

impl CaptureProvider {
    fn take(&self) -> Tuple {
        self.captured
            .lock()
            .expect("capture lock must not be poisoned")
            .take()
            .expect("generated call must be captured")
            .1
    }
}

impl ContractProvider for CaptureProvider {
    type Error = Infallible;

    fn run_get_method(
        &self,
        _address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send {
        *self
            .captured
            .lock()
            .expect("capture lock must not be poisoned") = Some((method_id, arguments));
        let returns_int = matches!(method_id, 114_471 | 102_212 | 69_131 | 81_512 | 77_385);
        ready(Ok(if returns_int {
            Tuple(vec![int_item(0)])
        } else {
            Tuple::empty()
        }))
    }
}

async fn deployed_contract() -> (
    generated::LotsOfGetters<TvmContractProvider>,
    TvmTransaction,
) {
    let storage = generated::StorageMe {
        id: bi(0),
        counter: bi(0),
    };
    let contract = generated::LotsOfGetters::from_storage(&storage)
        .expect("contract deployment init must build");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("TVM transaction provider must initialize");
    let contract = contract.with_provider(provider);
    let deployer = TvmSender::new("deployer", 0x11);
    let transaction = contract
        .send_deploy(&deployer, bi(NANO_005), SendOptions::default())
        .await
        .expect("deployment message must send");
    (contract, transaction)
}

#[tokio::test]
async fn should_deploy() {
    let (contract, transaction) = deployed_contract().await;
    let init = contract.init().expect("deployment init must be retained");
    let decoded = generated::LotsOfGetters::<TvmContractProvider>::storage_from_cell(&init.data)
        .expect("deployment storage must deserialize");
    let deployed = contract
        .provider()
        .is_deployed()
        .expect("TVM deployment state must be readable");

    expect![[r#"
        (
            true,
            64,
            StorageMe {
                id: 0,
                counter: 0,
            },
            TvmTransaction {
                sender: "deployer",
                sender_matches: true,
                recipient_matches: true,
                value: 50000000,
                bounce: false,
                body: "x{}",
                opcode: None,
                deploy: true,
                success: true,
                aborted: false,
                exit_code: Some(
                    0,
                ),
                action_result_code: Some(
                    0,
                ),
            },
        )
    "#]]
    .assert_debug_eq(&(deployed, init.data.bit_len(), decoded, transaction));
}

#[tokio::test]
async fn should_increase_counter() {
    let (contract, _) = deployed_contract().await;
    let mut transitions = Vec::new();
    for (index, increase_by) in [13_u32, 42, 0].into_iter().enumerate() {
        let increaser = TvmSender::new(format!("increaser{index}"), 0x20 + index as u8);
        let before = contract
            .get_current_counter()
            .await
            .expect("counter getter must decode");
        let transaction = contract
            .send_increase_counter(
                &increaser,
                bi(NANO_005),
                &generated::IncreaseCounter {
                    query_id: bi(0),
                    increase_by: bi(increase_by),
                },
                SendOptions::default(),
            )
            .await
            .expect("increase message must send");
        let after = contract
            .get_current_counter()
            .await
            .expect("counter getter must decode");
        transitions.push((before, bi(increase_by), after, transaction));
    }

    expect![[r#"
        [
            (
                0,
                13,
                13,
                TvmTransaction {
                    sender: "increaser0",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF00000000000000000000000D}",
                    opcode: Some(
                        2122802415,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
            ),
            (
                13,
                42,
                55,
                TvmTransaction {
                    sender: "increaser1",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF00000000000000000000002A}",
                    opcode: Some(
                        2122802415,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
            ),
            (
                55,
                0,
                55,
                TvmTransaction {
                    sender: "increaser2",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF000000000000000000000000}",
                    opcode: Some(
                        2122802415,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
            ),
        ]
    "#]]
    .assert_debug_eq(&transitions);
}

#[tokio::test]
async fn should_reset_counter() {
    let (contract, _) = deployed_contract().await;
    let increaser = TvmSender::new("increaser", 0x20);
    let mut transactions = Vec::new();
    let mut values = vec![
        contract
            .get_current_counter()
            .await
            .expect("counter getter must decode"),
    ];
    transactions.push(
        contract
            .send_increase_counter(
                &increaser,
                bi(NANO_005),
                &generated::IncreaseCounter {
                    query_id: bi(0),
                    increase_by: bi(5),
                },
                SendOptions::default(),
            )
            .await
            .expect("increase message must send"),
    );
    values.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must decode"),
    );
    transactions.push(
        contract
            .send_reset_counter_alias(
                &increaser,
                bi(NANO_005),
                &generated::ResetCounter { query_id: bi(0) },
                SendOptions::default(),
            )
            .await
            .expect("reset message must send"),
    );
    values.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must decode"),
    );
    transactions.push(
        contract
            .send_transfer_notification(
                &increaser,
                bi(NANO_005),
                &generated::TransferNotification { payload: bi(500) },
                SendOptions::default(),
            )
            .await
            .expect("transfer-notification message must send"),
    );
    values.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must decode"),
    );

    expect![[r#"
        (
            [
                0,
                5,
                0,
                500,
            ],
            [
                TvmTransaction {
                    sender: "increaser",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF000000000000000000000005}",
                    opcode: Some(
                        2122802415,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
                TvmTransaction {
                    sender: "increaser",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{3A752F060000000000000000}",
                    opcode: Some(
                        980758278,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
                TvmTransaction {
                    sender: "increaser",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{F0F0F0F0000001F4}",
                    opcode: Some(
                        4042322160,
                    ),
                    deploy: false,
                    success: true,
                    aborted: false,
                    exit_code: Some(
                        0,
                    ),
                    action_result_code: Some(
                        0,
                    ),
                },
            ],
        )
    "#]]
    .assert_debug_eq(&(values, transactions));
}

async fn assert_complex_response1(path: ApiPath) {
    let mut items = vec![
        tlb_slice_item(&AnyAddr::None),
        int_item(0),
        tuple_item(vec![TupleItem::Null]),
        TupleItem::Builder(cell_uint(123, 32)),
        int_item(-1),
        slice_item(cell_uint(0x0102, 16)),
        tuple_item(Vec::new()),
        slice_item(Cell::default()),
        int_item(123),
        int_item(BigInt::from(1_u8) << 99),
        TupleItem::Null,
    ];
    acton_client::stack::write_string("hello", &mut items);
    acton_client::stack::write_string(STR_128, &mut items);
    let stack = Tuple(items);

    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("complexResponse1", 124_411, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[none,{$:"PartReply",f1:0,f2:[unknown(null)],f3:[builder(x{0000007B}),{$:"NestedPartReply",n1:true,n2:slice(x{0102})}]},[],[slice(x{}),123,633825300114114700748351602688],null,"hello","12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678"]"#]].assert_eq(&normalized);
        return;
    }

    let contract = result_contract(ResultProvider::empty(124_411, stack, path));
    let result = contract
        .get_complex_response1()
        .await
        .expect("complexResponse1 must decode");
    let nested_builder_cell = result
        .1
        .f3
        .0
        .clone()
        .build()
        .expect("builder must build into a cell");
    let mut nested_builder = nested_builder_cell
        .as_slice()
        .expect("builder cell must be readable as a slice");
    let nested_slice = result.1.f3.1.n2.as_slice().expect("slice must decode");
    let remaining = result.3.0.as_slice().expect("remaining slice must decode");

    expect![[r#"
        (
            (
                None,
                0,
                Some(
                    Null,
                ),
                (
                    32,
                    123,
                ),
                true,
                "x{0102}",
            ),
            (
                0,
                (
                    0,
                    0,
                ),
                123,
                633825300114114700748351602688,
                true,
                "hello",
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        (
            result.0,
            result.1.f1,
            result.1.f2.last().cloned(),
            (
                result.1.f3.0.size_bits(),
                acton_client::cell::load_fixed_int(&mut nested_builder, 32, false)
                    .expect("builder uint32 must decode"),
            ),
            result.1.f3.1.n1,
            format!("x{{{:X}}}", nested_slice.display_data()),
        ),
        (
            result.2.len(),
            (remaining.size_bits(), remaining.size_refs()),
            result.3.1,
            result.3.2,
            result.4.is_none(),
            result.5,
            result.6,
        ),
    ));
}

async fn assert_complex_response2(path: ApiPath) {
    let mut dictionary = Dictionary::new();
    dictionary.insert(bi(1), bi(10));
    let dictionary_root = acton_client::cell::build_dictionary_root::<32, _, _>(
        &dictionary,
        |key, builder| {
            acton_client::cell::store_fixed_int(builder, key, 32, false)?;
            Ok(())
        },
        |value, builder| {
            acton_client::cell::store_fixed_int(builder, value, 32, true)?;
            Ok(())
        },
    )
    .expect("fixture dictionary must serialize")
    .expect("non-empty fixture dictionary must have a root");
    let max_int = "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        .parse::<BigInt>()
        .expect("enum value must parse");
    let items = vec![
        int_item(500),
        int_item(NANO_005),
        tuple_item(vec![TupleItem::Cell(dictionary_root)]),
        TupleItem::Cell(cell_int(8, 8)),
        TupleItem::Cell(cell_int(8, 8)),
        tuple_item(vec![int_item(2), int_item(max_int)]),
        tuple_item(Vec::new()),
        int_item(-1),
    ];
    let stack = Tuple(items);
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("complexResponse2", 120_216, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[500,50000000,[cell(x{A0000000010000000A})],cell(x{08}),{ref:8},{$:"Empty"},[2,115792089237316195423570985008687907853269984665640564039457584007913129639935],[],{$:"PackOptions",skipBitsNValidation:true}]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(120_216, stack, path));
    let result = contract
        .get_complex_response2()
        .await
        .expect("complexResponse2 must decode");
    let dictionary = acton_client::cell::load_dictionary_root::<32, _, _>(
        result.2.0.as_ref(),
        |slice| acton_client::cell::load_fixed_int(slice, 32, false),
        |slice| acton_client::cell::load_fixed_int(slice, 32, true),
    )
    .expect("returned dictionary must decode");
    let mut int8 = result.3.as_slice().expect("int8 cell must be readable");

    expect![[r#"
        (
            500,
            50000000,
            Dictionary(
                [
                    (
                        1,
                        10,
                    ),
                ],
            ),
            8,
            8,
            Empty,
            2,
            115792089237316195423570985008687907853269984665640564039457584007913129639935,
            (),
            true,
        )
    "#]]
    .assert_debug_eq(&(
        result.0,
        result.1,
        dictionary,
        acton_client::cell::load_fixed_int(&mut int8, 8, true).expect("int8 cell must decode"),
        result.4.r#ref,
        result.5,
        result.6.0.0,
        result.6.1.0,
        result.7,
        result.8.skip_bits_n_validation,
    ));
}

fn complex_response3_stack(return_null: bool) -> Tuple {
    if return_null {
        return Tuple((0..16).map(|_| TupleItem::Null).collect());
    }

    Tuple(vec![
        int_item(8),
        int_item(NANO_005),
        TupleItem::Cell(Cell::default()),
        slice_item(cell_uint(0x0102, 16)),
        TupleItem::Builder(cell_uint(0xff, 32)),
        int_item(-1),
        tlb_slice_item(&address()),
        tuple_item(vec![int_item(1), int_item(2)]),
        slice_item(cell_uint(0x0102, 16)),
        TupleItem::Cell(cell_int(8, 8)),
        TupleItem::Cell(ref_cell(&cell_int(8, 8))),
        int_item(123),
        int_item(-1),
        tuple_item(vec![int_item(1), tuple_item(vec![int_item(2)])]),
        {
            let mut string = Vec::new();
            acton_client::stack::write_string("spoon", &mut string);
            string.pop().expect("string writer must emit one item")
        },
        TupleItem::Null,
    ])
}

async fn assert_complex_response3(path: ApiPath) {
    if matches!(path, ApiPath::Dynamic) {
        let nulls = call_dynamic(
            "complexResponse3",
            116_153,
            Tuple(vec![int_item(-1)]),
            complex_response3_stack(true),
            &[DynamicValue::Bool(true)],
        )
        .await;
        let values = call_dynamic(
            "complexResponse3",
            116_153,
            Tuple(vec![int_item(0)]),
            complex_response3_stack(false),
            &[DynamicValue::Bool(false)],
        )
        .await;
        let normalized = format!(
            "[{},{}]",
            normalize_dynamic(&nulls),
            normalize_dynamic(&values)
        );
        expect![[r#"[[null,null,null,null,null,null,null,null,null,null,null,{$:"Wrapper",item:null},{$:"WithWrapper",nestedW:{$:"Wrapper",item:{$:"Wrapper",item:null}}},null,null,null],[8,50000000,cell(x{}),slice(x{0102}),builder(x{000000FF}),true,address(wc=9),[unknown(1),unknown(2)],bits(x{0102}),{ref:8},{ref:{ref:8}},{$:"Wrapper",item:123},{$:"WithWrapper",nestedW:{$:"Wrapper",item:{$:"Wrapper",item:true}}},[1,[2]],"spoon",null]]"#]].assert_eq(&normalized);
        return;
    }

    let null_contract = result_contract(ResultProvider::new(
        116_153,
        Tuple(vec![int_item(-1)]),
        complex_response3_stack(true),
        path,
    ));
    let nulls = null_contract
        .get_complex_response3(&true)
        .await
        .expect("nullable response must decode");

    let value_contract = result_contract(ResultProvider::new(
        116_153,
        Tuple(vec![int_item(0)]),
        complex_response3_stack(false),
        path,
    ));
    let values = value_contract
        .get_complex_response3(&false)
        .await
        .expect("populated response must decode");
    let value_slice = values.3.as_ref().expect("slice must be present");
    let bits = values.8.as_ref().expect("bits must be present");

    expect![[r#"
        (
            [
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
            ],
            (
                (
                    8,
                    50000000,
                    "x{}",
                    "x{0102}",
                    "x{000000FF}",
                    true,
                    9,
                    2,
                ),
                (
                    16,
                    8,
                    8,
                    123,
                    true,
                    (
                        1,
                        2,
                    ),
                    "spoon",
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        [
            nulls.0.is_none(),
            nulls.1.is_none(),
            nulls.2.is_none(),
            nulls.3.is_none(),
            nulls.4.is_none(),
            nulls.5.is_none(),
            nulls.6.is_none(),
            nulls.7.is_none(),
            nulls.8.is_none(),
            nulls.9.is_none(),
            nulls.10.is_none(),
            nulls.11.item.is_none(),
            nulls.12.nested_w.item.item.is_none(),
            nulls.13.is_none(),
            nulls.14.is_none(),
        ],
        (
            (
                values.0.expect("int must be present"),
                values.1.expect("coins must be present"),
                cell_hex(values.2.as_ref().expect("cell must be present")),
                format!(
                    "x{{{:X}}}",
                    value_slice
                        .as_slice()
                        .expect("slice must be readable")
                        .display_data()
                ),
                cell_hex(
                    &values
                        .4
                        .as_ref()
                        .expect("builder must be present")
                        .clone()
                        .build()
                        .expect("builder must finish"),
                ),
                values.5.expect("bool must be present"),
                values.6.expect("address must be present").workchain,
                values.7.expect("tuple must be present").len(),
            ),
            (
                bits.0
                    .as_slice()
                    .expect("bits must be readable")
                    .size_bits(),
                *values.9.expect("cell ref must be present").r#ref,
                *values
                    .10
                    .expect("nested cell ref must be present")
                    .r#ref
                    .r#ref,
                values.11.item.expect("wrapped int must be present"),
                values
                    .12
                    .nested_w
                    .item
                    .item
                    .expect("wrapped bool must be present"),
                {
                    let tuple = values.13.expect("shaped tuple must be present");
                    (tuple.0, tuple.1.0)
                },
                values.14.expect("string must be present"),
                values.15,
            ),
        ),
    ));
}

async fn complex_response4_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    let with_cells = generated::WithCells {
        c1: CellRef::new(bi(1)),
        c2: None,
        c3: Some(CellRef::new(generated::WithWrapper {
            nested_w: generated::Wrapper { item: bi(3) },
        })),
        c4: CellRef::new(generated::UnionTy90::Variant1(bi(4))),
        c5: CellRef::new(None),
        c6: CellRef::new(generated::Color::green()),
    };
    let options = CellRef::new(generated::PackOptions {
        skip_bits_n_validation: true,
    });
    let mut coins = Dictionary::new();
    coins.insert(bi(8), bi(NANO_005));
    let bools = Dictionary::new();
    let mut colors = Dictionary::new();
    colors.insert(address(), generated::Color::blue());

    contract
        .get_complex_params4(&with_cells, &options, &coins, &bools, &Some(colors), &None)
        .await
        .expect("complexResponse4 mirror parameters must encode");
    provider.take()
}

async fn assert_complex_response4(path: ApiPath) {
    let stack = complex_response4_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("complexResponse4", 111_966, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"WithCells",c1:{ref:1},c2:null,c3:{ref:{$:"WithWrapper",nestedW:{$:"Wrapper",item:3}}},c4:{ref:{$:"int64",value:4}},c5:{ref:null},c6:{ref:1}},{ref:{$:"PackOptions",skipBitsNValidation:true}},map{8:50000000},map{},map{address(wc=9):2},null]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(111_966, stack, path));
    let result = contract
        .get_complex_response4()
        .await
        .expect("complexResponse4 must decode");
    let c4 = match result.0.c4.r#ref.as_ref() {
        generated::UnionTy90::Variant1(value) => value.clone(),
        other @ generated::UnionTy90::Variant0(_) => {
            panic!("expected int64 union variant, got {other:?}")
        }
    };
    let address_color = result
        .4
        .as_ref()
        .expect("address dictionary must be present")
        .0
        .first()
        .expect("address dictionary must contain one item");

    expect![[r#"
        (
            1,
            true,
            3,
            4,
            true,
            1,
            true,
            Dictionary(
                [
                    (
                        8,
                        50000000,
                    ),
                ],
            ),
            true,
            (
                9,
                2,
            ),
            true,
        )
    "#]]
    .assert_debug_eq(&(
        *result.0.c1.r#ref,
        result.0.c2.is_none(),
        result.0.c3.expect("c3 must be present").r#ref.nested_w.item,
        c4,
        result.0.c5.r#ref.is_none(),
        result.0.c6.r#ref.0,
        result.1.r#ref.skip_bits_n_validation,
        result.2,
        result.3.is_empty(),
        (address_color.0.workchain, address_color.1.0.clone()),
        result.5.is_none(),
    ));
}

async fn complex_response5_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
        .get_complex_params5(
            &generated::WrapperN::<BigInt> { item: None },
            &generated::WrapperN {
                item: Some(bi(123)),
            },
            &generated::WrapperN::<OwnedSlice> { item: None },
            &generated::WrapperN {
                item: Some(OwnedSlice::full(cell_uint(1, 8))),
            },
            &generated::WrapperN::<StdAddr> { item: None },
            &generated::WrapperN::<AnyAddr> { item: None },
            &generated::WrapperN {
                item: Some(AnyAddr::None),
            },
            &generated::WrapperN {
                item: Some(AnyAddr::Std(address())),
            },
        )
        .await
        .expect("complexResponse5 mirror parameters must encode");
    provider.take()
}

async fn assert_complex_response5(path: ApiPath) {
    let stack = complex_response5_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("complexResponse5", 107_903, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"WrapperN",item:null},{$:"WrapperN",item:123},{$:"WrapperN",item:null},{$:"WrapperN",item:slice(x{01})},{$:"WrapperN",item:null},{$:"WrapperN",item:null},{$:"WrapperN",item:none},{$:"WrapperN",item:address(wc=9)}]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(107_903, stack, path));
    let result = contract
        .get_complex_response5()
        .await
        .expect("complexResponse5 must decode");
    let slice = result.3.item.as_ref().expect("slice must be present");
    let workchain = match result.7.item.as_ref().expect("address must be present") {
        AnyAddr::Std(address) => address.workchain,
        other => panic!("expected standard address, got {other:?}"),
    };

    expect![[r#"
        (
            true,
            123,
            true,
            8,
            true,
            true,
            Some(
                None,
            ),
            9,
        )
    "#]]
    .assert_debug_eq(&(
        result.0.item.is_none(),
        result.1.item.expect("coins must be present"),
        result.2.item.is_none(),
        slice
            .as_slice()
            .expect("slice must be readable")
            .size_bits(),
        result.4.item.is_none(),
        result.5.item.is_none(),
        result.6.item,
        workchain,
    ));
}

async fn arrays_response1_stack(empty: bool) -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    let raw = if empty {
        Vec::new()
    } else {
        vec![
            int_item(1),
            int_item(-1),
            tlb_slice_item(&address()),
            tlb_slice_item(&AnyAddr::None),
            tuple_item(vec![int_item(10), int_item(20)]),
        ]
    };
    let ints = if empty {
        Vec::new()
    } else {
        vec![bi(1), bi(2), bi(3)]
    };
    let nullable_ints = if empty {
        Vec::new()
    } else {
        vec![Some(bi(1)), None, Some(bi(3))]
    };
    let points = if empty {
        Vec::new()
    } else {
        vec![
            generated::Point {
                x: bi(10),
                y: bi(20),
            },
            generated::Point {
                x: bi(100),
                y: bi(200),
            },
        ]
    };
    let units = if empty { Vec::new() } else { vec![(), ()] };
    let pairs = if empty {
        Vec::new()
    } else {
        vec![(bi(10), bi(20)), (bi(30), bi(40))]
    };
    let strings = if empty {
        Vec::new()
    } else {
        vec!["one".to_owned(), "two".to_owned()]
    };

    contract
        .get_array_params1(
            &raw,
            &ints,
            &nullable_ints,
            &points,
            &units,
            &pairs,
            &strings,
        )
        .await
        .expect("array response mirror parameters must encode");
    provider.take()
}

async fn assert_arrays_response1(path: ApiPath) {
    let stack = arrays_response1_stack(false).await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("arraysResponse1", 108_929, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[unknown(1),unknown(-1),unknown(slice(x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_})),unknown(slice(x{2_})),unknown([10,20])],[1,2,3],[1,null,3],[{$:"Point",x:10,y:20},{$:"Point",x:100,y:200}],[[],[]],[[10,20],[30,40]],["one","two"]]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(108_929, stack, path));
    let result = contract
        .get_arrays_response1()
        .await
        .expect("arraysResponse1 must decode");
    let none_address = match &result.0[3] {
        TupleItem::Slice(cell) => cell_hex(cell),
        other => panic!("expected raw slice, got {other:?}"),
    };

    expect![[r#"
        (
            5,
            Int(
                -1,
            ),
            "x{2_}",
            Tuple(
                Tuple(
                    [
                        Int(
                            10,
                        ),
                        Int(
                            20,
                        ),
                    ],
                ),
            ),
            [
                1,
                2,
                3,
            ],
            [
                Some(
                    1,
                ),
                None,
                Some(
                    3,
                ),
            ],
            [
                Point {
                    x: 10,
                    y: 20,
                },
                Point {
                    x: 100,
                    y: 200,
                },
            ],
            [
                (),
                (),
            ],
            [
                (
                    10,
                    20,
                ),
                (
                    30,
                    40,
                ),
            ],
            [
                "one",
                "two",
            ],
        )
    "#]]
    .assert_debug_eq(&(
        result.0.len(),
        result.0[1].clone(),
        none_address,
        result.0[4].clone(),
        result.1,
        result.2,
        result.3,
        result.4,
        result.5,
        result.6,
    ));
}

async fn assert_arrays_response2(path: ApiPath) {
    let stack = arrays_response1_stack(true).await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("arraysResponse2", 104_930, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[],[],[],[],[],[],[]]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(104_930, stack, path));
    let result = contract
        .get_arrays_response2()
        .await
        .expect("arraysResponse2 must decode");

    expect![[r#"
        [
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    "#]]
    .assert_debug_eq(&[
        result.0.len(),
        result.1.len(),
        result.2.len(),
        result.3.len(),
        result.4.len(),
        result.5.len(),
        result.6.len(),
    ]);
}

async fn arrays_response3_stack(empty: bool) -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    let p1 = if empty {
        Vec::new()
    } else {
        vec![bi(1), bi(2), bi(3)]
    };
    let p2 = if empty {
        Vec::new()
    } else {
        vec![vec![bi(1), bi(2)], Vec::new(), vec![bi(5), bi(6)]]
    };
    let p3 = CellRef::new(p1.clone());
    let p4 = if empty {
        Vec::new()
    } else {
        vec![
            CellRef::new(bi(1)),
            CellRef::new(bi(2)),
            CellRef::new(bi(3)),
        ]
    };
    let p5 = Some(if empty {
        Vec::new()
    } else {
        vec![bi(1), bi(2), bi(3)]
    });
    let p6 = if empty {
        None
    } else {
        Some(vec![Some(bi(1)), None, Some(bi(3))])
    };
    let p7 = if empty {
        None
    } else {
        Some(vec![
            generated::Point {
                x: bi(10),
                y: bi(20),
            },
            generated::Point {
                x: bi(-100),
                y: bi(200),
            },
        ])
    };
    let p8 = CellRef::new(if empty {
        Vec::new()
    } else {
        vec![Some((bi(1), bi(2))), None, None, Some((bi(7), bi(8)))]
    });

    contract
        .get_array_params3(&p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8)
        .await
        .expect("array cell response mirror parameters must encode");
    let mut stack = provider.take();
    if !empty {
        let TupleItem::Cell(mirror_p3) = &stack.0[2] else {
            panic!("arrayParams3 p3 must encode as a cell");
        };
        let TupleItem::Cell(mirror_p8) = &stack.0[7] else {
            panic!("arrayParams3 p8 must encode as a cell");
        };
        let (tolk_p3, tolk_p8) = tolk_arrays_response3_cells();
        let layouts = format!(
            "tolk p3:\n{}\ntolk p8:\n{}\nwrapper p3:\n{}\nwrapper p8:\n{}",
            cell_tree(&tolk_p3),
            cell_tree(&tolk_p8),
            cell_tree(mirror_p3),
            cell_tree(mirror_p8)
        );
        expect![[r#"tolk p3:
x{03C_}
 x{008101C_}
tolk p8:
x{04C_}
 x{40004000000088003800000044_}
wrapper p3:
x{03C_}
 x{80C_}
  x{814_}
   x{01C_}
wrapper p8:
x{04C_}
 x{C00040000000A_}
  x{A_}
   x{A_}
    x{4001C00000022_}"#]]
        .assert_eq(&layouts);

        // Upstream's wrapper intentionally writes one element per referenced cell,
        // while the Tolk compiler packs all fixture elements into one chunk. Use the
        // compiler layout for the expected get-method stack and assert it byte-for-byte.
        stack.0[2] = TupleItem::Cell(tolk_p3);
        stack.0[7] = TupleItem::Cell(tolk_p8);
    }
    stack
}

fn tolk_arrays_response3_cells() -> (Cell, Cell) {
    fn array_with_one_chunk(length: u8, store_items: impl FnOnce(&mut CellBuilder)) -> Cell {
        let mut chunk = CellBuilder::new();
        chunk
            .store_bit(false)
            .expect("final array chunk must have no tail");
        store_items(&mut chunk);
        let chunk = chunk.build().expect("array chunk must build");

        let mut root = CellBuilder::new();
        root.store_uint(u64::from(length), 8)
            .expect("array length must fit");
        root.store_bit(true).expect("array head must be present");
        root.store_reference(chunk)
            .expect("array head reference must fit");
        root.build().expect("array root must build")
    }

    let p3 = array_with_one_chunk(3, |chunk| {
        for value in [1, 2, 3] {
            acton_client::cell::store_fixed_int(chunk, &bi(value), 8, true)
                .expect("p3 int8 must fit");
        }
    });
    let p8 = array_with_one_chunk(4, |chunk| {
        for value in [Some((1, 2)), None, None, Some((7, 8))] {
            chunk
                .store_bit(value.is_some())
                .expect("p8 nullable prefix must fit");
            if let Some((first, second)) = value {
                acton_client::cell::store_fixed_int(chunk, &bi(first), 16, true)
                    .expect("p8 int16 must fit");
                acton_client::cell::store_fixed_int(chunk, &bi(second), 32, true)
                    .expect("p8 int32 must fit");
            }
        }
    });
    (p3, p8)
}

async fn assert_arrays_response3(path: ApiPath) {
    let stack = arrays_response3_stack(false).await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("arraysResponse3", 100_803, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[1,2,3],[[1,2],[],[5,6]],{ref:[1,2,3]},[{ref:1},{ref:2},{ref:3}],[1,2,3],[1,null,3],[{$:"Point",x:10,y:20},{$:"Point",x:-100,y:200}],{ref:[[1,2],null,null,[7,8]]}]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(100_803, stack, path));
    let result = contract
        .get_arrays_response3()
        .await
        .expect("arraysResponse3 must decode");

    expect![[r#"
        (
            [
                1,
                2,
                3,
            ],
            [
                [
                    1,
                    2,
                ],
                [],
                [
                    5,
                    6,
                ],
            ],
            CellRef {
                ref: [
                    1,
                    2,
                    3,
                ],
            },
            [
                CellRef {
                    ref: 1,
                },
                CellRef {
                    ref: 2,
                },
                CellRef {
                    ref: 3,
                },
            ],
            Some(
                [
                    1,
                    2,
                    3,
                ],
            ),
            Some(
                [
                    Some(
                        1,
                    ),
                    None,
                    Some(
                        3,
                    ),
                ],
            ),
            Some(
                [
                    Point {
                        x: 10,
                        y: 20,
                    },
                    Point {
                        x: -100,
                        y: 200,
                    },
                ],
            ),
            CellRef {
                ref: [
                    Some(
                        (
                            1,
                            2,
                        ),
                    ),
                    None,
                    None,
                    Some(
                        (
                            7,
                            8,
                        ),
                    ),
                ],
            },
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn assert_arrays_response4(path: ApiPath) {
    let mut stack = arrays_response3_stack(true).await;
    stack.push(tuple_item(vec![TupleItem::Null]));
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("arraysResponse4", 129_316, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[],[],{ref:[]},[],[],null,null,{ref:[]},[null]]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(129_316, stack, path));
    let result = contract
        .get_arrays_response4()
        .await
        .expect("arraysResponse4 must decode");

    expect![[r#"
        (
            [],
            [],
            CellRef {
                ref: [],
            },
            [],
            Some(
                [],
            ),
            None,
            None,
            CellRef {
                ref: [],
            },
            [
                (),
            ],
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn shape_response1_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    let point = generated::Point {
        x: bi(10),
        y: bi(20),
    };
    let p1 = (
        bi(10),
        point.clone(),
        (),
        (point.clone(),),
        generated::Wrapper {
            item: point.clone(),
        },
        vec![int_item(1), tlb_slice_item(&AnyAddr::None)],
        STR_128.to_owned(),
    );
    let p2 = (
        (bi(1), None),
        CellRef::new((bi(8), None)),
        None,
        Vec::new(),
        vec![bi(1), bi(2), bi(3)],
    );

    contract
        .get_shape_params1(&p1, &p2)
        .await
        .expect("shape response mirror parameters must encode");
    provider.take()
}

async fn assert_shape_response1(path: ApiPath) {
    let stack = shape_response1_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("shapeResponse1", 115_143, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[10,{$:"Point",x:10,y:20},[],[{$:"Point",x:10,y:20}],{$:"Wrapper",item:{$:"Point",x:10,y:20}},[unknown(1),unknown(slice(x{2_}))],"12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678"],[[1,null],{ref:[8,null]},null,[],[1,2,3]]]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(115_143, stack, path));
    let result = contract
        .get_shape_response1()
        .await
        .expect("shapeResponse1 must decode");

    expect![[r#"
        (
            (
                10,
                Point {
                    x: 10,
                    y: 20,
                },
                (),
                Point {
                    x: 10,
                    y: 20,
                },
                Point {
                    x: 10,
                    y: 20,
                },
                2,
                Int(
                    1,
                ),
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678",
            ),
            (
                (
                    1,
                    None,
                ),
                CellRef {
                    ref: (
                        8,
                        None,
                    ),
                },
                true,
                [],
                [
                    1,
                    2,
                    3,
                ],
            ),
        )
    "#]]
    .assert_debug_eq(&(
        (
            result.0.0,
            result.0.1,
            result.0.2,
            result.0.3.0,
            result.0.4.item,
            result.0.5.len(),
            result.0.5[0].clone(),
            result.0.6,
        ),
        (
            result.1.0,
            result.1.1,
            result.1.2.is_none(),
            result.1.3,
            result.1.4,
        ),
    ));
}

async fn lisp_response1_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
        .get_lisp_params1(
            &vec![bi(1), bi(2), bi(3)],
            &Vec::new(),
            &vec![
                generated::Point {
                    x: bi(-60),
                    y: bi(60),
                },
                generated::Point {
                    x: bi(10),
                    y: bi(20),
                },
            ],
            &vec![Some(bi(1)), None, Some(bi(3))],
            &vec![()],
            &vec![
                generated::Wrapper {
                    item: vec![bi(1), bi(2)],
                },
                generated::Wrapper { item: Vec::new() },
                generated::Wrapper {
                    item: vec![bi(5), bi(6)],
                },
            ],
            &CellRef::new(vec![(bi(32), bi(64)), (bi(320), bi(640))]),
            &vec!["one".to_owned(), "two".to_owned()],
        )
        .await
        .expect("lisp response mirror parameters must encode");
    provider.take()
}

async fn assert_lisp_response1(path: ApiPath) {
    let stack = lisp_response1_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("lispResponse1", 68_858, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[1,2,3],[],[{$:"Point",x:-60,y:60},{$:"Point",x:10,y:20}],[1,null,3],[[]],[{$:"Wrapper",item:[1,2]},{$:"Wrapper",item:[]},{$:"Wrapper",item:[5,6]}],{ref:[[32,64],[320,640]]},["one","two"]]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(68_858, stack, path));
    let result = contract
        .get_lisp_response1()
        .await
        .expect("lispResponse1 must decode");

    expect![[r#"
        (
            [
                1,
                2,
                3,
            ],
            [],
            [
                Point {
                    x: -60,
                    y: 60,
                },
                Point {
                    x: 10,
                    y: 20,
                },
            ],
            [
                Some(
                    1,
                ),
                None,
                Some(
                    3,
                ),
            ],
            [
                (),
            ],
            [
                Wrapper {
                    item: [
                        1,
                        2,
                    ],
                },
                Wrapper {
                    item: [],
                },
                Wrapper {
                    item: [
                        5,
                        6,
                    ],
                },
            ],
            CellRef {
                ref: [
                    (
                        32,
                        64,
                    ),
                    (
                        320,
                        640,
                    ),
                ],
            },
            [
                "one",
                "two",
            ],
        )
    "#]]
    .assert_debug_eq(&result);
}

fn without_final_argument(mut tuple: Tuple) -> Tuple {
    assert_eq!(tuple.pop(), Some(int_item(0)));
    tuple
}

async fn wide_nullable_response1_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
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
            &bi(0),
        )
        .await
        .expect("wide nullable response mirror parameters must encode");
    without_final_argument(provider.take())
}

async fn assert_wide_nullable_response1(path: ApiPath) {
    let stack = wide_nullable_response1_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value =
            call_dynamic("wideNullableResponse1", 111_419, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"OnlyIntN",i:0},{$:"OnlyIntN",i:null},{$:"OnlyIntN",i:2},{$:"OnlyIntN",i:null},null,{$:"Wrapper",item:{$:"OnlyIntN",i:null}},null,[7,[]],null]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(111_419, stack, path));
    let result = contract
        .get_wide_nullable_response1()
        .await
        .expect("wideNullableResponse1 must decode");

    expect![[r#"
        (
            OnlyIntN {
                i: Some(
                    0,
                ),
            },
            OnlyIntN {
                i: None,
            },
            Some(
                OnlyIntN {
                    i: Some(
                        2,
                    ),
                },
            ),
            Some(
                OnlyIntN {
                    i: None,
                },
            ),
            None,
            Some(
                Wrapper {
                    item: OnlyIntN {
                        i: None,
                    },
                },
            ),
            None,
            Some(
                (
                    7,
                    (),
                ),
            ),
            None,
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn wide_nullable_response2_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    let point = generated::Point {
        x: bi(10),
        y: bi(20),
    };
    contract
        .get_wide_nullable_params2(
            &Some((bi(1), bi(2))),
            &Some(point.clone()),
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
                point_n: Some(point),
            }),
            &None,
            &bi(0),
        )
        .await
        .expect("wide nullable response mirror parameters must encode");
    without_final_argument(provider.take())
}

async fn assert_wide_nullable_response2(path: ApiPath) {
    let stack = wide_nullable_response2_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("wideNullableResponse2", 99_160, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[[1,2],{$:"Point",x:10,y:20},{$:"WithWideNullables",pairN:[5,6],pointN:{$:"Point",x:7,y:8}},{$:"WithWideNullables",pairN:null,pointN:null},{$:"WithWideNullables",pairN:[5,6],pointN:null},{$:"WithWideNullables",pairN:null,pointN:{$:"Point",x:10,y:20}},null]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(99_160, stack, path));
    let result = contract
        .get_wide_nullable_response2()
        .await
        .expect("wideNullableResponse2 must decode");

    expect![[r#"
        (
            Some(
                (
                    1,
                    2,
                ),
            ),
            Some(
                Point {
                    x: 10,
                    y: 20,
                },
            ),
            WithWideNullables {
                pair_n: Some(
                    (
                        5,
                        6,
                    ),
                ),
                point_n: Some(
                    Point {
                        x: 7,
                        y: 8,
                    },
                ),
            },
            WithWideNullables {
                pair_n: None,
                point_n: None,
            },
            WithWideNullables {
                pair_n: Some(
                    (
                        5,
                        6,
                    ),
                ),
                point_n: None,
            },
            Some(
                WithWideNullables {
                    pair_n: None,
                    point_n: Some(
                        Point {
                            x: 10,
                            y: 20,
                        },
                    ),
                },
            ),
            None,
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn union_response1_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
        .get_union_params1(
            &generated::UnionTy167::Variant0(bi(1)),
            &generated::UnionTy167::Variant1(true),
            &generated::UnionTy168::Variant0(bi(3)),
            &generated::UnionTy170::Variant1((bi(4), bi(4))),
            &generated::UnionTy171::Variant0(bi(5)),
            &generated::UnionTy172::Variant0(generated::Point { x: bi(6), y: bi(6) }),
            &bi(0),
        )
        .await
        .expect("union response mirror parameters must encode");
    without_final_argument(provider.take())
}

async fn assert_union_response1(path: ApiPath) {
    let stack = union_response1_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("unionResponse1", 97_257, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"int",value:1},{$:"bool",value:true},{$:"int",value:3},{$:"tensor",value:[4,4]},{$:"int",value:5},{$:"Point",x:6,y:6}]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(97_257, stack, path));
    let result = contract
        .get_union_response1()
        .await
        .expect("unionResponse1 must decode");

    expect![[r#"
        (
            Variant0(
                1,
            ),
            Variant1(
                true,
            ),
            Variant0(
                3,
            ),
            Variant1(
                (
                    4,
                    4,
                ),
            ),
            Variant0(
                5,
            ),
            Variant0(
                Point {
                    x: 6,
                    y: 6,
                },
            ),
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn union_response2_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
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
            &bi(0),
        )
        .await
        .expect("nested union response mirror parameters must encode");
    without_final_argument(provider.take())
}

async fn assert_union_response2(path: ApiPath) {
    let stack = union_response2_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("unionResponse2", 84_874, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"Point",x:0,y:0},{$:"Cell",value:{ref:2}},{$:"tensor",value:[3,3,3]},null,{$:"TransferNotification",payload:void},{$:"Wrapper",item:{$:"int32",value:6}},{$:"Wrapper",item:{$:"IncreaseCounter",queryId:7,increaseBy:7}},null]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(84_874, stack, path));
    let result = contract
        .get_union_response2()
        .await
        .expect("unionResponse2 must decode");

    expect![[r#"
        (
            Variant1(
                Point {
                    x: 0,
                    y: 0,
                },
            ),
            Variant0(
                CellRef {
                    ref: 2,
                },
            ),
            Variant1(
                (
                    3,
                    3,
                    3,
                ),
            ),
            Variant2(
                (),
            ),
            Variant1(
                TransferNotification {
                    payload: (),
                },
            ),
            Some(
                Wrapper {
                    item: Variant1(
                        6,
                    ),
                },
            ),
            Some(
                Wrapper {
                    item: Variant2(
                        IncreaseCounter {
                            query_id: 7,
                            increase_by: 7,
                        },
                    ),
                },
            ),
            None,
        )
    "#]]
    .assert_debug_eq(&result);
}

async fn union_response3_stack() -> Tuple {
    let provider = CaptureProvider::default();
    let contract = generated::LotsOfGetters::from_address(contract_address(), provider.clone());
    contract
        .get_union_params3(
            &generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant0(generated::Point {
                    x: bi(10),
                    y: bi(20),
                }),
                u2: generated::UnionTy188::Variant0(()),
                u3: generated::UnionTy189::Variant0(()),
            },
            &generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant1((
                    (),
                    generated::Point {
                        x: bi(10),
                        y: bi(20),
                    },
                )),
                u2: generated::UnionTy188::Variant1(bi(2)),
                u3: generated::UnionTy189::Variant1((bi(2), bi(2))),
            },
            &Some(generated::WithWeirdUnions {
                u1: generated::UnionTy187::Variant1((
                    (),
                    generated::Point {
                        x: bi(10),
                        y: bi(70),
                    },
                )),
                u2: generated::UnionTy188::Variant1(bi(3)),
                u3: generated::UnionTy189::Variant2(()),
            }),
            &None,
            &bi(0),
        )
        .await
        .expect("weird union response mirror parameters must encode");
    without_final_argument(provider.take())
}

async fn assert_union_response3(path: ApiPath) {
    let stack = union_response3_stack().await;
    if matches!(path, ApiPath::Dynamic) {
        let value = call_dynamic("unionResponse3", 89_003, Tuple::empty(), stack, &[]).await;
        let normalized = normalize_dynamic(&value);
        expect![[r#"[{$:"WithWeirdUnions",u1:{$:"Point",x:10,y:20},u2:{$:"tensor",value:[]},u3:{$:"()",value:[]}},{$:"WithWeirdUnions",u1:{$:"tensor",value:[[],{$:"Point",x:10,y:20}]},u2:{$:"int",value:2},u3:{$:"(int, int)",value:[2,2]}},{$:"WithWeirdUnions",u1:{$:"tensor",value:[[],{$:"Point",x:10,y:70}]},u2:{$:"int",value:3},u3:null},null]"#]].assert_eq(&normalized);
        return;
    }
    let contract = result_contract(ResultProvider::empty(89_003, stack, path));
    let result = contract
        .get_union_response3()
        .await
        .expect("unionResponse3 must decode");

    expect![[r#"
        (
            WithWeirdUnions {
                u1: Variant0(
                    Point {
                        x: 10,
                        y: 20,
                    },
                ),
                u2: Variant0(
                    (),
                ),
                u3: Variant0(
                    (),
                ),
            },
            WithWeirdUnions {
                u1: Variant1(
                    (
                        (),
                        Point {
                            x: 10,
                            y: 20,
                        },
                    ),
                ),
                u2: Variant1(
                    2,
                ),
                u3: Variant1(
                    (
                        2,
                        2,
                    ),
                ),
            },
            Some(
                WithWeirdUnions {
                    u1: Variant1(
                        (
                            (),
                            Point {
                                x: 10,
                                y: 70,
                            },
                        ),
                    ),
                    u2: Variant1(
                        3,
                    ),
                    u3: Variant2(
                        (),
                    ),
                },
            ),
            None,
        )
    "#]]
    .assert_debug_eq(&result);
}

macro_rules! dual_getter_tests {
    ($generated:ident, $dynamic:ident, $runner:ident) => {
        #[tokio::test]
        async fn $generated() {
            $runner(ApiPath::Generated).await;
        }

        #[tokio::test]
        async fn $dynamic() {
            $runner(ApiPath::Dynamic).await;
        }
    };
}

dual_getter_tests!(
    complex_response1_generated,
    complex_response1_dynamic,
    assert_complex_response1
);
dual_getter_tests!(
    complex_response2_generated,
    complex_response2_dynamic,
    assert_complex_response2
);
dual_getter_tests!(
    complex_response3_generated,
    complex_response3_dynamic,
    assert_complex_response3
);
dual_getter_tests!(
    complex_response4_generated,
    complex_response4_dynamic,
    assert_complex_response4
);
dual_getter_tests!(
    complex_response5_generated,
    complex_response5_dynamic,
    assert_complex_response5
);
dual_getter_tests!(
    arrays_response1_generated,
    arrays_response1_dynamic,
    assert_arrays_response1
);
dual_getter_tests!(
    arrays_response2_generated,
    arrays_response2_dynamic,
    assert_arrays_response2
);
dual_getter_tests!(
    arrays_response3_generated,
    arrays_response3_dynamic,
    assert_arrays_response3
);
dual_getter_tests!(
    arrays_response4_generated,
    arrays_response4_dynamic,
    assert_arrays_response4
);
dual_getter_tests!(
    shape_response1_generated,
    shape_response1_dynamic,
    assert_shape_response1
);
dual_getter_tests!(
    lisp_response1_generated,
    lisp_response1_dynamic,
    assert_lisp_response1
);
dual_getter_tests!(
    wide_nullable_response1_generated,
    wide_nullable_response1_dynamic,
    assert_wide_nullable_response1
);
dual_getter_tests!(
    wide_nullable_response2_generated,
    wide_nullable_response2_dynamic,
    assert_wide_nullable_response2
);
dual_getter_tests!(
    union_response1_generated,
    union_response1_dynamic,
    assert_union_response1
);
dual_getter_tests!(
    union_response2_generated,
    union_response2_dynamic,
    assert_union_response2
);
dual_getter_tests!(
    union_response3_generated,
    union_response3_dynamic,
    assert_union_response3
);
