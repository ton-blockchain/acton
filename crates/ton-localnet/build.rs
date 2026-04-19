use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let ui_dir = manifest_dir.join("../acton-localnet-ui");
    let shared_ui_dir = manifest_dir.join("../acton-shared-ui");
    let dist_index = ui_dir.join("dist/index.html");

    track_path(&ui_dir.join("src"));
    track_path(&shared_ui_dir.join("src"));
    track_path(&ui_dir.join("index.html"));
    track_path(&ui_dir.join("package.json"));
    track_path(&ui_dir.join("tsconfig.json"));
    track_path(&ui_dir.join("vite.config.ts"));
    track_path(&manifest_dir.join("../package.json"));
    track_path(&manifest_dir.join("../bun.lock"));
    println!("cargo:rerun-if-env-changed=ACTON_SKIP_LOCALNET_UI_BUILD");

    if env::var_os("ACTON_SKIP_LOCALNET_UI_BUILD").is_some() {
        ensure_dist_exists(&dist_index);
        return;
    }

    match Command::new("bun")
        .args(["run", "build"])
        .current_dir(&ui_dir)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => {
            if !dist_index.exists() {
                panic!("failed to build Localnet UI with `bun run build` (status: {status})");
            }
            println!(
                "cargo:warning=Localnet UI build failed with status {status}; using existing dist/"
            );
        }
        Err(error) => {
            if !dist_index.exists() {
                panic!("failed to run `bun run build` for Localnet UI: {error}");
            }
            println!(
                "cargo:warning=Failed to run `bun run build` for Localnet UI ({error}); using existing dist/"
            );
        }
    }
}

fn ensure_dist_exists(dist_index: &Path) {
    if !dist_index.exists() {
        panic!("ACTON_SKIP_LOCALNET_UI_BUILD is set, but Localnet UI dist/index.html is missing");
    }
}

fn track_path(path: &Path) {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
            .map(|entry| entry.expect("failed to read directory entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            track_path(&entry);
        }
    } else {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
