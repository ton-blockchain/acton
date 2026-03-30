use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=TARGET");

    let target = env::var("TARGET").expect("TARGET env variable not set for xtask build");
    println!("cargo:rustc-env=X_TASK_TARGET={target}");
}
