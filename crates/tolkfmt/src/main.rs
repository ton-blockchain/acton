use std::fs;
use std::time::Instant;

fn main() {
    // let code = fs::read_to_string("/Users/petrmakhnev/emulator-rs/.jetton/tests/wallet.test.tolk")
    //     .unwrap();
    // let code = fs::read_to_string(
    //     "/Users/petrmakhnev/emulator-rs/crates/tolkc/assets/tolk-stdlib/common.tolk",
    // )
    // .unwrap();
    // let code = code.as_str();

    let code = "
struct CreateExternalLogMessageOptions<TBody = void> {
    /// destination is either an external address or a pattern to calculate it
    dest:     | any_address // either some valid external/none address (not internal!)
        | builder // ... or a manually constructed builder with a valid external address
        | ExtOutLogBucket // ... or encode topic/eventID in destination
    /// body is any serializable object (or just miss this field for empty body)
    body: TBody
}
    ";

    let now = Instant::now();
    let result = tolkfmt::format_source(code, 100).unwrap();
    println!("{}", result);
    println!("tolkfmt took {:?}", now.elapsed());
}
