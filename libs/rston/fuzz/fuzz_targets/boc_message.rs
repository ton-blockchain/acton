#![no_main]
use libfuzzer_sys::{Corpus, fuzz_target};
use rston::models::Message;
use rston::prelude::Boc;

fuzz_target!(|data: &[u8]| -> Corpus {
    if let Ok(cell) = Boc::decode(data)
        && cell.parse::<Message>().is_ok()
    {
        return Corpus::Keep;
    }
    Corpus::Reject
});
