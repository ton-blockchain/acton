use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let abi_path = manifest_dir.join("../../../../acton-client/tests/fixtures/counter.abi.json");
    println!("cargo:rerun-if-changed={}", abi_path.display());

    let bindings = acton_client_codegen::generate_from_file(&abi_path)
        .expect("Counter ABI must generate Rust bindings");
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    fs::write(output_dir.join("counter.rs"), bindings)
        .expect("generated bindings must be written to OUT_DIR");
}
