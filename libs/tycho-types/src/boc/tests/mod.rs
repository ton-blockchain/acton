use super::*;
use crate::util::decode_base64;

#[test]
fn big_cell() -> anyhow::Result<()> {
    fn make_big_tree(depth: u8, count: &mut u32, target: u32) -> Cell {
        *count += 1;

        if depth == 0 {
            CellBuilder::build_from(*count).unwrap()
        } else {
            let mut b = CellBuilder::new();
            for _ in 0..4 {
                if *count < target {
                    b.store_reference(make_big_tree(depth - 1, count, target))
                        .unwrap();
                }
            }
            b.build().unwrap()
        }
    }

    let first = make_big_tree(10, &mut 0, 100000);

    let mut cache = ser::BocHeaderCache::<ahash::RandomState>::default();
    let bytes = Boc::encode_with_cache(first.as_ref(), &mut cache);
    let second = Boc::decode(&bytes)?;
    debug_assert_eq!(first.repr_hash(), second.repr_hash());

    Ok(())
}

#[test]
fn boc_with_crc() {
    let boc_without_crc = decode_base64("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").unwrap();

    let cell = Boc::decode(&boc_without_crc).unwrap();

    let mut boc_with_crc = Vec::new();
    ser::BocHeader::<ahash::RandomState>::with_root(cell.as_ref())
        .with_crc(true)
        .encode(&mut boc_with_crc);
    assert_eq!(boc_without_crc.len() + 4, boc_with_crc.len());

    let decoded = Boc::decode(&boc_with_crc).unwrap();
    assert_eq!(decoded.as_ref(), cell.as_ref());

    let last_byte = boc_with_crc.last_mut().unwrap();
    *last_byte = !*last_byte;

    assert!(matches!(
        Boc::decode(&boc_with_crc),
        Err(de::Error::InvalidChecksum)
    ));
}

#[cfg(feature = "serde")]
#[allow(unused)]
#[derive(::serde::Serialize)]
struct SerdeWithCellRef<'a> {
    cell: &'a DynCell,
}

#[cfg(feature = "serde")]
#[derive(::serde::Serialize, ::serde::Deserialize)]
struct SerdeWithHashBytes {
    some_hash: crate::cell::HashBytes,
}

#[cfg(feature = "serde")]
#[derive(::serde::Serialize, ::serde::Deserialize)]
struct SerdeWithCellContainer {
    #[serde(with = "Boc")]
    some_cell: Cell,
}

#[cfg(feature = "serde")]
#[derive(::serde::Serialize, ::serde::Deserialize)]
struct SerdeWithRepr {
    #[serde(with = "BocRepr")]
    dict: crate::dict::RawDict<32>,
    #[serde(with = "BocRepr")]
    tuple: SomeStruct,
}

#[cfg(feature = "serde")]
#[derive(Debug, Eq, PartialEq, Store, Load, ::serde::Serialize, ::serde::Deserialize)]
struct SomeStruct {
    field1: bool,
    field2: HashBytes,
    #[serde(with = "Boc")]
    field3: Cell,
}

#[cfg(feature = "serde")]
#[test]
fn hex_bytes() {
    let hash: crate::cell::HashBytes = rand9::random();

    let test = format!(r#"{{"some_hash":"{hash}"}}"#);
    let SerdeWithHashBytes { some_hash } = serde_json::from_str(&test).unwrap();
    assert_eq!(some_hash, hash);

    let serialized = serde_json::to_string(&SerdeWithHashBytes { some_hash }).unwrap();
    assert_eq!(serialized, test);
}

#[cfg(feature = "serde")]
#[test]
fn struct_with_cell() {
    let boc = "te6ccgEBAQEAWwAAsUgBUkKKaORs1v/d2CpkdS1rueLjL5EbgaivG/SlIBcUZ5cAKkhRTRyNmt/7uwVMjqWtdzxcZfIjcDUV436UpALijPLQ7msoAAYUWGAAAD6o4PtmhMeK8nJA";

    let test = format!(r#"{{"some_cell":"{boc}"}}"#);
    let SerdeWithCellContainer { some_cell } = serde_json::from_str(&test).unwrap();

    let original = Boc::decode_base64(boc).unwrap();
    assert_eq!(some_cell.as_ref(), original.as_ref());
}

#[cfg(feature = "serde")]
#[test]
fn struct_with_repr() {
    let boc_dict =
        "te6ccgEBCAEAMAABAcABAgPPQAUCAgEgBAMACQAAADqgAAkAAABQYAIBIAcGAAkAAAAe4AAJAAAAbCA=";
    let boc_dict_escaped =
        "te6ccgEBC\\u0041EAMAABAcABAgPPQAUCAgEgBAMACQAAADqgAAkAAABQYAIBIAcGAAkAAAAe4AAJAAAAbCA=";

    let boc_tuple = "te6ccgEBAgEAJgABQYAAAAAAAAAAAAAAAAAAAAAAAAAAAAUXneTYztHJnZqRwAEAAA==";

    let test = format!(r#"{{"dict":"{boc_dict_escaped}","tuple":"{boc_tuple}"}}"#);
    let SerdeWithRepr { dict, tuple } = serde_json::from_str(&test).unwrap();

    let boc = Boc::decode_base64(boc_dict).unwrap();
    let orig_dict = boc.parse::<crate::dict::RawDict<32>>().unwrap();
    assert_eq!(dict, orig_dict);

    let boc = Boc::decode_base64(boc_tuple).unwrap();
    let orig_tuple = boc.parse::<SomeStruct>().unwrap();
    assert_eq!(tuple, orig_tuple);
}

#[test]
#[cfg(feature = "rayon")]
fn encode_decode_rayon() {
    use crate::boc::Boc;

    let data = include_bytes!("block.boc.2");
    let cell = Boc::decode(data).unwrap();

    let data_serial = Boc::encode(&cell);
    let data_parallel = Boc::encode_rayon(&cell);

    assert_eq!(data_serial, data_parallel);
    assert_eq!(data.as_slice(), data_serial.as_slice());
}
