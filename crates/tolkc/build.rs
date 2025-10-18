use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    println!("cargo:rustc-link-search=native={manifest_dir}/assets/obj/");
    println!("cargo:rustc-link-lib=static=tolkfiftlib");
    println!("cargo:rustc-link-lib=static=fift");
}
