use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMULATOR");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TOLK");

    let objs_dir = objs_dir();
    let link_emulator = feature_enabled("EMULATOR");
    let link_tolk = feature_enabled("TOLK");

    if link_emulator {
        link_static_archive(&objs_dir, "emulator");
        link_emulator_deps();
    }

    if link_tolk {
        link_static_archive(&objs_dir, "tolk");
    }
}

fn feature_enabled(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

fn objs_dir() -> PathBuf {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("CARGO_MANIFEST_DIR env variable not set");

    let Some(workspace_root) = manifest_dir.ancestors().nth(2) else {
        panic!("crate manifest directory must be nested under <workspace>/crates/<crate>");
    };

    workspace_root.join("objs")
}

fn link_static_archive(objs_dir: &Path, lib_name: &str) {
    println!("cargo:rustc-link-search=native={}", objs_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        objs_dir.join(format!("lib{lib_name}.a")).display()
    );
    println!("cargo:rustc-link-lib=static={lib_name}");
}

fn link_emulator_deps() {
    match env::var("CARGO_CFG_TARGET_OS").ok().as_deref() {
        Some("macos") => {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=dylib=c++abi");
        }
        Some("linux") => {
            println!("cargo:rustc-link-lib=dylib=stdc++");

            pkg_config::Config::new()
                .atleast_version("3.0.0")
                .probe("openssl")
                .expect("OpenSSL not found via pkg-config. Install pkg-config and libssl-dev.");
        }
        _ => {}
    }
}
