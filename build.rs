use chrono::Utc;
use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .expect("failed to execute git");

    let git_hash = String::from_utf8(output.stdout).expect("git output not utf8");
    let git_hash = git_hash.trim();

    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    let build_date = Utc::now().format("%Y-%m-%d").to_string();
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    let target_triple = std::env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());
    println!("cargo:rustc-env=TARGET_TRIPLE={target_triple}");

    let build_profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={build_profile}");
}
