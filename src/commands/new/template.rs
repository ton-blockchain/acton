use clap::ValueEnum;
use include_dir::{Dir, include_dir};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

static EMPTY_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/empty");

static EMPTY_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/empty-app");

static COUNTER_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter");

static COUNTER_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/counter-app");

static JETTON_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/jetton");

static JETTON_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/jetton-app");

static NFT_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/nft");

static NFT_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/nft-app");

static W5_EXTENSION_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/w5-extension");

static W5_EXTENSION_APP_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/new/templates/w5-extension-app");

const AGENTS_FILE_NAME: &str = "AGENTS.md";
const NPM_PACKAGE_NAME_PLACEHOLDER: &str = "__ACTON_NPM_PACKAGE_NAME__";
const AUTHOR_PLACEHOLDER: &str = "__ACTON_AUTHOR__";

#[derive(Clone, Copy)]
struct TemplateRenderContext<'a> {
    npm_package_name: Option<&'a str>,
    author: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectLayout {
    Standard,
    App,
}

impl ProjectLayout {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::App => "app",
        }
    }

    #[must_use]
    pub(crate) const fn contracts_mapping(self) -> &'static str {
        match self {
            Self::Standard => "contracts",
            Self::App => "contracts/src",
        }
    }

    #[must_use]
    pub(crate) const fn tests_mapping(self) -> &'static str {
        match self {
            Self::Standard => "tests",
            Self::App => "contracts/tests",
        }
    }

    #[must_use]
    pub(crate) const fn wrappers_mapping(self) -> &'static str {
        match self {
            Self::Standard => "wrappers",
            Self::App => "contracts/wrappers",
        }
    }

    #[must_use]
    pub(crate) const fn includes_typescript_app(self) -> bool {
        matches!(self, Self::App)
    }

    pub(crate) fn remap_path(self, path: &str) -> String {
        match self {
            Self::Standard => path.to_owned(),
            Self::App => {
                const REMAPPINGS: &[(&str, &str)] = &[
                    ("contracts/", "contracts/src/"),
                    ("scripts/", "contracts/scripts/"),
                    ("tests/", "contracts/tests/"),
                    ("wrappers/", "contracts/wrappers/"),
                ];
                for &(from, to) in REMAPPINGS {
                    if let Some(rest) = path.strip_prefix(from) {
                        return format!("{to}{rest}");
                    }
                }
                path.to_owned()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ContractTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub src: &'static str,
    pub depends: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExtraScript {
    pub alias: &'static str,
    pub script: &'static str,
    pub net: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProjectScaffold {
    base_dir: &'static Dir<'static>,
    app_overlay_dir: Option<&'static Dir<'static>>,
    layout: ProjectLayout,
    contracts: &'static [ContractTemplate],
    deploy_script: &'static str,
    extra_scripts: &'static [ExtraScript],
}

impl ProjectScaffold {
    #[must_use]
    pub(crate) const fn layout(self) -> ProjectLayout {
        self.layout
    }

    #[must_use]
    pub(crate) const fn contracts(self) -> &'static [ContractTemplate] {
        self.contracts
    }

    #[must_use]
    pub(crate) fn deploy_script_path(&self) -> String {
        self.layout.remap_path(self.deploy_script)
    }

    #[must_use]
    pub(crate) fn contract_src(&self, contract: &ContractTemplate) -> String {
        self.layout.remap_path(contract.src)
    }

    #[must_use]
    pub(crate) fn extra_scripts(&self) -> Vec<(String, String)> {
        self.extra_scripts
            .iter()
            .map(|script| {
                let path = self.layout.remap_path(script.script);
                let cmd = match script.net {
                    Some(net) => format!("acton script {path} --net {net}"),
                    None => format!("acton script {path}"),
                };
                (script.alias.to_owned(), cmd)
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct TemplateDefinition {
    default_scaffold: ProjectScaffold,
    app_scaffold: Option<ProjectScaffold>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemplateCatalog {
    schema_version: u8,
    templates: Vec<TemplateCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateCatalogEntry {
    id: &'static str,
    aliases: &'static [&'static str],
    category: &'static str,
    description: &'static str,
    supports_app: bool,
    scaffolds: Vec<TemplateScaffoldInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateScaffoldInfo {
    kind: &'static str,
    includes_typescript_app: bool,
    deploy_script: String,
    scripts: Vec<TemplateScriptInfo>,
    contracts: Vec<TemplateContractInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateScriptInfo {
    alias: String,
    command: String,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateContractInfo {
    id: &'static str,
    name: &'static str,
    src: String,
}

const EMPTY_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "Empty",
    name: "Empty",
    src: "contracts/Empty.tolk",
    depends: &[],
}];

const COUNTER_CONTRACTS: [ContractTemplate; 1] = [ContractTemplate {
    id: "Counter",
    name: "Counter",
    src: "contracts/Counter.tolk",
    depends: &[],
}];

const JETTON_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "JettonMinter",
        name: "JettonMinter",
        src: "contracts/JettonMinter.tolk",
        depends: &["JettonWallet"],
    },
    ContractTemplate {
        id: "JettonWallet",
        name: "JettonWallet",
        src: "contracts/JettonWallet.tolk",
        depends: &[],
    },
];

const NFT_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "NftCollection",
        name: "NftCollection",
        src: "contracts/NftCollection.tolk",
        depends: &[],
    },
    ContractTemplate {
        id: "NftItem",
        name: "NftItem",
        src: "contracts/NftItem.tolk",
        depends: &[],
    },
];

const W5_EXTENSION_CONTRACTS: [ContractTemplate; 2] = [
    ContractTemplate {
        id: "SimpleExtension",
        name: "SimpleExtension",
        src: "contracts/SimpleExtension.tolk",
        depends: &[],
    },
    ContractTemplate {
        id: "WalletV5",
        name: "WalletV5",
        src: "contracts/walletv5/WalletV5.tolk",
        depends: &[],
    },
];

const EMPTY_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &EMPTY_TEMPLATE_DIR,
    app_overlay_dir: None,
    layout: ProjectLayout::Standard,
    contracts: &EMPTY_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: &[],
};

const EMPTY_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &EMPTY_TEMPLATE_DIR,
    app_overlay_dir: Some(&EMPTY_APP_TEMPLATE_DIR),
    layout: ProjectLayout::App,
    contracts: &EMPTY_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: &[],
};

const COUNTER_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &COUNTER_TEMPLATE_DIR,
    app_overlay_dir: None,
    layout: ProjectLayout::Standard,
    contracts: &COUNTER_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: &[],
};

const COUNTER_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &COUNTER_TEMPLATE_DIR,
    app_overlay_dir: Some(&COUNTER_APP_TEMPLATE_DIR),
    layout: ProjectLayout::App,
    contracts: &COUNTER_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: &[],
};

const JETTON_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &JETTON_TEMPLATE_DIR,
    app_overlay_dir: None,
    layout: ProjectLayout::Standard,
    contracts: &JETTON_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: JETTON_EXTRA_SCRIPTS,
};

