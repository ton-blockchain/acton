use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, DynCell, HashBytes, StaticCell, Store};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::models::{StateInit, StdAddr};

pub const MAX_NONCE: u64 = 300;

#[derive(Debug)]
pub struct MinedData {
    pub nonce: u64,
    pub hash: HashBytes,
    pub data: Vec<u8>,
}

fn do_mine(code: &Cell, factory_addr: &StdAddr, recipient: &StdAddr, reward: u128) -> MinedData {
    const KEY_ZERO: &DynCell =
        &unsafe { StaticCell::new(&[0, 0, 0, 0, 0, 0, 0, 0, 0x80], 64, &[0u8; 32]) };
    const KEY_ONE: &DynCell =
        &unsafe { StaticCell::new(&[0, 0, 0, 0, 0, 0, 0, 1, 0x80], 64, &[0u8; 32]) };
    const KEY_TWO: &DynCell =
        &unsafe { StaticCell::new(&[0, 0, 0, 0, 0, 0, 0, 2, 0x80], 64, &[0u8; 32]) };

    let cx = Cell::empty_context();

    let target_bits = factory_addr.address.0[0] >> 4;

    let mut data = RawDict::<64>::new();

    data.set_ext(KEY_ZERO.as_slice_allow_exotic(), &HashBytes::ZERO, cx)
        .unwrap();

    let airdrop_data_child = CellBuilder::build_from_ext((recipient, reward), cx).unwrap();
    let airdrop_data = CellBuilder::build_from_ext((factory_addr, airdrop_data_child), cx).unwrap();

    data.set_ext(KEY_TWO.as_slice_allow_exotic(), &airdrop_data, cx)
        .unwrap();

    let nonce_key = KEY_ONE.as_slice_allow_exotic();

    let mut nonce = 0;
    loop {
        let mut builder = CellBuilder::new();
        builder.store_zeros(192).unwrap();
        builder.store_u64(nonce).unwrap();
        data.set_ext(nonce_key, &builder.as_data_slice(), cx)
            .unwrap();

        let partial_state_init = CellBuilder::build_from_ext(
            StateInit {
                split_depth: None,
                special: None,
                code: Some(code.clone()),
                data: Some(CellBuilder::build_from_ext(&data, cx).unwrap()),
                libraries: Dict::new(),
            },
            cx,
        )
        .unwrap();

        let address = partial_state_init.repr_hash();
        if address.0[0] >> 4 == target_bits || nonce >= MAX_NONCE {
            let mut builder = CellBuilder::new();

            let child = CellBuilder::build_from_ext((recipient, reward), cx).unwrap();

            // uint256 nonce
            builder.store_zeros(192).unwrap();
            builder.store_u64(nonce).unwrap();

            // addr
            factory_addr.store_into(&mut builder, cx).unwrap();
            builder.store_reference(child).unwrap();

            let full_airdrop_data = builder.build_ext(cx).unwrap();

            let hash = *full_airdrop_data.repr_hash();
            let data = Boc::encode(&full_airdrop_data);

            return MinedData { nonce, hash, data };
        }

        nonce += 1;
    }
}

