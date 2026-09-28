#![no_main]
use libfuzzer_sys::fuzz_target;
use tycho_types::prelude::Boc;

fuzz_target!(|data: &[u8]| {
    if let Ok(cell) = Boc::decode(data) {
        _ = Boc::encode(cell.as_ref());
    }
});
