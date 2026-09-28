#![no_main]
use libfuzzer_sys::fuzz_target;
use tycho_types::prelude::{Boc, Cell};

fuzz_target!(|cell: Cell| {
    _ = Boc::encode(cell);
});
