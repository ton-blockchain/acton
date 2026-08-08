use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::Args;

const QUERY_DIRECTORIES: &[(&str, &str)] = &[
    (
        "crates/tree-sitter-tolk/queries",
        "apps/zed-ton/languages/tolk",
    ),
    (
        "crates/tree-sitter-tlb/queries",
        "apps/zed-ton/languages/tlb",
    ),
    (
        "crates/tree-sitter-tasm/queries",
        "apps/zed-ton/languages/tasm",
    ),
    (
        "crates/tree-sitter-fift/queries",
        "apps/zed-ton/languages/fift",
    ),
];

#[derive(Args)]
pub(crate) struct SyncZedQueriesArgs {
    /// Verify that the Zed copies match the grammar queries without writing files.
    #[arg(long)]
    pub(crate) check: bool,
}

pub(crate) fn run(args: SyncZedQueriesArgs) -> Result<()> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask manifest directory has no parent")?;

    for &(source, destination) in QUERY_DIRECTORIES {
        sync_directory(
            &workspace_root.join(source),
            &workspace_root.join(destination),
            args.check,
        )?;
    }

    if args.check {
        println!("Zed language queries are up to date");
    } else {
        println!("Synchronized Zed language queries");
    }

    Ok(())
}

fn sync_directory(source: &Path, destination: &Path, check: bool) -> Result<()> {
    let source_queries = read_queries(source)?;
    if source_queries.is_empty() {
        bail!("no query files found in `{}`", source.display());
    }

    if check {
        let destination_queries = read_queries(destination)?;
        if source_queries != destination_queries {
            bail!(
                "Zed queries in `{}` are out of date; run `cargo xtask sync-zed-queries`",
                destination.display()
            );
        }
        return Ok(());
    }

    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create `{}`", destination.display()))?;

    for entry in fs::read_dir(destination)
        .with_context(|| format!("failed to read `{}`", destination.display()))?
    {
        let path = entry?.path();
        if is_query_file(&path)
            && !source_queries.contains_key(
                path.file_name()
                    .context("query path has no file name")?
                    .to_string_lossy()
                    .as_ref(),
            )
        {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale `{}`", path.display()))?;
        }
    }

    for (name, content) in source_queries {
        let path = destination.join(name);
        if fs::read(&path).ok().as_deref() == Some(content.as_slice()) {
            continue;
        }
        fs::write(&path, content)
            .with_context(|| format!("failed to write `{}`", path.display()))?;
    }

    Ok(())
}

fn read_queries(directory: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut queries = BTreeMap::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read `{}`", directory.display()))?
    {
        let path = entry?.path();
        if !is_query_file(&path) {
            continue;
        }
        let name = path
            .file_name()
            .context("query path has no file name")?
            .to_string_lossy()
            .into_owned();
        let content =
            fs::read(&path).with_context(|| format!("failed to read `{}`", path.display()))?;
        queries.insert(name, content);
    }
    Ok(queries)
}

fn is_query_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|extension| extension == "scm")
}