fn mine_address(c: &mut Criterion) {
    let inputs = [
        (
            "0:cf9b339921e0b981f4c0bc3053c4e386f14cf97127ffa297fd93d7de6a561ffa",
            "0:cccc221b25626349429bc14669c6c072c1e238c6fe5d3ea02de707860a8c6d21",
            120000000,
            0,
        ),
        (
            "0:b84ee317b5ddb05002a52a0ffe6a524ccb4c87f9cf35b9a98a1ac623de70f113",
            "0:bbbb83ba6af3bca1a78bf09271b163bbd54736272768c9544faf1efd1049dba6",
            110000000,
            3,
        ),
        (
            "0:da612c822390fdc8b62b14a432d401a73b3247dec38e3747fb9cd665ca4df5bf",
            "0:ddddfdd246302e87bad55712acc50d4d72e55f5636a52741f6a188939e2fe518",
            210000000,
            4,
        ),
        (
            "0:7973aac2a0f1f1048aa4364ab545a1d1c85839f7dac7f16bec6cd4d9a252f92b",
            "0:77775d17834917c7a5d66a59d3dedd2f993d0059d1824c7272854fc808acc574",
            170000000,
            5,
        ),
        (
            "0:b27e8a967935bcb17c1cf8afbaf09d317c1553926c9c772fd53240c0933c1043",
            "0:bbbb83ba6af3bca1a78bf09271b163bbd54736272768c9544faf1efd1049dba6",
            150000000,
            7,
        ),
        (
            "0:b3de1d08c26fe6e35bddc3ac4efc573c33d639fa69195cad243e54e68f2dbdcd",
            "0:bbbb83ba6af3bca1a78bf09271b163bbd54736272768c9544faf1efd1049dba6",
            220000000,
            9,
        ),
        (
            "0:12c9c61c113f292d19d2e7b29c578c0a6e0efaf52fb64d2582aa962b4c4fa908",
            "0:1111160d461226ad45bacafbdf42bd76e33643547c41e2e42bcae19b42b90122",
            100000000,
            10,
        ),
        (
            "0:3d7c3041562f788f00381ddf5a61631b37a7ec13127f89b428ca713f547bb4ad",
            "0:3334dd1d2f96f3907f7332d6974c383f51500130ce1bc00480020bbb52b8dfb3",
            160000000,
            14,
        ),
        (
            "0:387d7b2854430a64560179fcbdf9d3ba0950d6aaf481bd3664630d6211a7a23e",
            "0:3334dd1d2f96f3907f7332d6974c383f51500130ce1bc00480020bbb52b8dfb3",
            180000000,
            15,
        ),
        (
            "0:962a78c7dfa5605327730fe3110eb3caed1310d6715de59ae6d4e6907c0ef2be",
            "0:9999be620c23631cf75b16c53201e3bad9528c17118e9d124b644fb649bfdbad",
            130000000,
            18,
        ),
        (
            "0:8e63172ad2163783e195909a5e81aff8004dbf4357ad98400a4f84d9576d3701",
            "0:8888b8c017f3afa3c16b394959fed181153edb7253c827c6ffd68ddb88560c8e",
            20000000,
            24,
        ),
        (
            "0:21e6c0a5a7a9f287d974601e594262a963f2df755de0714afbed3412698a03f6",
            "0:2222d76864be64b9b872df63a311b1217d1cba3fe687cd107fd29b9b7fe01570",
            190000000,
            28,
        ),
        (
            "0:81de3632ee719ff174a0fe1318c28cbb0c80a8903a112fe4b864a193fbfa2584",
            "0:8888b8c017f3afa3c16b394959fed181153edb7253c827c6ffd68ddb88560c8e",
            250000000,
            30,
        ),
        (
            "0:2f8737396a897945a082390fbb476a3151f2881dbd1cf6180b05fd000bfb205a",
            "0:2222d76864be64b9b872df63a311b1217d1cba3fe687cd107fd29b9b7fe01570",
            140000000,
            37,
        ),
        (
            "0:49d5682a789d0d7820a9a9657df0280c87597e312344cdda08c51dd46996c00a",
            "0:44442bf5dd7b2bd3653537e116a255baff32cfbc7af9f10128f2897eba3efd0b",
            240000000,
            46,
        ),
    ];

    for (wallet_addr, factory_addr, tokens, nonce) in inputs {
        let wallet_addr = wallet_addr.parse::<StdAddr>().unwrap();
        let factory_addr = factory_addr.parse::<StdAddr>().unwrap();

        let mut group = c.benchmark_group("mine");
        group.bench_with_input(BenchmarkId::from_parameter(nonce), &nonce, move |b, _| {
            b.iter(|| {
                thread_local! {
                    static CODE_CELL: std::cell::UnsafeCell<Option<Cell>> = const {
                        std::cell::UnsafeCell::new(None)
                    };
                }

                CODE_CELL.with(|code_cell| {
                    let code_cell = {
                        let slot = unsafe { &mut *code_cell.get() };
                        &*slot.get_or_insert_with(|| {
                            Boc::decode_base64("te6ccgECDgEAAd0ABCSK7VMg4wMgwP/jAiDA/uMC8gsBAgMMAhD0pCD0vfLATgwEA1LtRNDXScMB+GYi0NMD+kAw+GmpOADcIccA4wIh1w0f8rwh4wMB2zzyPAUFBwLA7UTQ10nDAfhmjQhgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAE+Gkh2zzTAAGOH4MI1xgg+CjIzs7J+QAB0wABlNP/AwGTAvhC4vkQ8qiV0wAB8nri0z8BCQYAFHNvbCAwLjcxLjAACvhG8uBMAU74QyG58rQg+COBA+iogggbd0CgufK0+GPTHwH4I7zyudMfAds88jwHARQgghBCHlYVuuMCCAPEMPhCbuMA+EbycyGT1NHQ3vpA03/RW/hL0PpA1NHQ+kDTf9FbIPpCbxPXC//DAPhJWMcFsPLgZNs8cPsC+Ev4SvhJcMjPhYDKAM+EQM6CECeY5YTPC47L/8zJgwb7ANs88gAJCgsCdO1E0NdJwgGOr3DtRND0BXEhgED0DpPXC/+RcOJyIoBA9A+OgYjf+Gv4aoBA9A7yvdcL//hicPhj4w0MDQAKggnJw4AAKvhL+Er4Q/hCyMv/yz/Pg8v/zMntVAAAACztRNDT/9M/0wAx0//U0fhr+Gr4Y/hi").unwrap()
                        })
                    };

                    let res = do_mine(code_cell, &factory_addr, &wallet_addr, tokens);
                    assert_eq!(res.nonce, nonce);
                    black_box( res);
                })
            });
        });
        group.finish();
    }
}

criterion_group!(mine, mine_address);
criterion_main!(mine);
