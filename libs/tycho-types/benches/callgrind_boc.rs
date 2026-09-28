use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[macro_export]
macro_rules! decl_boc_benches {
    ($(
        $name:literal
    ),* $(,)?) => {
        // generate setup functions for raw bytes
        $(
            paste::paste! {
                fn [<$name _setup>]() -> &'static [u8] {
                     include_bytes!(concat!("data/", $name))
                }
            }
        )*

        // generate setup functions for cells
        $(
            paste::paste! {
                fn [<$name _setup_de>]() -> Cell {
                    let bytes = include_bytes!(concat!("data/", $name));
                    Boc::decode(&bytes).unwrap()
                }
            }
        )*

        // generate benchmark functions attributes for decode / encode
        paste::paste! {
            #[library_benchmark]
            $(
                #[bench::[<$name>](setup = [<$name _setup>])]
            )*
            fn deserialize_boc(input: &[u8]) {
                let result = Boc::decode(input);
                _ = black_box(result);
            }

            #[library_benchmark]
            $(
                #[bench::[<$name>](setup = [<$name _setup_de>])]
            )*
            fn serialize_boc(input: Cell) {
                let result = Boc::encode(&input);
                _ = black_box(result);
                std::mem::forget(input);
            }
        }
    };
}

decl_boc_benches![
    "external_message",
    "internal_message_empty",
    "internal_message_with_body",
    "internal_message_with_deploy",
    "masterchain_block",
    "masterchain_key_block",
    "shard_block_empty",
    "shard_block_with_messages",
    "masterchain_block_proof",
    "shard_block_proof"
];

library_benchmark_group!(
    name = benches;
    benchmarks =
        deserialize_boc,
        serialize_boc
);

main!(library_benchmark_groups = benches);
