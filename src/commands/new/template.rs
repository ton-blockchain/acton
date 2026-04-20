use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use serde::Serialize;
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

static NFT_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/nft");

const AGENTS_FILE_NAME: &str = "AGENTS.md";
const NPM_PACKAGE_NAME_PLACEHOLDER: &str = "__ACTON_NPM_PACKAGE_NAME__";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectLayout {
    Standard,
    App,
}

impl ProjectLayout {
    #[must_use]
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::App => "app",
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
    deploy_script: &'static str,
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

    #[must_use]
    pub(super) const fn deploy_script_path(self) -> &'static str {
        self.deploy_script
    }
}

#[derive(Clone, Copy)]
struct TemplateDefinition {
    default_scaffold: ProjectScaffold,
    app_scaffold: Option<ProjectScaffold>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct TemplateCatalog {
    schema_version: u8,
    templates: Vec<TemplateCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateCatalogEntry {
    id: &'static str,
    description: &'static str,
    supports_app: bool,
    scaffolds: Vec<TemplateScaffoldInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateScaffoldInfo {
    kind: &'static str,
    includes_typescript_app: bool,
    contracts: Vec<TemplateContractInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateContractInfo {
    id: &'static str,
    name: &'static str,
    src: &'static str,
}

const EMPTY_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "Empty",
    name: "Empty",
    src: "contracts/Empty.tolk",
}];

const COUNTER_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "Counter",
    name: "Counter",
    src: "contracts/Counter.tolk",
}];

const COUNTER_APP_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "Counter",
    name: "Counter",
    src: "contracts/src/Counter.tolk",
}];

const JETTON_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "JettonMinter",
        name: "JettonMinter",
        src: "contracts/JettonMinter.tolk",
    },
    ContractTemplate {
        id: "JettonWallet",
        name: "JettonWallet",
        src: "contracts/JettonWallet.tolk",
    },
];

const NFT_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "NftCollection",
        name: "NftCollection",
        src: "contracts/NftCollection.tolk",
    },
    ContractTemplate {
        id: "NftItem",
        name: "NftItem",
        src: "contracts/NftItem.tolk",
    },
];

const EMPTY_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &EMPTY_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &EMPTY_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
};

const COUNTER_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &COUNTER_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &COUNTER_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
};

const COUNTER_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &COUNTER_APP_TEMPLATE_DIR,
    layout: ProjectLayout::App,
    contracts: &COUNTER_APP_CONTRACTS,
    deploy_script: "contracts/scripts/deploy.tolk",
};

const JETTON_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &JETTON_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &JETTON_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
};

const NFT_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    dir: &NFT_TEMPLATE_DIR,
    layout: ProjectLayout::Standard,
    contracts: &NFT_CONTRACTS,
    deploy_script: "scripts/deployCollection.tolk",
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProjectTemplate {
    Empty,
    Counter,
    Jetton,
    Nft,
}

impl ProjectTemplate {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Counter => "counter",
            Self::Jetton => "jetton",
            Self::Nft => "nft",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Empty => "Minimal project skeleton",
            Self::Counter => "Simple counter contract",
            Self::Jetton => "Jetton minter and wallet contracts",
            Self::Nft => "NFT collection and item contracts",
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

const NFT_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: NFT_SCAFFOLD,
    app_scaffold: None,
};

const fn template_definition(template: ProjectTemplate) -> &'static TemplateDefinition {
    match template {
        ProjectTemplate::Empty => &EMPTY_TEMPLATE_DEFINITION,
        ProjectTemplate::Counter => &COUNTER_TEMPLATE_DEFINITION,
        ProjectTemplate::Jetton => &JETTON_TEMPLATE_DEFINITION,
        ProjectTemplate::Nft => &NFT_TEMPLATE_DEFINITION,
    }
}

pub(super) fn get_available_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate::Empty,
        ProjectTemplate::Counter,
        ProjectTemplate::Jetton,
        ProjectTemplate::Nft,
    ]
}

pub(super) fn template_catalog() -> TemplateCatalog {
    let templates = get_available_templates()
        .into_iter()
        .map(|template| {
            let definition = template_definition(template);
            let mut scaffolds = vec![serialize_scaffold(definition.default_scaffold)];
            if let Some(app_scaffold) = definition.app_scaffold {
                scaffolds.push(serialize_scaffold(app_scaffold));
            }

            TemplateCatalogEntry {
                id: template.as_str(),
                description: template.description(),
                supports_app: definition.app_scaffold.is_some(),
                scaffolds,
            }
        })
        .collect();

    TemplateCatalog {
        schema_version: 1,
        templates,
    }
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

fn serialize_scaffold(scaffold: ProjectScaffold) -> TemplateScaffoldInfo {
    TemplateScaffoldInfo {
        kind: scaffold.layout().as_str(),
        includes_typescript_app: scaffold.layout().includes_typescript_app(),
        contracts: scaffold
            .contracts()
            .iter()
            .map(|contract| TemplateContractInfo {
                id: contract.id,
                name: contract.name,
                src: contract.src,
            })
            .collect(),
    }
}

pub(super) fn create_project_from_scaffold(
    scaffold: ProjectScaffold,
    target_dir: &Path,
    include_agents: bool,
    npm_package_name: Option<&str>,
) -> anyhow::Result<()> {
    extract_template_dir(scaffold.dir, target_dir, include_agents, npm_package_name)?;
    Ok(())
}

fn extract_template_dir(
    dir: &Dir<'static>,
    base_path: &Path,
    include_agents: bool,
    npm_package_name: Option<&str>,
) -> std::io::Result<()> {
    for entry in dir.entries() {
        if !include_agents && should_skip_entry(entry.path()) {
            continue;
        }

        let path = base_path.join(entry.path());

        if let Some(subdir) = entry.as_dir() {
            fs::create_dir_all(&path)?;
            extract_template_dir(subdir, base_path, include_agents, npm_package_name)?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            if let Some(package_name) = npm_package_name
                && matches!(
                    entry.path().to_str(),
                    Some("package.json" | "package-lock.json")
                )
            {
                let content = String::from_utf8_lossy(file.contents())
                    .replace(NPM_PACKAGE_NAME_PLACEHOLDER, package_name);
                fs::write(path, content)?;
            } else {
                fs::write(path, file.contents())?;
            }
        }
    }

    Ok(())
}

fn should_skip_entry(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new(AGENTS_FILE_NAME))
}
