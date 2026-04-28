use std::fs;
use std::path::PathBuf;

use acton_config::config::ActonConfig;
use acton_config::lint_output::LintJsonReport;
use acton_config::mutation_rules::CustomMutationRulesFile;
use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;

const ACTON_TOML_OUTPUT_PATH: &str = "crates/acton-config/schemas/acton.schema.json";
const LINT_REPORT_OUTPUT_PATH: &str = "crates/acton-config/schemas/lint-report.schema.json";
const MUTATION_RULES_OUTPUT_PATH: &str = "crates/acton-config/schemas/mutation-rules.schema.json";

#[derive(Args)]
pub(crate) struct SchemaArgs {
    #[arg(long, value_enum, default_value_t = SchemaTarget::ActonToml)]
    pub(crate) schema: SchemaTarget,
    #[arg(long, short = 'o', value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
    #[arg(long)]
    pub(crate) check: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum SchemaTarget {
    ActonToml,
    LintReport,
    MutationRules,
}

impl SchemaTarget {
    const fn default_output_path(self) -> &'static str {
        match self {
            Self::ActonToml => ACTON_TOML_OUTPUT_PATH,
            Self::LintReport => LINT_REPORT_OUTPUT_PATH,
            Self::MutationRules => MUTATION_RULES_OUTPUT_PATH,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::ActonToml => "Acton.toml",
            Self::LintReport => "lint JSON report",
            Self::MutationRules => "custom mutation rules",
        }
    }
}

pub(crate) fn run(args: SchemaArgs) -> Result<()> {
    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from(args.schema.default_output_path()));
    let content = match args.schema {
        SchemaTarget::ActonToml => schema_content::<ActonConfig>()?,
        SchemaTarget::LintReport => schema_content::<LintJsonReport>()?,
        SchemaTarget::MutationRules => schema_content::<CustomMutationRulesFile>()?,
    };

    if args.check {
        let existing = fs::read_to_string(&output)
            .with_context(|| format!("failed to read schema from {}", output.display()))?;

        if existing != content {
            bail!(
                "{} schema is out of date: {}",
                args.schema.label(),
                output.display()
            );
        }

        println!("Schema is up to date: {}", output.display());
        return Ok(());
    }

    fs::write(&output, content)
        .with_context(|| format!("failed to write schema to {}", output.display()))?;

    println!("Wrote JSON schema to {}", output.display());
    Ok(())
}

fn schema_content<T: JsonSchema>() -> Result<String> {
    let generator = SchemaSettings::draft07().with(|settings| {
        settings.option_add_null_type = false;
    });
    let schema = generator.into_generator().into_root_schema_for::<T>();
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&schema).context("failed to serialize JSON schema")?
    ))
}
