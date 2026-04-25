pub const ACTON_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../acton.schema.json"
));

pub const MUTATION_RULES_SCHEMA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../mutation-rules.schema.json"
));
