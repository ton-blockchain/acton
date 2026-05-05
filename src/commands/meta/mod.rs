use acton_config::schema::{
    ACTON_SCHEMA_JSON, LINT_REPORT_SCHEMA_JSON, MUTATION_RULES_SCHEMA_JSON,
};
use anyhow::Result;
use clap::ValueEnum;
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BuiltinSchema {
    #[value(name = "acton-toml", alias = "acton")]
    ActonToml,
    #[value(alias = "lint-json")]
    LintReport,
    #[value(alias = "custom-mutation-rules")]
    MutationRules,
}

impl BuiltinSchema {
    const fn content(self) -> &'static str {
        match self {
            Self::ActonToml => ACTON_SCHEMA_JSON,
            Self::LintReport => LINT_REPORT_SCHEMA_JSON,
            Self::MutationRules => MUTATION_RULES_SCHEMA_JSON,
        }
    }
}

pub fn print_schema_cmd(schema: BuiltinSchema) -> Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(schema.content().as_bytes())?;
    stdout.flush()?;
    Ok(())
}