const JETTON_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &JETTON_TEMPLATE_DIR,
    app_overlay_dir: Some(&JETTON_APP_TEMPLATE_DIR),
    layout: ProjectLayout::App,
    contracts: &JETTON_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: JETTON_EXTRA_SCRIPTS,
};

const JETTON_EXTRA_SCRIPTS: &[ExtraScript] = &[
    ExtraScript {
        alias: "jetton-mint",
        script: "scripts/mint.tolk",
        net: None,
    },
    ExtraScript {
        alias: "jetton-transfer",
        script: "scripts/transfer.tolk",
        net: None,
    },
    ExtraScript {
        alias: "jetton-info",
        script: "scripts/info.tolk",
        net: None,
    },
    ExtraScript {
        alias: "jetton-change-admin",
        script: "scripts/change-admin.tolk",
        net: None,
    },
    ExtraScript {
        alias: "jetton-change-metadata",
        script: "scripts/change-metadata.tolk",
        net: None,
    },
    ExtraScript {
        alias: "jetton-claim-admin",
        script: "scripts/claim-admin.tolk",
        net: None,
    },
];

const NFT_EXTRA_SCRIPTS: &[ExtraScript] = &[
    ExtraScript {
        alias: "nft-deploy-item",
        script: "scripts/deploy-item.tolk",
        net: None,
    },
    ExtraScript {
        alias: "nft-deploy-batch",
        script: "scripts/deploy-batch.tolk",
        net: None,
    },
    ExtraScript {
        alias: "nft-transfer-item",
        script: "scripts/transfer-item.tolk",
        net: None,
    },
    ExtraScript {
        alias: "nft-change-admin",
        script: "scripts/change-admin.tolk",
        net: None,
    },
];

