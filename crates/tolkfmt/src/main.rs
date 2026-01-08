use std::time::Instant;

fn main() {
    // let code = fs::read_to_string("/Users/petrmakhnev/emulator-rs/.jetton/tests/wallet.test.tolk")
    //     .unwrap();
    // let code = fs::read_to_string(
    //     "/Users/petrmakhnev/emulator-rs/.jetton/contracts/jetton-wallet-contract.tolk",
    // )
    // .unwrap();
    // let code = code.as_str();

    let code = "
// first comment
    // second comment
    fun foo() {

}
    ";

    let now = Instant::now();
    let result = tolkfmt::format_source(code, 100).unwrap();
    println!("{}", result);
    println!("tolkfmt took {:?}", now.elapsed());
}
