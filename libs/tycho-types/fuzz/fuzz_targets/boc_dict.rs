#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};
use tycho_types::prelude::{Boc, RawDict};

fuzz_target!(|data: &[u8]| -> Corpus {
    if let Ok(cell) = Boc::decode(data)
        && let Ok(map) = cell.parse::<RawDict<32>>()
    {
        _ = map.iter().count();
        return Corpus::Keep;
    }
    Corpus::Reject
});