const NFT_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &NFT_TEMPLATE_DIR,
    app_overlay_dir: None,
    layout: ProjectLayout::Standard,
    contracts: &NFT_CONTRACTS,
    deploy_script: "scripts/deploy-collection.tolk",
    extra_scripts: NFT_EXTRA_SCRIPTS,
};

const NFT_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &NFT_TEMPLATE_DIR,
    app_overlay_dir: Some(&NFT_APP_TEMPLATE_DIR),
    layout: ProjectLayout::App,
    contracts: &NFT_CONTRACTS,
    deploy_script: "scripts/deploy-collection.tolk",
    extra_scripts: NFT_EXTRA_SCRIPTS,
};

const W5_EXTENSION_EXTRA_SCRIPTS: &[ExtraScript] = &[
    ExtraScript {
        alias: "install-extension",
        script: "scripts/install-extension.tolk",
        net: Some("testnet"),
    },
    ExtraScript {
        alias: "delete-extension",
        script: "scripts/delete-extension.tolk",
        net: Some("testnet"),
    },
];

const W5_EXTENSION_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &W5_EXTENSION_TEMPLATE_DIR,
    app_overlay_dir: None,
    layout: ProjectLayout::Standard,
    contracts: &W5_EXTENSION_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: W5_EXTENSION_EXTRA_SCRIPTS,
};

const W5_EXTENSION_APP_SCAFFOLD: ProjectScaffold = ProjectScaffold {
    base_dir: &W5_EXTENSION_TEMPLATE_DIR,
    app_overlay_dir: Some(&W5_EXTENSION_APP_TEMPLATE_DIR),
    layout: ProjectLayout::App,
    contracts: &W5_EXTENSION_CONTRACTS,
    deploy_script: "scripts/deploy.tolk",
    extra_scripts: W5_EXTENSION_EXTRA_SCRIPTS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProjectTemplate {
    Empty,
    Counter,
    Jetton,
    Nft,
    #[value(alias = "w5-plugin")]
    W5Extension,
}

impl ProjectTemplate {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Counter => "counter",
            Self::Jetton => "jetton",
            Self::Nft => "nft",
            Self::W5Extension => "w5-extension",
        }
    }

    #[must_use]
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::W5Extension => &["w5-plugin"],
            _ => &[],
        }
    }

    #[must_use]
    pub const fn category(self) -> &'static str {
        match self {
            Self::Empty | Self::Counter => "starter",
            Self::Jetton => "token",
            Self::Nft => "nft",
            Self::W5Extension => "wallet",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Empty => "Minimal project skeleton",
            Self::Counter => "Simple counter contract",
            Self::Jetton => "Jetton minter and wallet contracts",
            Self::Nft => "NFT collection and item contracts",
            Self::W5Extension => "Wallet V5 extension contract and subscription example",
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
    app_scaffold: Some(EMPTY_APP_SCAFFOLD),
};

const COUNTER_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: COUNTER_SCAFFOLD,
    app_scaffold: Some(COUNTER_APP_SCAFFOLD),
};

const JETTON_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: JETTON_SCAFFOLD,
    app_scaffold: Some(JETTON_APP_SCAFFOLD),
};

