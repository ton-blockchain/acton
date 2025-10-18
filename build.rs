use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-search=native=/Users/petrmakhnev/emulator-rs/libemulator/");
    // println!("cargo:rustc-link-search=native=/Users/petrmakhnev/emulator-rs/libemulator-opt/");

    pkg_config::Config::new().probe("openssl").unwrap();
    pkg_config::Config::new().probe("libsodium").unwrap();
    pkg_config::Config::new().probe("zlib").unwrap();

    println!("cargo:rustc-link-lib=static=emulator_static");
    println!("cargo:rustc-link-lib=static=smc-envelope");
    println!("cargo:rustc-link-lib=static=tdutils");
    println!("cargo:rustc-link-lib=static=ton_crypto");
    println!("cargo:rustc-link-lib=static=ton_crypto_core");
    println!("cargo:rustc-link-lib=static=ton_block");
    println!("cargo:rustc-link-lib=static=src_parser");
    // Release
    // println!("cargo:rustc-link-lib=static=emulator_static-opt");
    // println!("cargo:rustc-link-lib=static=smc-envelope-opt");
    // println!("cargo:rustc-link-lib=static=tdutils-opt");
    // println!("cargo:rustc-link-lib=static=ton_crypto-opt");
    // println!("cargo:rustc-link-lib=static=ton_crypto_core-opt");
    // println!("cargo:rustc-link-lib=static=ton_block-opt");
    // println!("cargo:rustc-link-lib=static=src_parser-opt");

    println!("cargo:rustc-link-lib=static=absl_hash");
    println!("cargo:rustc-link-lib=static=absl_raw_hash_set");
    println!("cargo:rustc-link-lib=static=absl_hashtablez_sampler");
    println!("cargo:rustc-link-lib=static=absl_low_level_hash");
    println!("cargo:rustc-link-lib=static=absl_base");
    println!("cargo:rustc-link-lib=static=absl_throw_delegate");
    println!("cargo:rustc-link-lib=static=crc32c");
    println!("cargo:rustc-link-lib=static=blst");

    println!("cargo:rustc-link-lib=dylib=ssl");

    println!("cargo:rustc-link-lib=dylib=c++");
    println!("cargo:rustc-link-lib=dylib=c++abi");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
