use include_dir::{Dir, include_dir};
use std::path::Path;

static EMPTY_TEMPLATE_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/empty");

static COUNTER_TEMPLATE_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter");

static JETTON_TEMPLATE_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/jetton");

pub fn get_available_templates() -> Vec<&'static str> {
    vec!["empty", "counter", "jetton"]
}

pub fn create_project_from_template(template_name: &str, target_dir: &Path) -> anyhow::Result<()> {
    let template = match template_name {
        "empty" => &EMPTY_TEMPLATE_DIR,
        "counter" => &COUNTER_TEMPLATE_DIR,
        "jetton" => &JETTON_TEMPLATE_DIR,
        _ => anyhow::bail!("Unknown template name: {}", template_name),
    };

    template.extract(target_dir)?;
    Ok(())
}
