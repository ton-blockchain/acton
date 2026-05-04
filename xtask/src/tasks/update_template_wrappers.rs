use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use tempfile::TempDir;

struct Template {
    name: &'static str,
    contracts: &'static [&'static str],
}

const TEMPLATES: &[Template] = &[
    Template {
        name: "empty",
        contracts: &["Empty"],
    },
    Template {
        name: "counter",
        contracts: &["Counter"],
    },
    Template {
        name: "jetton",
        contracts: &["JettonMinter", "JettonWallet"],
    },
    Template {
        name: "nft",
        contracts: &["NftCollection", "NftItem"],
    },
    Template {
        name: "w5-extension",
        contracts: &["SimpleExtension", "WalletV5"],
    },
];

pub(crate) fn run() -> Result<()> {
    let workspace_root =
        env::current_dir().context("failed to determine workspace root from current directory")?;
    let templates_dir = workspace_root.join("src/commands/new/templates");
    if !templates_dir.is_dir() {
        bail!(
            "templates directory `{}` not found; run from the repository root",
            templates_dir.display()
        );
    }

    let acton = build_acton(&workspace_root)?;

    let scratch =
        TempDir::new().context("failed to create temporary directory for scaffolded projects")?;

    for template in TEMPLATES {
        update_template(&acton, &templates_dir, scratch.path(), template)?;
    }

    Ok(())
}

fn build_acton(workspace_root: &Path) -> Result<PathBuf> {
    let status = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["build", "--release", "--bin", "acton"])
        .current_dir(workspace_root)
        .status()
        .context("failed to spawn `cargo build`")?;
    if !status.success() {
        bail!("`cargo build --release --bin acton` failed");
    }

    let binary = workspace_root.join("target/release/acton");
    if !binary.is_file() {
        bail!("acton binary not found at `{}`", binary.display());
    }
    Ok(binary)
}

fn update_template(
    acton: &Path,
    templates_dir: &Path,
    scratch: &Path,
    template: &Template,
) -> Result<()> {
    let project_dir = scratch.join(template.name);

    run_acton(
        acton,
        scratch,
        &[
            "new",
            project_dir
                .to_str()
                .context("project path is not valid UTF-8")?,
            "--name",
            template.name,
            "--template",
            template.name,
            "--license",
            "MIT",
            "--app",
        ],
    )?;
    run_acton(acton, &project_dir, &["build"])?;

    let std_wrappers_dir = templates_dir.join(template.name).join("wrappers");
    let app_wrappers_ts_dir = templates_dir
        .join(format!("{}-app", template.name))
        .join("wrappers-ts");
    fs::create_dir_all(&app_wrappers_ts_dir)
        .with_context(|| format!("failed to create `{}`", app_wrappers_ts_dir.display()))?;

    for contract in template.contracts {
        run_acton(acton, &project_dir, &["wrapper", contract])?;
        let generated_tolk = project_dir
            .join("contracts")
            .join("wrappers")
            .join(format!("{contract}.gen.tolk"));
        let target_tolk = std_wrappers_dir.join(format!("{contract}.gen.tolk"));
        copy_file(&generated_tolk, &target_tolk)?;

        run_acton(acton, &project_dir, &["wrapper", contract, "--ts"])?;
        let generated_ts = project_dir
            .join("wrappers-ts")
            .join(format!("{contract}.gen.ts"));
        let target_ts = app_wrappers_ts_dir.join(format!("{contract}.gen.ts"));
        copy_file(&generated_ts, &target_ts)?;
    }

    Ok(())
}

fn run_acton(acton: &Path, working_dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(acton)
        .args(args)
        .current_dir(working_dir)
        .status()
        .with_context(|| format!("failed to spawn `acton {}`", args.join(" ")))?;
    if !status.success() {
        bail!("`acton {}` failed", args.join(" "));
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy `{}` to `{}`",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}
