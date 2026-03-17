use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use std::path::Path;

static EMPTY_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/empty");

static COUNTER_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter");

static JETTON_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/jetton");

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProjectTemplate {
    Empty,
    Counter,
    Jetton,
}

impl ProjectTemplate {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Counter => "counter",
            Self::Jetton => "jetton",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Empty => "Minimal project skeleton",
            Self::Counter => "Simple counter contract",
            Self::Jetton => "Jetton minter and wallet contracts",
        }
    }
}

impl std::fmt::Display for ProjectTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(super) fn get_available_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate::Empty,
        ProjectTemplate::Counter,
        ProjectTemplate::Jetton,
    ]
}

pub(super) fn create_project_from_template(
    template_name: ProjectTemplate,
    target_dir: &Path,
) -> anyhow::Result<()> {
    let template = match template_name {
        ProjectTemplate::Empty => &EMPTY_TEMPLATE_DIR,
        ProjectTemplate::Counter => &COUNTER_TEMPLATE_DIR,
        ProjectTemplate::Jetton => &JETTON_TEMPLATE_DIR,
    };

    template.extract(target_dir)?;
    Ok(())
}
