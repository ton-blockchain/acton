use std::env;
use std::path::Path;

fn main() {
    let workspace_root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join("..");
    let objc_dir = workspace_root.join("objs");

    println!("cargo:rustc-link-search=native={}", objc_dir.display());
    println!("cargo:rerun-if-changed={}/libtolk.a", objc_dir.display());
    println!("cargo:rustc-link-lib=static=tolk");
}
