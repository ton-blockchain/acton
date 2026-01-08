use std::fs;
use std::time::Instant;

fn main() {
    // let code = fs::read_to_string("/Users/petrmakhnev/emulator-rs/.jetton/tests/wallet.test.tolk")
    //     .unwrap();
    // let code = fs::read_to_string(
    //     "/Users/petrmakhnev/emulator-rs/.jetton/contracts/jetton-minter-contract.tolk",
    // )
    // .unwrap();
    // let code = code.as_str();

    let code = "
        fun foo() {
            foo(
                a, // comment 1
                bb, 
                ccc // comment 3
            );
        }
    ";

    let now = Instant::now();
    let result = tolkfmt::format_source(code, 100).unwrap();
    println!("{}", result);
    fs::write("out.tolk", result).unwrap();
    println!("tolkfmt took {:?}", now.elapsed());
}
