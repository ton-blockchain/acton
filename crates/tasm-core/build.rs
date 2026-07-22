use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=spec/tvm-specification.json");

    let raw_json =
        fs::read("spec/tvm-specification.json").expect("failed to read bundled TVM spec JSON");
    let compressed =
        zstd::stream::encode_all(&raw_json[..], 19).expect("failed to compress TVM spec JSON");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    fs::write(out_dir.join("tvm-specification.json.zst"), compressed)
        .expect("failed to write compressed TVM spec JSON");
}
