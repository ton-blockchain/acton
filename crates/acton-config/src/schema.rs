pub const ACTON_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/acton.schema.json"
));

pub const LINT_REPORT_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/lint-report.schema.json"
));

pub const MUTATION_RULES_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/mutation-rules.schema.json"
));
