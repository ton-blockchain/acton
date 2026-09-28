use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tycho_types::boc::Boc;

fn deserialize_boc(id: BenchmarkId, boc: &'static [u8], c: &mut Criterion) {
    c.bench_with_input(id, &boc, |b, boc| {
        b.iter(|| {
            let result = Boc::decode(boc);
            _ = black_box(result);
        });
    });
}

fn serialize_boc(id: BenchmarkId, boc: &'static [u8], c: &mut Criterion) {
    let cell = Boc::decode(boc).unwrap();

    c.bench_with_input(id, &cell, |b, cell| {
        b.iter(|| {
            let result = Boc::encode(cell.as_ref());
            _ = black_box(result);
        });
    });
}

fn boc_group(c: &mut Criterion) {
    macro_rules! decl_boc_benches {
        ($($name:literal),*$(,)?) => {
            $({
                let id = BenchmarkId::new(
                    "deserialize_boc",
                    format!("name={}", $name)
                );
                let boc = include_bytes!(concat!("data/", $name));
                deserialize_boc(id, boc, c);
            });*

            $({
                let id = BenchmarkId::new(
                    "serialize_boc",
                    format!("name={}", $name)
                );
                let boc = include_bytes!(concat!("data/", $name));
                serialize_boc(id, boc, c);
            });*
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
}

criterion_group!(boc, boc_group);
criterion_main!(boc);
