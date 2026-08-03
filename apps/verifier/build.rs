use chrono::Utc;
use std::env;
use std::process::Command;

fn main() {
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

    println!("cargo:rustc-env=VERIFIER_LONG_VERSION={pkg_version} ({git_hash} {build_date})");
}
