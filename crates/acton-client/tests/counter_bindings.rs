use acton_client::{
    ContractProvider, ContractSender, InternalMessage, SendOptions, StdAddr, Tuple, TupleItem,
    decode, encode,
};
use expect_test::expect;
use num_bigint::BigInt;
use std::convert::Infallible;
use std::future::{Future, ready};
use std::sync::Mutex;

#[acton_client::contract(abi = "tests/fixtures/counter.abi.json")]
pub mod counter {}

#[test]
fn generated_counter_types_round_trip_through_cells() {
    let message = counter::IncreaseCounter {
        query_id: BigInt::from(0),
        increase_by: BigInt::from(5),
    };
    let message_cell = encode(&message).expect("message must encode");
    let decoded_message =
        decode::<counter::IncreaseCounter>(&message_cell).expect("message must decode");
    let reset = counter::ResetCounter {
        query_id: BigInt::from(0),
    };
    let reset_cell = encode(&reset).expect("message must encode");
    let decoded_reset = decode::<counter::ResetCounter>(&reset_cell).expect("message must decode");

    let storage = counter::Storage {
        id: BigInt::from(0),
        counter: BigInt::from(0),
    };
    let storage_cell = encode(&storage).expect("storage must encode");
    let decoded_storage = decode::<counter::Storage>(&storage_cell).expect("storage must decode");

    expect![[r"
        (
            128,
            IncreaseCounter {
                query_id: 0,
                increase_by: 5,
            },
            96,
            ResetCounter {
                query_id: 0,
            },
            64,
            Storage {
                id: 0,
                counter: 0,
            },
        )
    "]]
    .assert_debug_eq(&(
        message_cell.bit_len(),
        decoded_message,
        reset_cell.bit_len(),
        decoded_reset,
        storage_cell.bit_len(),
        decoded_storage,
    ));
}

#[derive(Debug, Default)]
struct SenderProvider {
    messages: Mutex<Vec<InternalMessage>>,
}

impl ContractSender for SenderProvider {
    type Error = Infallible;
    type Sender = ();
    type Output = ();

    fn send_internal(
        &self,
        _via: &Self::Sender,
        _address: &StdAddr,
        message: InternalMessage,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send {
        self.messages
            .lock()
            .expect("messages lock must not be poisoned")
            .push(message);
        ready(Ok(()))
    }
}

#[tokio::test]
async fn generated_senders_match_upstream_counter_scenarios() {
    let address = StdAddr {
        anycast: None,
        workchain: 0,
        address: Default::default(),
    };
    let contract = counter::TolkCounter::from_address(address, SenderProvider::default());

    contract
        .send_increase_counter(
            &(),
            BigInt::from(50_000_000_u64),
            &counter::IncreaseCounter {
                query_id: BigInt::from(0),
                increase_by: BigInt::from(5),
            },
            SendOptions::default(),
        )
        .await
        .expect("increase message must send");
    contract
        .send_reset_counter(
            &(),
            BigInt::from(50_000_000_u64),
            &counter::ResetCounter {
                query_id: BigInt::from(0),
            },
            SendOptions::default(),
        )
        .await
        .expect("reset message must send");

    let messages = contract
        .provider()
        .messages
        .lock()
        .expect("messages lock must not be poisoned");
    let increase = decode::<counter::IncreaseCounter>(&messages[0].body)
        .expect("increase message must decode");
    let reset =
        decode::<counter::ResetCounter>(&messages[1].body).expect("reset message must decode");
    expect![[r#"
        (
            "50000000",
            None,
            IncreaseCounter {
                query_id: 0,
                increase_by: 5,
            },
            "50000000",
            None,
            ResetCounter {
                query_id: 0,
            },
        )
    "#]]
    .assert_debug_eq(&(
        messages[0].value.to_string(),
        messages[0].options.bounce,
        increase,
        messages[1].value.to_string(),
        messages[1].options.bounce,
        reset,
    ));
}

#[test]
fn generated_counter_metadata_is_stable() {
    expect![[r#"
        (
            "1.0",
            "TolkCounter",
            "tolk",
            "1.4.2",
            [
                GetMethod {
                    name: "currentCounter",
                    method_id: 117456,
                },
                GetMethod {
                    name: "initialId",
                    method_id: 71937,
                },
            ],
            2122802415,
            980758278,
        )
    "#]]
    .assert_debug_eq(&(
        counter::ABI_SCHEMA_VERSION,
        counter::CONTRACT_NAME,
        counter::COMPILER_NAME,
        counter::COMPILER_VERSION,
        counter::GET_METHODS,
        counter::IncreaseCounter::PREFIX,
        counter::ResetCounter::PREFIX,
    ));
}

#[derive(Debug, Clone, Copy)]
struct CounterProvider;

impl ContractProvider for CounterProvider {
    type Error = Infallible;

    fn run_get_method(
        &self,
        _address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send {
        let result = match method_id {
            117_456 if arguments.is_empty() => Tuple(vec![TupleItem::Int(BigInt::from(5))]),
            71_937 if arguments.is_empty() => Tuple(vec![TupleItem::Int(BigInt::from(0))]),
            _ => Tuple::empty(),
        };
        ready(Ok(result))
    }
}

#[tokio::test]
async fn generated_getter_decodes_provider_result() {
    let address = StdAddr {
        anycast: None,
        workchain: 0,
        address: Default::default(),
    };
    let contract = counter::TolkCounter::from_address(address, CounterProvider);
    let current = contract
        .get_current_counter()
        .await
        .expect("getter result must decode");
    let initial = contract
        .get_initial_id()
        .await
        .expect("getter result must decode");

    expect![[r#"
        (
            "5",
            "0",
        )
    "#]]
    .assert_debug_eq(&(current.to_string(), initial.to_string()));
}
