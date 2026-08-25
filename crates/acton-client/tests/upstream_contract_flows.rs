mod support;

use acton_client::__private::tycho_types::cell::CellBuilder;
use acton_client::{AbiStore, BigInt, Cell, SendOptions};
use expect_test::expect;
use support::{TvmContractProvider, TvmSender};

#[acton_client::contract(abi = "tests/fixtures/upstream/tolk_counter.abi.json")]
mod tolk_counter {}

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-messages.abi.json")]
mod lots_of_messages {}

fn msg_value() -> BigInt {
    BigInt::from(50_000_000_u64)
}

fn cell_hex(cell: &Cell) -> String {
    format!(
        "x{{{:X}}}",
        cell.as_slice()
            .expect("cell must be readable")
            .display_data()
    )
}

async fn setup_counter() -> tolk_counter::TolkCounter<TvmContractProvider> {
    let storage = tolk_counter::Storage {
        id: BigInt::from(0),
        counter: BigInt::from(0),
    };
    let contract = tolk_counter::TolkCounter::from_storage(&storage)
        .expect("counter deployment init must encode");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("local TVM provider must initialize");
    let contract = contract.with_provider(provider);
    contract
        .send_deploy(
            &TvmSender::new("deployer", 0xd0),
            msg_value(),
            SendOptions::default(),
        )
        .await
        .expect("deploy transaction must execute");
    contract
}

