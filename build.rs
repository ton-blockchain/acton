use chrono::Utc;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ACTON_RELEASE_CHANNEL");

    compress_man();
    compress_project_templates();
    compress_go_generator();
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

fn compress_go_generator() {
    let module_dir = Path::new("packages/abi-go");
    let manifest = fs::read_to_string(module_dir.join("go.mod"))
        .expect("packages/abi-go/go.mod is required to bundle Acton's Go generator");
    let go_version = manifest
        .lines()
        .find_map(|line| line.strip_prefix("go "))
        .expect("packages/abi-go/go.mod must declare a Go version")
        .trim();
    println!("cargo:rustc-env=ACTON_ABI_GO_VERSION={go_version}");

    // Only these production packages are shipped. Do not recursively archive the
    // module: testdata, generated catalogs, and local build output can be very large.
    let mut files = vec![
        module_dir.join("go.mod"),
        module_dir.join("go.sum"),
        module_dir.join("LICENSE"),
    ];
    for package in ["", "codegen", "cmd/tolk-abi-to-go"] {
        let dir = module_dir.join(package);
        // Watching each directory catches additions and deletions as well as edits.
        println!("cargo:rerun-if-changed={}", dir.display());
        for entry in fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        {
            let entry = entry.expect("failed to read Go generator package entry");
            let path = entry.path();
            if entry.file_type().expect("Go source file type").is_file()
                && path.extension() == Some(OsStr::new("go"))
                && !entry.file_name().to_string_lossy().ends_with("_test.go")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let out_path =
        Path::new(&env::var("OUT_DIR").expect("OUT_DIR must be set")).join("go-generator.tar.zst");
    let dst = fs::File::create(out_path).expect("failed to create Go generator archive");
    let encoder =
        zstd::stream::write::Encoder::new(dst, 19).expect("failed to create Go generator encoder");
    let mut archive = tar::Builder::new(encoder);
    archive.mode(tar::HeaderMode::Deterministic);
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        archive
            .append_path_with_name(
                &path,
                path.strip_prefix(module_dir).expect("Go module path"),
            )
            .expect("failed to append Go generator source");
    }
    archive
        .into_inner()
        .expect("failed to finish Go generator archive")
        .finish()
        .expect("failed to finish Go generator compression");
}

fn compress_project_templates() {
    const TEMPLATE_NAMES: &[&str] = &[
        "empty",
        "empty-app",
        "counter",
        "counter-app",
        "jetton",
        "jetton-app",
        "nft",
        "nft-app",
        "w5-extension",
        "w5-extension-app",
    ];

    let templates_dir = Path::new("src/commands/new/templates");
    println!("cargo:rerun-if-changed={}", templates_dir.display());

    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR must be set"))
        .join("project-templates.tar.zst");
    let dst = fs::File::create(out_path).expect("failed to create project templates archive");
    let encoder = zstd::stream::write::Encoder::new(dst, 19)
        .expect("failed to create project templates encoder");
    let mut archive = tar::Builder::new(encoder);
    archive.mode(tar::HeaderMode::Deterministic);

    for template_name in TEMPLATE_NAMES {
        let template_dir = templates_dir.join(template_name);
        let mut files = Vec::new();
        collect_files(&template_dir, &mut files);
        files.sort();

        for path in files {
            println!("cargo:rerun-if-changed={}", path.display());
            let archive_path = path
                .strip_prefix(templates_dir)
                .expect("template must be inside templates directory");
            archive
                .append_path_with_name(&path, archive_path)
                .expect("failed to append project template file");
        }
    }

    let encoder = archive
        .into_inner()
        .expect("failed to finish project templates archive");
    encoder
        .finish()
        .expect("failed to finish project templates compression");
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));

    for entry in entries {
        let path = entry.expect("failed to read template entry").path();
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
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