const NFT_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: NFT_SCAFFOLD,
    app_scaffold: Some(NFT_APP_SCAFFOLD),
};

const W5_EXTENSION_TEMPLATE_DEFINITION: TemplateDefinition = TemplateDefinition {
    default_scaffold: W5_EXTENSION_SCAFFOLD,
    app_scaffold: Some(W5_EXTENSION_APP_SCAFFOLD),
};

const fn template_definition(template: ProjectTemplate) -> &'static TemplateDefinition {
    match template {
        ProjectTemplate::Empty => &EMPTY_TEMPLATE_DEFINITION,
        ProjectTemplate::Counter => &COUNTER_TEMPLATE_DEFINITION,
        ProjectTemplate::Jetton => &JETTON_TEMPLATE_DEFINITION,
        ProjectTemplate::Nft => &NFT_TEMPLATE_DEFINITION,
        ProjectTemplate::W5Extension => &W5_EXTENSION_TEMPLATE_DEFINITION,
    }
}

pub(crate) fn get_available_templates() -> Vec<ProjectTemplate> {
    vec![
        ProjectTemplate::Empty,
        ProjectTemplate::Counter,
        ProjectTemplate::Jetton,
        ProjectTemplate::Nft,
        ProjectTemplate::W5Extension,
    ]
}

pub(crate) fn template_catalog() -> TemplateCatalog {
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
                aliases: template.aliases(),
                category: template.category(),
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

pub(crate) const fn template_supports_app(template: ProjectTemplate) -> bool {
    template_definition(template).app_scaffold.is_some()
}

pub(crate) const fn project_scaffold(
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
    let mut scripts = vec![
        TemplateScriptInfo {
            alias: "deploy-emulation".to_owned(),
            command: format!("acton script {}", scaffold.deploy_script_path()),
        },
        TemplateScriptInfo {
            alias: "deploy-testnet".to_owned(),
            command: format!(
                "acton script {} --net testnet",
                scaffold.deploy_script_path()
            ),
        },
    ];
    scripts.extend(
        scaffold
            .extra_scripts()
            .into_iter()
            .map(|(alias, command)| TemplateScriptInfo { alias, command }),
    );

    TemplateScaffoldInfo {
        kind: scaffold.layout().as_str(),
        includes_typescript_app: scaffold.layout().includes_typescript_app(),
        deploy_script: scaffold.deploy_script_path(),
        scripts,
        contracts: scaffold
            .contracts()
            .iter()
            .map(|contract| TemplateContractInfo {
                id: contract.id,
                name: contract.name,
                src: scaffold.contract_src(contract),
            })
            .collect(),
    }
}

/// Extracts the standalone TypeScript dApp scaffold (the empty-app overlay)
/// without contract-specific wrappers, for `acton init --create-dapp`.
pub fn extract_standalone_app_scaffold(
    target_dir: &Path,
    npm_package_name: &str,
) -> std::io::Result<()> {
    extract_template_dir(
        &EMPTY_APP_TEMPLATE_DIR,
        target_dir,
        false,
        true,
        TemplateRenderContext {
            npm_package_name: Some(npm_package_name),
            author: None,
        },
    )
}

pub(crate) fn create_project_from_scaffold(
    scaffold: ProjectScaffold,
    target_dir: &Path,
    include_agents: bool,
    npm_package_name: Option<&str>,
    author: &str,
) -> anyhow::Result<()> {
    let render_context = TemplateRenderContext {
        npm_package_name,
        author: Some(author),
    };

    if let Some(overlay_dir) = scaffold.app_overlay_dir {
        extract_base_for_app_layout(scaffold.base_dir, target_dir, render_context)?;
        extract_template_dir(
            overlay_dir,
            target_dir,
            include_agents,
            false,
            render_context,
        )?;
    } else {
        extract_template_dir(
            scaffold.base_dir,
            target_dir,
            include_agents,
            false,
            render_context,
        )?;
    }
    Ok(())
}

pub(crate) fn scaffold_file_paths(scaffold: ProjectScaffold, include_agents: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(overlay_dir) = scaffold.app_overlay_dir {
        paths.extend(
            template_files(scaffold.base_dir)
                .into_iter()
                .filter_map(|path| remap_for_app_layout(&path)),
        );
        paths.extend(
            template_files(overlay_dir)
                .into_iter()
                .filter(|path| !should_skip_entry(path, include_agents, false)),
        );
    } else {
        paths.extend(
            template_files(scaffold.base_dir)
                .into_iter()
                .filter(|path| !should_skip_entry(path, include_agents, false)),
        );
    }

    paths.sort();
    paths.dedup();
    paths
}

pub(crate) fn contract_scaffold_file_paths(
    scaffold: ProjectScaffold,
    target_layout: ProjectLayout,
    namespace: &str,
) -> Vec<PathBuf> {
    let mut paths = template_files(scaffold.base_dir)
        .into_iter()
        .filter_map(|path| contract_scaffold_target_path(&path, target_layout, namespace))
        .collect::<Vec<_>>();

    paths.sort();
    paths.dedup();
    paths
}

pub(crate) fn create_contract_files_from_scaffold(
    scaffold: ProjectScaffold,
    target_dir: &Path,
    target_layout: ProjectLayout,
    namespace: &str,
    author: &str,
) -> std::io::Result<Vec<PathBuf>> {
    let render_context = TemplateRenderContext {
        npm_package_name: None,
        author: Some(author),
    };
    let mut written = Vec::new();
    extract_contract_files_from_dir(
        scaffold.base_dir,
        target_dir,
        target_layout,
        namespace,
        render_context,
        &mut written,
    )?;
    written.sort();
    Ok(written)
}

pub(crate) fn namespaced_scaffold_path(path: &str, namespace: &str) -> String {
    for prefix in ["contracts/", "scripts/", "tests/", "wrappers/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            return format!("{prefix}{namespace}/{rest}");
        }
    }
    path.to_owned()
}

