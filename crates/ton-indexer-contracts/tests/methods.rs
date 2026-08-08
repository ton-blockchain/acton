use std::fmt::Write;

use expect_test::expect;
use ton_indexer_contracts::known_get_methods::{
    KNOWN_GET_METHODS, known_get_method_name, known_get_method_names,
};
use ton_indexer_contracts::methods::parse_contract_methods;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};

fn render_result(code: &Cell) -> String {
    match parse_contract_methods(code) {
        Ok(method_ids) => format!("methods: {method_ids:?}"),
        Err(error) => format!("error: {error}"),
    }
}

#[test]
fn parses_method_dictionary_and_rejects_other_dispatchers() {
    let contracts = [
        (
            "discoverable jetton minter",
            include_bytes!(
                "../../../tests/integration/testdata/disasm_reference/jetton_minter_discoverable_JettonMinter.boc"
            )
            .as_slice(),
        ),
        (
            "jetton v1 master",
            include_bytes!(
                "../../../tests/integration/testdata/toncenter_v3_actions/contracts/JettonV1Master.boc"
            )
            .as_slice(),
        ),
        (
            "jetton v1 wallet",
            include_bytes!(
                "../../../tests/integration/testdata/toncenter_v3_actions/contracts/JettonV1Wallet.boc"
            )
            .as_slice(),
        ),
        (
            "vesting wallet",
            include_bytes!(
                "../../../tests/integration/testdata/toncenter_v3_actions/contracts/WalletVesting.boc"
            )
            .as_slice(),
        ),
    ];

    let mut invalid_setcp = CellBuilder::new();
    invalid_setcp
        .store_u8(0xfe)
        .expect("invalid opcode must fit");
    invalid_setcp.store_u8(0).expect("codepage must fit");
    let invalid_setcp = invalid_setcp
        .build()
        .expect("invalid SETCP test cell must be structurally valid");

    let mut invalid_dictpush = CellBuilder::new();
    invalid_dictpush
        .store_u8(0xff)
        .expect("SETCP opcode must fit");
    invalid_dictpush.store_u8(0).expect("codepage must fit");
    invalid_dictpush
        .store_uint(0, 13)
        .expect("invalid dictionary opcode must fit");
    invalid_dictpush
        .store_bit(true)
        .expect("dictionary marker must fit");
    let invalid_dictpush = invalid_dictpush
        .build()
        .expect("invalid DICTPUSHCONST test cell must be structurally valid");

    let mut actual = String::new();
    for (name, boc) in contracts {
        let code = Boc::decode(boc).expect("real contract fixture must contain a valid code cell");
        writeln!(actual, "{name}: {}", render_result(&code))
            .expect("writing to String must succeed");
    }
    writeln!(actual, "{}", render_result(&invalid_setcp)).expect("writing to String must succeed");
    writeln!(actual, "{}", render_result(&invalid_dictpush))
        .expect("writing to String must succeed");

    expect![[r"
        discoverable jetton minter: error: SETCP0 is not followed by DICTPUSHCONST
        jetton v1 master: methods: [0, 8, 103289, 106029]
        jetton v1 wallet: methods: [0, 1, 8, 9, 10, 11, 97026]
        vesting wallet: methods: [0, 78748, 81467, 82536, 85143, 85425, 107618, 120902, 524287]
        error: contract code does not start with SETCP0
        error: SETCP0 is not followed by DICTPUSHCONST
    "]]
    .assert_eq(&actual);
}

#[test]
fn looks_up_known_get_method_names() {
    let sorted = KNOWN_GET_METHODS.windows(2).all(|pair| pair[0] <= pair[1]);
    let actual = format!(
        "entries: {}\nsorted: {sorted}\nfirst: {:?}\nlast: {:?}\ncollision: {:?}\nsingle: {:?}\nunknown: {:?}\n",
        KNOWN_GET_METHODS.len(),
        KNOWN_GET_METHODS.first(),
        KNOWN_GET_METHODS.last(),
        known_get_method_names(76_407),
        known_get_method_name(103_289),
        known_get_method_name(65_536),
    );

    expect![[r#"
        entries: 405
        sorted: true
        first: Some((65842, "get_governance_contract"))
        last: Some((131036, "get_loan_state"))
        collision: [(76407, "is_plugin_installed"), (76407, "version")]
        single: Some("get_wallet_address")
        unknown: None
    "#]]
    .assert_eq(&actual);
}
