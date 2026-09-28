#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};
use tycho_types::models::Message;
use tycho_types::prelude::Boc;

fuzz_target!(|data: &[u8]| -> Corpus {
    if let Ok(cell) = Boc::decode(data)
        && cell.parse::<Message>().is_ok()
    {
        return Corpus::Keep;
    }
    Corpus::Reject
});
