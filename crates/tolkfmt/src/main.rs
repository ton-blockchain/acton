use std::fs;
use std::time::Instant;

fn main() {
    // let code = fs::read_to_string("/Users/petrmakhnev/emulator-rs/.jetton/tests/wallet.test.tolk")
    //     .unwrap();
    let code = fs::read_to_string(
        "/Users/petrmakhnev/emulator-rs/.jetton/contracts/jetton-wallet-contract.tolk",
    )
    .unwrap();
    let code = code.as_str();

    // let code = "
    // fun main(
    //     // aaaa
    //     a: int // bbb
    // ) {
    //     assert(
    //         // comment
    //         10 +
    //         20
    //     ) // bbb
    //     throw 20;
    // }
    // ";

    let now = Instant::now();
    let result = tolkfmt::format_source(code, 100).unwrap();
    println!("{}", result);
    println!("tolkfmt took {:?}", now.elapsed());
}
