use chrono::Utc;
use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ACTON_RELEASE_CHANNEL");

    let pkg_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION must be set");

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
    println!("cargo:rustc-env=ACTON_LONG_VERSION={short_version} ({git_hash} {build_date})");
}
