use std::fs;
use std::path::PathBuf;

use acton_config::config::ActonConfig;
use acton_config::lint_output::LintJsonReport;
use acton_config::mutation_rules::CustomMutationRulesFile;
use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use schemars::r#gen::SchemaSettings;
use schemars::schema::{InstanceType, RootSchema, Schema, SchemaObject};
use schemars::{JsonSchema, Map};
use tolk_linter::Linter;

const ACTON_TOML_OUTPUT_PATH: &str = "crates/acton-config/schemas/acton.schema.json";
const LINT_REPORT_OUTPUT_PATH: &str = "crates/acton-config/schemas/lint-report.schema.json";
const MUTATION_RULES_OUTPUT_PATH: &str = "crates/acton-config/schemas/mutation-rules.schema.json";
const LINT_RULE_LEVEL_SCHEMA_NAME: &str = "LintRuleLevel";

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
        SchemaTarget::ActonToml => acton_toml_schema_content()?,
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

fn acton_toml_schema_content() -> Result<String> {
    let mut schema = root_schema::<ActonConfig>();
    add_lint_rule_documentation(&mut schema)?;
    serialize_schema(&schema)
}

fn schema_content<T: JsonSchema>() -> Result<String> {
    serialize_schema(&root_schema::<T>())
}

fn root_schema<T: JsonSchema>() -> RootSchema {
    let generator = SchemaSettings::draft07().with(|settings| {
        settings.option_add_null_type = false;
    });
    generator.into_generator().into_root_schema_for::<T>()
}

fn serialize_schema(schema: &RootSchema) -> Result<String> {
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(schema).context("failed to serialize JSON schema")?
    ))
}

fn add_lint_rule_documentation(schema: &mut RootSchema) -> Result<()> {
    let lint_level = schema
        .definitions
        .get("LintLevel")
        .cloned()
        .context("Acton.toml schema is missing the LintLevel definition")?;
    schema.definitions.insert(
        LINT_RULE_LEVEL_SCHEMA_NAME.to_owned(),
        without_documentation(lint_level),
    );
    let lint_rules = schema
        .definitions
        .get_mut("LintRules")
        .and_then(|schema| match schema {
            Schema::Object(schema) => Some(schema),
            Schema::Bool(_) => None,
        })
        .context("Acton.toml schema is missing the LintRules definition")?;
    lint_rules
        .object()
        .properties
        .extend(lint_rule_properties());

    Ok(())
}

fn lint_rule_properties() -> Map<String, Schema> {
    Linter::Tolk
        .all_rules()
        // Compiler errors are emitted before configurable lint rules are evaluated.
        .filter(|rule| rule.name() != "compiler-error")
        .map(|rule| {
            let title = Linter::Tolk.code_for_rule(rule).map_or_else(
                || rule.name().to_owned(),
                |code| format!("{code}: {}", rule.name()),
            );
            let mut schema = SchemaObject::default();
            schema.metadata().title = Some(title);
            schema.metadata().description = rule.explanation().map(|it| it.trim().to_owned());
            let mut contract_overrides = SchemaObject {
                instance_type: Some(InstanceType::Object.into()),
                ..SchemaObject::default()
            };
            contract_overrides.object().additional_properties = Some(Box::new(Schema::new_ref(
                format!("#/definitions/{LINT_RULE_LEVEL_SCHEMA_NAME}"),
            )));
            schema.subschemas().any_of = Some(vec![
                Schema::new_ref(format!("#/definitions/{LINT_RULE_LEVEL_SCHEMA_NAME}")),
                contract_overrides.into(),
            ]);
            (rule.name().to_owned(), schema.into())
        })
        .collect()
}

fn without_documentation(mut schema: Schema) -> Schema {
    remove_documentation(&mut schema);
    schema
}

fn remove_documentation(schema: &mut Schema) {
    let Schema::Object(schema) = schema else {
        return;
    };
    if let Some(metadata) = &mut schema.metadata {
        metadata.title = None;
        metadata.description = None;
    }
    if let Some(subschemas) = &mut schema.subschemas {
        for branch in subschemas
            .all_of
            .iter_mut()
            .chain(&mut subschemas.any_of)
            .chain(&mut subschemas.one_of)
            .flatten()
        {
            remove_documentation(branch);
        }
    }
}
