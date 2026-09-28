#![no_main]
use libfuzzer_sys::fuzz_target;
use rston::prelude::{Boc, Cell};

fuzz_target!(|cell: Cell| {
    _ = Boc::encode(cell);
});
