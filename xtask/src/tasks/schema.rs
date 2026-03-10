use std::fs;
use std::path::PathBuf;

use acton_config::config::ActonConfig;
use anyhow::{Context, Result, bail};
use clap::Args;
use schemars::r#gen::SchemaSettings;

const DEFAULT_OUTPUT_PATH: &str = "acton-new.schema.json";

#[derive(Args)]
pub(crate) struct SchemaArgs {
    #[arg(long, short = 'o', value_name = "PATH", default_value = DEFAULT_OUTPUT_PATH)]
    pub(crate) output: PathBuf,
    #[arg(long)]
    pub(crate) check: bool,
}

pub(crate) fn run(args: SchemaArgs) -> Result<()> {
    let generator = SchemaSettings::draft07().with(|settings| {
        settings.option_add_null_type = false;
    });
    let schema = generator
        .into_generator()
        .into_root_schema_for::<ActonConfig>();
    let content = format!(
        "{}\n",
        serde_json::to_string_pretty(&schema).context("failed to serialize ActonConfig schema")?
    );

    if args.check {
        let existing = fs::read_to_string(&args.output)
            .with_context(|| format!("failed to read schema from {}", args.output.display()))?;

        if existing != content {
            bail!("schema is out of date: {}", args.output.display());
        }

        println!("Schema is up to date: {}", args.output.display());
        return Ok(());
    }

    fs::write(&args.output, content)
        .with_context(|| format!("failed to write schema to {}", args.output.display()))?;

    println!("Wrote JSON schema to {}", args.output.display());
    Ok(())
}