fn template_files(dir: &Dir<'static>) -> Vec<PathBuf> {
    let mut paths = dir
        .files()
        .map(|file| file.path().to_path_buf())
        .collect::<Vec<_>>();

    for subdir in dir.dirs() {
        paths.extend(template_files(subdir));
    }

    paths
}

fn extract_contract_files_from_dir(
    dir: &Dir<'static>,
    base_path: &Path,
    target_layout: ProjectLayout,
    namespace: &str,
    render_context: TemplateRenderContext<'_>,
    written: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in dir.entries() {
        if let Some(subdir) = entry.as_dir() {
            extract_contract_files_from_dir(
                subdir,
                base_path,
                target_layout,
                namespace,
                render_context,
                written,
            )?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            let Some(relative_path) =
                contract_scaffold_target_path(entry.path(), target_layout, namespace)
            else {
                continue;
            };

            let target = base_path.join(&relative_path);
            write_template_file_with_import_namespace(
                &target,
                entry.path(),
                file.contents(),
                render_context,
                namespace,
            )?;
            written.push(relative_path);
        }
    }
    Ok(())
}

fn contract_scaffold_target_path(
    path: &Path,
    target_layout: ProjectLayout,
    namespace: &str,
) -> Option<PathBuf> {
    let path_str = path.to_str()?;
    if !is_contract_scaffold_path(path_str) {
        return None;
    }

    let namespaced = namespaced_scaffold_path(path_str, namespace);
    Some(PathBuf::from(target_layout.remap_path(&namespaced)))
}

