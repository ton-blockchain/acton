use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=data/data-abis.json");

    let raw_json =
        fs::read("data/data-abis.json").expect("failed to read bundled ABI catalog JSON");
    let compressed =
        zstd::stream::encode_all(&raw_json[..], 19).expect("failed to compress ABI catalog JSON");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    fs::write(out_dir.join("data-abis.json.zst"), compressed)
        .expect("failed to write compressed ABI catalog JSON");
}
