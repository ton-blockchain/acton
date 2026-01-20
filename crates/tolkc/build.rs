#[path = "../../build/ton_native.rs"]
mod ton_native;

fn main() {
    ton_native::emit_rerun_directives();
    let artifacts = ton_native::ensure_native_libs();

    println!("cargo:rustc-link-search=native={}", artifacts.lib_dir().display());
    println!("cargo:rustc-link-lib=static=tolk");
}