fn is_contract_scaffold_path(path: &str) -> bool {
    ["contracts/", "scripts/", "tests/", "wrappers/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

fn extract_base_for_app_layout(
    dir: &Dir<'static>,
    base_path: &Path,
    render_context: TemplateRenderContext<'_>,
) -> std::io::Result<()> {
    for entry in dir.entries() {
        if let Some(subdir) = entry.as_dir() {
            extract_base_for_app_layout(subdir, base_path, render_context)?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            let Some(remapped) = remap_for_app_layout(entry.path()) else {
                continue;
            };
            let target = base_path.join(remapped);
            write_template_file(&target, entry.path(), file.contents(), render_context)?;
        }
    }
    Ok(())
}

const APP_LAYOUT_REMAPPINGS: &[(&str, &str)] = &[
    ("contracts/", "contracts/src/"),
    ("scripts/", "contracts/scripts/"),
    ("tests/", "contracts/tests/"),
    ("wrappers/", "contracts/wrappers/"),
];

fn remap_for_app_layout(path: &Path) -> Option<PathBuf> {
    let path_str = path.to_str()?;
    for &(from, to) in APP_LAYOUT_REMAPPINGS {
        if let Some(rest) = path_str.strip_prefix(from) {
            return Some(PathBuf::from(format!("{to}{rest}")));
        }
    }
    None
}

fn extract_template_dir(
    dir: &Dir<'static>,
    base_path: &Path,
    include_agents: bool,
    skip_wrappers_ts: bool,
    render_context: TemplateRenderContext<'_>,
) -> std::io::Result<()> {
    for entry in dir.entries() {
        if should_skip_entry(entry.path(), include_agents, skip_wrappers_ts) {
            continue;
        }

        let path = base_path.join(entry.path());

        if let Some(subdir) = entry.as_dir() {
            fs::create_dir_all(&path)?;
            extract_template_dir(
                subdir,
                base_path,
                include_agents,
                skip_wrappers_ts,
                render_context,
            )?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            write_template_file(&path, entry.path(), file.contents(), render_context)?;
        }
    }

    Ok(())
}

fn write_template_file(
    target: &Path,
    template_path: &Path,
    contents: &[u8],
    render_context: TemplateRenderContext<'_>,
) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Some(content) = render_template_file(template_path, contents, render_context) {
        fs::write(target, content)
    } else {
        fs::write(target, contents)
    }
}

fn write_template_file_with_import_namespace(
    target: &Path,
    template_path: &Path,
    contents: &[u8],
    render_context: TemplateRenderContext<'_>,
    namespace: &str,
) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if template_path
        .extension()
        .is_some_and(|ext| ext == OsStr::new("tolk"))
    {
        let mut rendered = render_template_file(template_path, contents, render_context)
            .unwrap_or_else(|| String::from_utf8_lossy(contents).into_owned());
        rendered = namespace_template_imports(&rendered, namespace);
        fs::write(target, rendered)
    } else if let Some(rendered) = render_template_file(template_path, contents, render_context) {
        fs::write(target, rendered)
    } else {
        fs::write(target, contents)
    }
}

fn render_template_file(
    path: &Path,
    contents: &[u8],
    render_context: TemplateRenderContext<'_>,
) -> Option<String> {
    let mut rendered = if let Some(package_name) = render_context.npm_package_name
        && matches!(path.to_str(), Some("package.json" | "package-lock.json"))
    {
        Some(String::from_utf8_lossy(contents).replace(NPM_PACKAGE_NAME_PLACEHOLDER, package_name))
    } else {
        None
    };

    if let Some(author) = render_context.author
        && path
            .extension()
            .is_some_and(|ext| ext == OsStr::new("tolk"))
    {
        let content =
            rendered.get_or_insert_with(|| String::from_utf8_lossy(contents).into_owned());
        *content = content.replace(AUTHOR_PLACEHOLDER, &escape_tolk_string_content(author));
    }

    rendered
}

fn namespace_template_imports(content: &str, namespace: &str) -> String {
    content
        .replace("@contracts/", &format!("@contracts/{namespace}/"))
        .replace("@wrappers/", &format!("@wrappers/{namespace}/"))
}

fn escape_tolk_string_content(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn should_skip_entry(path: &Path, include_agents: bool, skip_wrappers_ts: bool) -> bool {
    (!include_agents
        && path
            .file_name()
            .is_some_and(|name| name == OsStr::new(AGENTS_FILE_NAME)))
        || (skip_wrappers_ts && path.starts_with("wrappers-ts"))
}