// Upstream: TolkCounter.spec.ts — "should deploy"
#[tokio::test]
async fn tolk_counter_should_deploy() {
    let contract = setup_counter().await;

    expect![[r#"
        (
            true,
            [
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
            ],
        )
    "#]]
    .assert_debug_eq(&(
        contract
            .provider()
            .is_deployed()
            .expect("deployment state must be readable"),
        contract
            .provider()
            .transactions()
            .expect("transactions must be readable"),
    ));
}

fn next_upstream_increase(seed: &mut u64) -> BigInt {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    BigInt::from((*seed >> 32) % 100)
}

// Upstream: TolkCounter.spec.ts — "should increase counter"
#[tokio::test]
async fn tolk_counter_should_increase_counter() {
    let contract = setup_counter().await;
    let mut seed = 0x544f_4c4b_434f_554e_u64;
    let mut transitions = Vec::new();

    for index in 0..3 {
        let increaser = TvmSender::new(format!("increaser{index}"), 0x10 + index);
        let before = contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed");
        let increase_by = next_upstream_increase(&mut seed);
        contract
            .send_increase_counter(
                &increaser,
                msg_value(),
                &tolk_counter::IncreaseCounter {
                    query_id: BigInt::from(0),
                    increase_by: increase_by.clone(),
                },
                SendOptions::default(),
            )
            .await
            .expect("increase message must send");
        let after = contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed");
        transitions.push((
            before.clone(),
            increase_by.clone(),
            after,
            before + increase_by,
        ));
    }

    expect![[r#"
        (
            [
                (
                    0,
                    59,
                    59,
                    59,
                ),
                (
                    59,
                    60,
                    119,
                    119,
                ),
                (
                    119,
                    1,
                    120,
                    120,
                ),
            ],
            [
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
                TvmTransaction {
                    sender: "increaser0",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF00000000000000000000003B}",
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
                    sender: "increaser1",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF00000000000000000000003C}",
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
                    sender: "increaser2",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{7E8764EF000000000000000000000001}",
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
            ],
        )
    "#]]
    .assert_debug_eq(&(
        transitions,
        contract
            .provider()
            .transactions()
            .expect("transactions must be readable"),
    ));
}

// Upstream: TolkCounter.spec.ts — "should reset counter"
#[tokio::test]
async fn tolk_counter_should_reset_counter() {
    let contract = setup_counter().await;
    let increaser = TvmSender::new("increaser", 0x10);
    let before = contract
        .get_current_counter()
        .await
        .expect("counter getter must succeed");
    let increase_by = BigInt::from(5);
    contract
        .send_increase_counter(
            &increaser,
            msg_value(),
            &tolk_counter::IncreaseCounter {
                query_id: BigInt::from(0),
                increase_by: increase_by.clone(),
            },
            SendOptions::default(),
        )
        .await
        .expect("increase message must send");
    let increased = contract
        .get_current_counter()
        .await
        .expect("counter getter must succeed");
    contract
        .send_reset_counter(
            &increaser,
            msg_value(),
            &tolk_counter::ResetCounter {
                query_id: BigInt::from(0),
            },
            SendOptions::default(),
        )
        .await
        .expect("reset message must send");
    let reset = contract
        .get_current_counter()
        .await
        .expect("counter getter must succeed");

    expect![[r#"
        (
            0,
            5,
            5,
            5,
            0,
            0,
            [
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
            ],
        )
    "#]]
    .assert_debug_eq(&(
        before,
        increase_by.clone(),
        increased,
        increase_by,
        reset,
        BigInt::from(0),
        contract
            .provider()
            .transactions()
            .expect("transactions must be readable"),
    ));
}

async fn setup_lots_of_messages() -> lots_of_messages::LotsOfMessages<TvmContractProvider> {
    let storage = lots_of_messages::BasicStorage::<BigInt> {
        counter_id: BigInt::from(0),
        counter_value: BigInt::from(0),
    };
    let contract = lots_of_messages::LotsOfMessages::from_storage(&storage)
        .expect("lots-of-messages deployment init must encode");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("local TVM provider must initialize");
    let contract = contract.with_provider(provider);
    contract
        .send_deploy(
            &TvmSender::new("deployer", 0xd0),
            msg_value(),
            SendOptions::default(),
        )
        .await
        .expect("deploy transaction must execute");
    contract
}

// Upstream: LotsOfMessages.spec.ts — "should increase and reset"
#[tokio::test]
async fn lots_of_messages_should_increase_and_reset() {
    let contract = setup_lots_of_messages().await;
    let increaser = TvmSender::new("increaser", 0x10);
    let mut observed = Vec::new();

    observed.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed"),
    );
    contract
        .send_increase_by(
            &increaser,
            msg_value(),
            &lots_of_messages::IncreaseBy::create(BigInt::from(0)),
            SendOptions::default(),
        )
        .await
        .expect("default increase message must send");
    observed.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed"),
    );
    contract
        .send_increase_by(
            &increaser,
            msg_value(),
            &lots_of_messages::IncreaseBy {
                counter_id: BigInt::from(0),
                inc_by: BigInt::from(5),
            },
            SendOptions::default(),
        )
        .await
        .expect("explicit increase message must send");
    observed.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed"),
    );
    contract
        .send_reset_to(
            &increaser,
            msg_value(),
            &lots_of_messages::ResetTo::<BigInt> {
                counter_id: BigInt::from(0),
                reset_to: BigInt::from(100),
            },
            SendOptions::default(),
        )
        .await
        .expect("reset message must send");
    observed.push(
        contract
            .get_current_counter()
            .await
            .expect("counter getter must succeed"),
    );

    expect![[r#"
        (
            [
                0,
                1,
                6,
                100,
            ],
            [
                TvmTransaction {
                    sender: "deployer",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: false,
                    body: "x{}",
                    opcode: None,
                    deploy: true,
                    success: false,
                    aborted: true,
                    exit_code: Some(
                        63,
                    ),
                    action_result_code: None,
                },
                TvmTransaction {
                    sender: "increaser",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: true,
                    body: "x{123456780000000000000001}",
                    opcode: Some(
                        305419896,
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
                    body: "x{123456780000000000000005}",
                    opcode: Some(
                        305419896,
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
                    body: "x{23456789000000000000000000000064}",
                    opcode: Some(
                        591751049,
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
    .assert_debug_eq(&(
        observed,
        contract
            .provider()
            .transactions()
            .expect("transactions must be readable"),
    ));
}

// Upstream: LotsOfMessages.spec.ts — "createCellOf works"
#[tokio::test]
async fn lots_of_messages_create_cell_of_works() {
    let contract = setup_lots_of_messages().await;

    let c1_1 = lots_of_messages::LotsOfMessages::<TvmContractProvider>::create_cell_of_increase_by(
        &lots_of_messages::IncreaseBy {
            counter_id: BigInt::from(10),
            inc_by: BigInt::from(20),
        },
    )
    .expect("IncreaseBy helper must encode");
    let c1_2 = lots_of_messages::IncreaseBy {
        counter_id: BigInt::from(10),
        inc_by: BigInt::from(20),
    }
    .to_cell()
    .expect("IncreaseBy declaration must encode");

    let c2_1 = lots_of_messages::LotsOfMessages::<TvmContractProvider>::create_cell_of_increase_by(
        &lots_of_messages::IncreaseBy::create(BigInt::from(0)),
    )
    .expect("IncreaseBy helper must apply the ABI default value");
    let c2_2 = lots_of_messages::IncreaseBy::create(BigInt::from(0))
        .to_cell()
        .expect("IncreaseBy declaration must encode");

    let c3_1 = lots_of_messages::LotsOfMessages::<TvmContractProvider>::create_cell_of_reset_to(
        &lots_of_messages::ResetTo::<BigInt> {
            counter_id: BigInt::from(500),
            reset_to: BigInt::from(0),
        },
    )
    .expect("ResetTo<int64> helper must encode");
    let mut expected = CellBuilder::new();
    acton_client::cell::store_fixed_int(
        &mut expected,
        &BigInt::from(lots_of_messages::ResetTo::<BigInt>::PREFIX),
        32,
        false,
    )
    .expect("prefix must encode");
    acton_client::cell::store_fixed_int(&mut expected, &BigInt::from(500), 32, true)
        .expect("counter id must encode");
    acton_client::cell::store_fixed_int(&mut expected, &BigInt::from(0), 64, true)
        .expect("reset value must encode");
    let c3_2 = expected.build().expect("manual ResetTo cell must build");

    expect![[r#"
        (
            (
                true,
                "x{123456780000000A00000014}",
                "x{123456780000000A00000014}",
                true,
                "x{123456780000000000000001}",
                "x{123456780000000000000001}",
                true,
                "x{23456789000001F40000000000000000}",
                "x{23456789000001F40000000000000000}",
            ),
            [
                TvmTransaction {
                    sender: "deployer",
                    sender_matches: true,
                    recipient_matches: true,
                    value: 50000000,
                    bounce: false,
                    body: "x{}",
                    opcode: None,
                    deploy: true,
                    success: false,
                    aborted: true,
                    exit_code: Some(
                        63,
                    ),
                    action_result_code: None,
                },
            ],
        )
    "#]]
    .assert_debug_eq(&(
        (
            c1_1.repr_hash() == c1_2.repr_hash(),
            cell_hex(&c1_1),
            cell_hex(&c1_2),
            c2_1.repr_hash() == c2_2.repr_hash(),
            cell_hex(&c2_1),
            cell_hex(&c2_2),
            c3_1.repr_hash() == c3_2.repr_hash(),
            cell_hex(&c3_1),
            cell_hex(&c3_2),
        ),
        contract
            .provider()
            .transactions()
            .expect("transactions must be readable"),
    ));
}
