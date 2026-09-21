use tycho_types::boc::Boc;

use super::{Fec, Message, SYMBOL_SIZE, Transfer};

#[tokio::test]
async fn recovers_mainnet_answers_with_missing_source_symbols() {
    // Public mainnet RLDP2 answers captured on 2026-09-20. Each record contains
    // a little-endian symbol ID followed by 768 bytes, in arrival order.
    // The missing source symbols must be reconstructed from TON repair symbols;
    // generating both sides with the same codec would hide incompatibilities.
    let cases: &[(&[u8], u32, u32, &str)] = &[
        (
            include_bytes!("fixtures/shard-98285374.symbols"),
            40_984,
            49,
            "b006518e18b46531d2902e4dbf73ea21d4c07b8f39a4bdeb8e4721b701e0b155",
        ),
        (
            include_bytes!("fixtures/shard-98285376.symbols"),
            49_932,
            64,
            "b2271f6a1a5f268db4caf25a32f72e6214f5be7cac21bb401ed034513a6a9779",
        ),
    ];

    for &(symbols, data_size, missing_symbol, expected_hash) in cases {
        let mut transfer = Transfer::new(data_size as usize);
        let fec = Fec {
            data_size,
            symbol_size: SYMBOL_SIZE,
            symbols_count: data_size.div_ceil(SYMBOL_SIZE),
        };

        for record in symbols.chunks_exact(4 + SYMBOL_SIZE as usize) {
            let seqno = u32::from_le_bytes(record[..4].try_into().unwrap());
            assert_ne!(seqno, missing_symbol);

            transfer
                .receive(
                    fec,
                    0,
                    u64::from(data_size),
                    seqno,
                    record[4..].to_vec(),
                    [0; 32],
                )
                .await
                .unwrap();
        }

        let bytes = transfer.finish().expect("complete RLDP2 answer");
        let Message::Answer { data, .. } = tl_proto::deserialize(&bytes).unwrap() else {
            panic!("expected an RLDP2 answer");
        };

        assert_eq!(Boc::file_hash(&data).to_string(), expected_hash);
    }
}
