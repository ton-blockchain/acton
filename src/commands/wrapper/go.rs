use acton_config::config::{ActonConfig, GoWrapperSettings, manifest_path, project_root};
use anyhow::Context;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

const GENERATOR_SOURCES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/go-generator.tar.zst"));
const GO_VERSION: &str = env!("ACTON_ABI_GO_VERSION");

pub fn go_wrapper_cmd(
    contract_id: Option<&str>,
    all: bool,
    catalog: Option<&Path>,
    output_dir: Option<&str>,
    package: Option<&str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        usize::from(contract_id.is_some()) + usize::from(all) + usize::from(catalog.is_some()) == 1,
        "Select exactly one contract name, --all, or --catalog"
    );
    let config = if catalog.is_some() && !manifest_path().exists() {
        ActonConfig::default()
    } else {
        ActonConfig::load_manifest().context("Failed to load Acton.toml")?
    };
    let project_settings = config
        .wrappers
        .as_ref()
        .and_then(|wrappers| wrappers.go.as_ref());
    let contract_settings = contract_id
        .and_then(|id| config.get_contract(id))
        .and_then(|contract| contract.wrappers.as_ref())
        .and_then(|wrappers| wrappers.go.as_ref());
    let (output_dir, package) = resolve_settings(
        project_root(),
        project_settings,
        contract_settings,
        output_dir,
        package,
    );

    // Keep the file alive until Go exits, including on failure. One invocation for
    // the entire selection prevents contracts from replacing the shared registry.
    let mut temporary_catalog;
    let catalog = if let Some(catalog) = catalog {
        catalog
    } else {
        let catalog = super::compile_go_catalog(&config, contract_id)?;
        temporary_catalog =
            NamedTempFile::new().context("Failed to create temporary Go ABI catalog")?;
        serde_json::to_writer(&mut temporary_catalog, &catalog)
            .context("Failed to write temporary Go ABI catalog")?;
        temporary_catalog
            .flush()
            .context("Failed to flush temporary Go ABI catalog")?;
        temporary_catalog.path()
    };
    run_generator(catalog, &output_dir, &package)
}

fn resolve_settings(
    root: &Path,
    project: Option<&GoWrapperSettings>,
    contract: Option<&GoWrapperSettings>,
    output_dir: Option<&str>,
    package: Option<&str>,
) -> (PathBuf, String) {
    let setting = |field: fn(&GoWrapperSettings) -> Option<&str>| {
        contract
            .and_then(field)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| project.and_then(field).filter(|s| !s.trim().is_empty()))
    };
    let output_dir = output_dir.map_or_else(
        || root.join(setting(|s| s.output_dir.as_deref()).unwrap_or("wrappers-go")),
        PathBuf::from,
    );
    let package = package
        .or_else(|| setting(|s| s.package.as_deref()))
        .unwrap_or("wrappers")
        .to_owned();
    (output_dir, package)
}

fn unpack_generator() -> anyhow::Result<TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("acton-go-")
        .tempdir()
        .context("Failed to create temporary Go generator directory")?;
    let decoder = zstd::stream::read::Decoder::new(GENERATOR_SOURCES)
        .context("Failed to decompress bundled Go generator")?;
    tar::Archive::new(decoder)
        .unpack(dir.path())
        .context("Failed to unpack bundled Go generator")?;
    Ok(dir)
}

fn run_generator(catalog: &Path, output_dir: &Path, package: &str) -> anyhow::Result<()> {
    // CLI paths belong to the caller, not to the temporary Go module. Output may
    // not exist yet, so resolve lexically rather than canonicalizing on disk.
    let catalog = std::path::absolute(catalog).context("Failed to resolve catalog path")?;
    let output_dir =
        std::path::absolute(output_dir).context("Failed to resolve Go output directory")?;
    let module = unpack_generator()?;
    let status = Command::new("go")
        .args(["run", "-mod=readonly", "-trimpath", "./cmd/tolk-abi-to-go"])
        .arg("--catalog").arg(&catalog)
        .arg("--output-dir").arg(&output_dir)
        .arg("--package").arg(package)
        .env("CGO_ENABLED", "0")
        .env("GOWORK", "off")
        // Go treats an empty GOFLAGS as unset and falls back to persistent GOENV
        // defaults. A nonempty whitespace value parses as no flags, preventing a
        // caller's -modfile or other build flags from changing this private module.
        .env("GOFLAGS", " ")
        .current_dir(module.path())
        .status()
        .with_context(|| format!(
            "Failed to run Go for Acton's bundled wrapper generator. Install Go {GO_VERSION} or newer from https://go.dev/dl/ and ensure `go` is on PATH. No separate generator installation is required."
        ))?;
    anyhow::ensure!(
        status.success(),
        "Bundled Go wrapper generation failed with {status}. See Go output above for details. The generator requires Go {GO_VERSION} or newer (https://go.dev/dl/)."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_settings_precedence_and_path_resolution() {
        let root = Path::new("/project");
        assert_eq!(
            resolve_settings(root, None, None, None, None),
            (root.join("wrappers-go"), "wrappers".into())
        );
        let project = GoWrapperSettings {
            output_dir: Some("generated go".into()),
            package: Some("codecs".into()),
        };
        let contract = GoWrapperSettings {
            package: Some("counter".into()),
            ..Default::default()
        };
        assert_eq!(
            resolve_settings(root, Some(&project), Some(&contract), None, None),
            (root.join("generated go"), "counter".into())
        );
        assert_eq!(
            resolve_settings(
                root,
                Some(&project),
                Some(&contract),
                Some("cwd/go"),
                Some("override")
            ),
            ("cwd/go".into(), "override".into())
        );
    }

    #[test]
    fn bundled_go_module_contains_only_production_sources() {
        let dir = unpack_generator().unwrap();
        let manifest = std::fs::read_to_string(dir.path().join("go.mod")).unwrap();
        assert!(manifest.contains("module github.com/ton-blockchain/acton/packages/abi-go"));
        assert!(manifest.contains(&format!("go {GO_VERSION}")));
        assert!(dir.path().join("go.sum").is_file());
        assert_eq!(
            std::fs::read(dir.path().join("LICENSE")).unwrap(),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/packages/abi-go/LICENSE"
            ))
        );
        assert!(dir.path().join("cmd/tolk-abi-to-go/main.go").is_file());
        assert!(dir.path().join("codegen/generate.go").is_file());
        for entry in walkdir::WalkDir::new(dir.path())
            .into_iter()
            .map(Result::unwrap)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().strip_prefix(dir.path()).unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(!name.ends_with("_test.go"), "{path:?}");
            assert!(
                name == "go.mod" || name == "go.sum" || name == "LICENSE" || name.ends_with(".go"),
                "{path:?}"
            );
            assert!(
                [
                    Path::new(""),
                    Path::new("codegen"),
                    Path::new("cmd/tolk-abi-to-go")
                ]
                .contains(&path.parent().unwrap()),
                "{path:?}"
            );
        }
    }
}
