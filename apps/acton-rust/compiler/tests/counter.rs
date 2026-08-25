use acton_rust_compiler::compile_source;
use expect_test::expect;

#[test]
fn lowers_counter_contract_to_tolk() {
    let source = include_str!("../../examples/counter/src/lib.rs");
    let generated = compile_source(source).expect("counter DSL must compile");

    expect![[r#"
        contract Counter {
            author: "TON Core"
            version: "0.1.0"
            description: "Minimal counter written with the Acton Rust DSL"
            incomingMessages: AllowedMessage
            storage: State
        }

        struct State {
            id: uint32
            counter: uint32
        }

        struct (0x7e8764ef) IncreaseCounter {
            increaseBy: uint32
        }

        struct (0x3a752f06) ResetCounter {
        }

        type AllowedMessage = IncreaseCounter | ResetCounter

        fun State.load(): State {
            return State.fromCell(contract.getData());
        }

        fun State.save(self) {
            contract.setData(self.toCell());
        }

        fun onInternalMessage(in: InMessage) {
            val msg = lazy AllowedMessage.fromSlice(in.body);

            match (msg) {
                IncreaseCounter => {
                    var storage = lazy State.load();
                    storage.counter += msg.increaseBy;
                    storage.save();
                }
                ResetCounter => {
                    var storage = lazy State.load();
                    storage.counter = 0;
                    storage.save();
                }
                else => {
                    assert (in.body.isEmpty()) throw 0xFFFF;
                }
            }
        }

        get fun currentCounter(): uint32 {
            val storage = lazy State.load();
            return storage.counter;
        }

    "#]]
    .assert_eq(&generated);
}
