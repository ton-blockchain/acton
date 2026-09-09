use acton_config::config::{ActonConfig, GoWrapperSettings, manifest_path, project_root};
use anyhow::Context;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

pub fn go_wrapper_cmd(
    contract_id: Option<&str>,
    all: bool,
    catalog: Option<&Path>,
    output_dir: Option<&str>,
    package: Option<&str>,
    generator: Option<&Path>,
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
    let (output_dir, package, generator) = resolve_settings(
        project_root(),
        project_settings,
        contract_settings,
        output_dir,
        package,
        generator,
    );

    // Keep the file alive until the generator exits, including on failure. One invocation
    // for the whole selection prevents each contract from replacing the shared registry.
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
    run_generator(&generator, catalog, &output_dir, &package)
}

fn resolve_settings(
    root: &Path,
    project: Option<&GoWrapperSettings>,
    contract: Option<&GoWrapperSettings>,
    output_dir: Option<&str>,
    package: Option<&str>,
    generator: Option<&Path>,
) -> (PathBuf, String, PathBuf) {
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
    let generator = generator.map_or_else(
        || {
            let name = setting(|s| s.generator.as_deref()).unwrap_or("tolk-abi-to-go");
            let path = Path::new(name);
            if path.components().count() > 1 {
                root.join(path)
            } else {
                path.to_path_buf()
            }
        },
        Path::to_path_buf,
    );
    (output_dir, package, generator)
}

fn run_generator(
    generator: &Path,
    catalog: &Path,
    output_dir: &Path,
    package: &str,
) -> anyhow::Result<()> {
    let status = Command::new(generator)
        .arg("--catalog").arg(catalog)
        .arg("--output-dir").arg(output_dir)
        .arg("--package").arg(package)
        .status()
        .with_context(|| format!(
            "Failed to execute Go wrapper generator {}. Put `tolk-abi-to-go` on PATH or set --go-generator / [wrappers.go].generator to an existing executable. Install a published, pinned revision with `go install github.com/toncenter/ton-indexer/ton-index-go/index/acton/cmd/tolk-abi-to-go@<version>` (replace <version> with a commit revision containing the generator, not an assumed Acton release). For an unpublished checkout, build the generator locally and pass its executable path. Acton does not download it automatically.",
            generator.display()
        ))?;
    anyhow::ensure!(
        status.success(),
        "Go wrapper generator {} failed with {status}",
        generator.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_settings_precedence_and_path_resolution() {
        let root = Path::new("/project");
        let defaults = resolve_settings(root, None, None, None, None, None);
        assert_eq!(
            defaults,
            (
                root.join("wrappers-go"),
                "wrappers".into(),
                "tolk-abi-to-go".into()
            )
        );
        let project = GoWrapperSettings {
            output_dir: Some("generated go".into()),
            package: Some("codecs".into()),
            generator: Some("./bin/my generator".into()),
        };
        let contract = GoWrapperSettings {
            package: Some("counter".into()),
            ..Default::default()
        };
        let configured = resolve_settings(root, Some(&project), Some(&contract), None, None, None);
        assert_eq!(
            configured,
            (
                root.join("generated go"),
                "counter".into(),
                root.join("./bin/my generator")
            )
        );
        let cli = resolve_settings(
            root,
            Some(&project),
            Some(&contract),
            Some("cwd/go"),
            Some("override"),
            Some(Path::new("./local tool")),
        );
        assert_eq!(
            cli,
            ("cwd/go".into(), "override".into(), "./local tool".into())
        );
    }

    #[test]
    fn go_missing_generator_is_actionable() {
        let dir = tempfile::tempdir().unwrap();
        let error = run_generator(
            &dir.path().join("missing"),
            Path::new("catalog.json"),
            dir.path(),
            "wrappers",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("--go-generator"));
        assert!(error.contains("go install github.com/toncenter/ton-indexer/ton-index-go/index/acton/cmd/tolk-abi-to-go@<version>"));
        assert!(error.contains("pinned revision"));
    }

    #[cfg(unix)]
    #[test]
    fn go_generator_arguments_and_exit_status() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let generator = dir.path().join("generator with spaces");
        let catalog = dir.path().join("catalog with spaces.json");
        let output = dir.path().join("output with spaces");
        std::fs::write(&catalog, "catalog marker").unwrap();
        std::fs::write(
            &generator,
            include_str!("../../../tests/integration/testdata/fake-go-generator.sh"),
        )
        .unwrap();
        std::fs::set_permissions(&generator, std::fs::Permissions::from_mode(0o755)).unwrap();
        run_generator(&generator, &catalog, &output, "codecs").unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join("input.json")).unwrap(),
            "catalog marker"
        );
        assert_eq!(
            std::fs::read_to_string(output.join("package.txt"))
                .unwrap()
                .trim(),
            "codecs"
        );
        let error = run_generator(&generator, &catalog, &output, "fail")
            .unwrap_err()
            .to_string();
        assert!(error.contains("23"), "{error}");
    }
}
