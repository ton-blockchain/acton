//! Decode generated metadata through TEP-64's public cell layout, including long
//! image values that span several cells. No running node is needed for this check.

#[path = "../src/activity/metadata.rs"]
mod metadata;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use expect_test::expect;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tycho_types::{
    cell::{Cell, HashBytes},
    dict::Dict,
};

fn field(attributes: &Dict<HashBytes, Cell>, name: &str) -> String {
    let mut cell = attributes
        .get(HashBytes(Sha256::digest(name.as_bytes()).into()))
        .expect("metadata dictionary")
        .expect("metadata field");
    let mut bytes = Vec::new();
    loop {
        let mut slice = cell.as_slice().expect("ordinary cell");
        let length = slice.size_bits();
        let mut chunk = vec![0; usize::from(length / 8)];
        slice
            .load_raw(&mut chunk, length)
            .expect("byte-aligned snake");
        bytes.extend(chunk);
        if slice.size_refs() == 0 {
            break;
        }
        expect![["1"]].assert_eq(&slice.size_refs().to_string());
        cell = slice.load_reference_cloned().expect("continuation");
    }
    expect![["0"]].assert_eq(&bytes.remove(0).to_string());
    String::from_utf8(bytes).expect("UTF-8 metadata")
}

#[test]
fn generated_assets_have_varied_names_symbols_and_complete_inline_art() {
    let mut names = HashSet::new();
    let mut symbols = HashSet::new();
    let mut images = HashSet::new();

    for _ in 0..32 {
        for (asset, token) in [
            (metadata::Asset::jetton(), true),
            (metadata::Asset::collection(), false),
            (metadata::Asset::item(), false),
        ] {
            let cell = asset.cell().expect("metadata cell");
            let (prefix, attributes): (u8, Dict<HashBytes, Cell>) =
                cell.parse().expect("TEP-64 content");
            expect![["0"]].assert_eq(&prefix.to_string());
            names.insert(field(&attributes, "name"));
            let image = field(&attributes, "image");
            let encoded = image
                .strip_prefix("data:image/svg+xml;base64,")
                .expect("inline SVG");
            let svg = String::from_utf8(STANDARD.decode(encoded).expect("base64 image"))
                .expect("SVG text");
            expect![["true:true:true"]].assert_eq(&format!(
                "{}:{}:{}",
                svg.starts_with("<svg "),
                svg.ends_with("</svg>"),
                svg.len() > 127,
            ));
            images.insert(image);

            if token {
                let symbol = field(&attributes, "symbol");
                expect![["true:5:9"]].assert_eq(&format!(
                    "{}:{}:{}",
                    symbol.bytes().all(|byte| byte.is_ascii_uppercase()),
                    symbol.len(),
                    field(&attributes, "decimals"),
                ));
                symbols.insert(symbol);
            }
        }
    }

    expect![["names vary: true, symbols vary: true, images vary: true"]].assert_eq(&format!(
        "names vary: {}, symbols vary: {}, images vary: {}",
        names.len() > 1,
        symbols.len() > 1,
        images.len() > 1,
    ));
}
