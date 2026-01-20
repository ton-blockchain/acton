#[path = "../../build/ton_native.rs"]
mod ton_native;

fn main() {
    ton_native::emit_rerun_directives();
    let artifacts = ton_native::ensure_native_libs();

    println!("cargo:rustc-link-search=native={}", artifacts.lib_dir().display());
    println!("cargo:rustc-link-lib=static=emulator");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=dylib=c++");
        println!("cargo:rustc-link-lib=dylib=c++abi");
        println!("cargo:rustc-link-lib=z");
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
