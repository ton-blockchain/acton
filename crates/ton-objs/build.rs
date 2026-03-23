use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{env, fs};
use toml::Value;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMULATOR");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TOLK");

    let manifest_path = artifacts_manifest_path();
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let link_emulator = feature_enabled("EMULATOR");
    let link_tolk = feature_enabled("TOLK");
    if !link_emulator && !link_tolk {
        return;
    }

    let objs_dir = objs_dir();
    println!("cargo:rustc-link-search=native={}", objs_dir.display());

    if link_emulator {
        link_static_archive(&objs_dir, &manifest_path, "emulator");
        link_emulator_deps();
    }

    if link_tolk {
        link_static_archive(&objs_dir, &manifest_path, "tolk");
    }
}

fn feature_enabled(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

fn manifest_dir() -> PathBuf {
    env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("CARGO_MANIFEST_DIR env variable not set")
}

fn artifacts_manifest_path() -> PathBuf {
    manifest_dir().join("artifacts_manifest.toml")
}

fn objs_dir() -> PathBuf {
    manifest_dir().join("..").join("..").join("objs")
}

fn link_static_archive(objs_dir: &Path, manifest_path: &Path, lib_name: &str) {
    verify_archive_sha(objs_dir, manifest_path, lib_name);

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

fn get_lib_filename(lib_name: &str) -> String {
    match env::var("CARGO_CFG_TARGET_ENV").ok().as_deref() {
        Some("msvc") => format!("{lib_name}.lib"),
        _ => format!("lib{lib_name}.a"),
    }
}

fn verify_archive_sha(objs_dir: &Path, manifest_path: &Path, lib_name: &str) {
    let lib_filename = get_lib_filename(lib_name);
    let lib_sha256_key = format!("lib{lib_name}");

    let expected_sha256 = load_lib_sha256_from_manifest(manifest_path, &lib_sha256_key);

    let archive_path = objs_dir.join(lib_filename);
    let actual_sha256 = sha256_hex(&archive_path);

    if actual_sha256 != expected_sha256 {
        panic!(
            "SHA-256 mismatch for {}: expected {}, got {}. Refresh {} if the archive update was intentional.",
            archive_path.display(),
            expected_sha256,
            actual_sha256,
            manifest_path.display()
        );
    }
}

fn load_lib_sha256_from_manifest(manifest_path: &Path, lib_name: &str) -> String {
    let contents = fs::read_to_string(manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
    let value: Value = toml::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", manifest_path.display()));

    let sha256 = value
        .get("sha256")
        .and_then(Value::as_table)
        .unwrap_or_else(|| panic!("missing table `sha256` in {}", manifest_path.display()));

    let value = sha256
        .get(lib_name)
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "missing string `sha256.{lib_name}` in {}",
                manifest_path.display()
            )
        })
        .trim();

    value.to_owned()
}

fn sha256_hex(path: &Path) -> String {
    let bytes = fs::read(path)
        .unwrap_or_else(|err| panic!("failed to read {} for SHA-256: {err}", path.display()));

    format!("{:x}", Sha256::digest(bytes))
}
