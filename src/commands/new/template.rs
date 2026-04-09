use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

static EMPTY_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/empty");

static COUNTER_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter");

static COUNTER_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter-app");

static JETTON_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/jetton");

const AGENTS_FILE_NAME: &str = "AGENTS.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectLayout {
    Standard,
    App,
}

impl ProjectLayout {
    #[must_use]
    pub(super) const fn deploy_script_path(self) -> &'static str {
        match self {
            Self::Standard => "scripts/deploy.tolk",
            Self::App => "contracts/scripts/deploy.tolk",
        }
    }

    #[must_use]
    pub(super) const fn contracts_mapping(self) -> &'static str {
        match self {
            Self::Standard => "contracts",
            Self::App => "contracts/src",
        }
    }

    #[must_use]
    pub(super) const fn tests_mapping(self) -> &'static str {
        match self {
            Self::Standard => "tests",
            Self::App => "contracts/tests",
        }
    }

    #[must_use]
    pub(super) const fn wrappers_mapping(self) -> &'static str {
        match self {
            Self::Standard => "wrappers",
            Self::App => "contracts/wrappers",
        }
    }

    #[must_use]
    pub(super) const fn includes_typescript_app(self) -> bool {
        matches!(self, Self::App)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ContractTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub src: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProjectScaffold {
    dir: &'static Dir<'static>,
    layout: ProjectLayout,
    contracts: &'static [ContractTemplate],
}

impl ProjectScaffold {
    #[must_use]
    pub(super) const fn layout(self) -> ProjectLayout {
        self.layout
    }

    #[must_use]
    pub(super) const fn contracts(self) -> &'static [ContractTemplate] {
        self.contracts
    }
}

#[derive(Clone, Copy)]
struct TemplateDefinition {
    default_scaffold: ProjectScaffold,
    app_scaffold: Option<ProjectScaffold>,
}

const EMPTY_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "empty",
    name: "Empty",
    src: "contracts/contract.tolk",
}];

const COUNTER_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "counter",
    name: "Counter",
    src: "contracts/counter.tolk",
}];

const COUNTER_APP_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "counter",
    name: "Counter",
    src: "contracts/src/counter.tolk",
}];

const JETTON_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "jetton_minter",
        name: "JettonMinter",
        src: "contracts/jetton-minter-contract.tolk",
    },
    ContractTemplate {
        id: "jetton_wallet",
        name: "JettonWallet",
        src: "contracts/jetton-wallet-contract.tolk",
    },
];

const EMPTY_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &EMPTY_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &EMPTY_CONTRACTS,
};

const COUNTER_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &COUNTER_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &COUNTER_CONTRACTS,
};

const COUNTER_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &COUNTER_APP_TEMPLATE_DIR,
    layout: ProjectLayout::App,
    contracts: &COUNTER_APP_CONTRACTS,
};

const JETTON_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &JETTON_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &JETTON_CONTRACTS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProjectTemplate {
    Empty,
    Counter,
    Jetton,
}

impl ProjectTemplate {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Counter => "counter",
            Self::Jetton => "jetton",
        }
    }

    #[must_use]
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

const EMPTY_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: EMPTY_SCAFFOLD,
    app_scaffold: None,
};

const COUNTER_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: COUNTER_SCAFFOLD,
    app_scaffold: Some(COUNTER_APP_SCAFFOLD),
};

const JETTON_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: JETTON_SCAFFOLD,
    app_scaffold: None,
};

const fn template_definition(template: ProjectTemplate) -> &'static TemplateDefinition {
    match template {
        ProjectTemplate::Empty => &EMPTY_TEMPLATE_DEFINITION,
        ProjectTemplate::Counter => &COUNTER_TEMPLATE_DEFINITION,
        ProjectTemplate::Jetton => &JETTON_TEMPLATE_DEFINITION,
    }
}

pub(super) fn get_available_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate::Empty,
        ProjectTemplate::Counter,
        ProjectTemplate::Jetton,
    ]
}

pub(super) const fn template_supports_app(template: ProjectTemplate) -> bool {
    template_definition(template).app_scaffold.is_some()
}

pub(super) const fn project_scaffold(
    template: ProjectTemplate,
    include_app: bool,
) -> Option<ProjectScaffold> {
    let definition = template_definition(template);
    if include_app {
        definition.app_scaffold
    } else {
        Some(definition.default_scaffold)
    }
}

pub(super) fn create_project_from_scaffold(
    scaffold: ProjectScaffold,
    target_dir: &Path,
    include_agents: bool,
) -> anyhow::Result<()> {
    extract_template_dir(scaffold.dir, target_dir, include_agents)?;
    Ok(())
}

fn extract_template_dir(
    dir: &Dir<'static>,
    base_path: &Path,
    include_agents: bool,
) -> std::io::Result<()> {
    for entry in dir.entries() {
        if !include_agents && should_skip_entry(entry.path()) {
            continue;
        }

        let path = base_path.join(entry.path());

        if let Some(subdir) = entry.as_dir() {
            fs::create_dir_all(&path)?;
            extract_template_dir(subdir, base_path, include_agents)?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(path, file.contents())?;
        }
    }

    Ok(())
}

fn should_skip_entry(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new(AGENTS_FILE_NAME))
}
