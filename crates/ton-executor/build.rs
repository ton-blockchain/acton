use std::env;
use std::path::Path;

fn main() {
    let workspace_root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join("..");
    let objc_dir = workspace_root.join("objs");

    println!("cargo:rustc-link-search=native={}", objc_dir.display());
    println!(
        "cargo:rerun-if-changed={}/libemulator.a",
        objc_dir.display()
    );
    println!("cargo:rustc-link-lib=static=emulator");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=dylib=c++");
        println!("cargo:rustc-link-lib=dylib=c++abi");
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=dylib=stdc++");

        pkg_config::Config::new()
            .atleast_version("3.0.0")
            .probe("openssl")
            .expect("OpenSSL not found via pkg-config. Install pkg-config and libssl-dev.");
    }
}
