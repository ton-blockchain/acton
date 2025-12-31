use std::env;
use std::path::Path;

fn main() {
    let workspace_root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join("..")
        .join("objs");

    println!(
        "cargo:rustc-link-search=native={}",
        workspace_root.display()
    );
    println!(
        "cargo:rerun-if-changed={}/libtolk.a",
        workspace_root.display()
    );
    println!("cargo:rustc-link-lib=static=tolk");
}
