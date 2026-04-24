use chrono::Utc;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

const CARGO_TOML_PATH: &str = "Cargo.toml";
const TOLK_VERSION_METADATA_PATH: &str = "workspace.metadata.acton.tolk-version";

fn main() {
    println!("cargo:rerun-if-env-changed=ACTON_RELEASE_CHANNEL");
    println!("cargo:rerun-if-changed={CARGO_TOML_PATH}");

    compress_man();
    let pkg_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION must be set");
    let tolk_version = read_tolk_version();

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .expect("failed to execute git");

    let git_hash = String::from_utf8(output.stdout).expect("git output not utf8");
    let git_hash = git_hash.trim();

    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    let build_date = Utc::now().format("%Y-%m-%d").to_string();
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    let target_triple = env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());
    println!("cargo:rustc-env=TARGET_TRIPLE={target_triple}");

    let build_profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={build_profile}");

    let release_channel = match env::var("ACTON_RELEASE_CHANNEL") {
        Ok(value) if value == "trunk" => "trunk",
        Ok(value) if value == "stable" || value.is_empty() => "stable",
        Ok(value) => panic!("Unsupported ACTON_RELEASE_CHANNEL value: {value}"),
        Err(_) => "stable",
    };
    println!("cargo:rustc-env=ACTON_RELEASE_CHANNEL={release_channel}");

    let is_trunk_build = if release_channel == "trunk" { "1" } else { "0" };
    println!("cargo:rustc-env=ACTON_IS_TRUNK_BUILD={is_trunk_build}");

    let short_version = if release_channel == "trunk" {
        format!("{pkg_version}-trunk")
    } else {
        pkg_version
    };
    println!("cargo:rustc-env=ACTON_SHORT_VERSION={short_version}");
    println!("cargo:rustc-env=TOLK_VERSION={tolk_version}");
    println!(
        "cargo:rustc-env=ACTON_LONG_VERSION={short_version} ({git_hash} {build_date}) with Tolk {tolk_version}"
    );
}

fn read_tolk_version() -> String {
    let contents = fs::read_to_string(CARGO_TOML_PATH)
        .unwrap_or_else(|err| panic!("failed to read {CARGO_TOML_PATH}: {err}"));
    let document = contents
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_else(|err| panic!("failed to parse {CARGO_TOML_PATH}: {err}"));
    let version = document["workspace"]["metadata"]["acton"]["tolk-version"]
        .as_str()
        .unwrap_or_else(|| panic!("missing string `{TOLK_VERSION_METADATA_PATH}`"));

    validate_exact_semver(version);

    version.to_owned()
}

fn validate_exact_semver(version: &str) {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3 {
        panic!("{TOLK_VERSION_METADATA_PATH} must contain an exact X.Y.Z version, got `{version}`");
    }

    if parts.iter().any(|part| part.is_empty()) {
        panic!(
            "{TOLK_VERSION_METADATA_PATH} must not contain empty version parts, got `{version}`"
        );
    }

    if parts
        .iter()
        .any(|part| part.len() > 1 && part.starts_with('0'))
    {
        panic!("{TOLK_VERSION_METADATA_PATH} must not contain leading zeroes, got `{version}`");
    }

    if parts
        .iter()
        .any(|part| !part.chars().all(|char| char.is_ascii_digit()))
    {
        panic!(
            "{TOLK_VERSION_METADATA_PATH} must contain only numeric version parts, got `{version}`"
        );
    }
}

fn compress_man() {
    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR must be set")).join("man.tgz");
    let dst = fs::File::create(out_path).expect("failed to create manual archive");
    let encoder = flate2::GzBuilder::new()
        .filename("man.tar")
        .write(dst, flate2::Compression::best());
    let mut ar = tar::Builder::new(encoder);
    ar.mode(tar::HeaderMode::Deterministic);

    let mut add_files = |dir: &Path, extension: &OsStr| {
        println!("cargo:rerun-if-changed={}", dir.display());

        let mut files = fs::read_dir(dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
            .map(|entry| entry.expect("failed to read manual entry").path())
            .collect::<Vec<_>>();
        files.sort();

        for path in files {
            if path.extension() != Some(extension) {
                continue;
            }

            println!("cargo:rerun-if-changed={}", path.display());
            ar.append_path_with_name(&path, path.file_name().expect("manual file name"))
                .expect("failed to append manual file");
        }
    };

    add_files(Path::new("src/etc/man"), OsStr::new("1"));
    add_files(Path::new("src/doc/man/generated_txt"), OsStr::new("txt"));

    let encoder = ar.into_inner().expect("failed to finish tar archive");
    encoder.finish().expect("failed to finish manual archive");
}
