use acton_config::config::{ActonConfig, ToolchainConfig, manifest_path};
use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use reqwest::blocking::Client;
use reqwest::header::{
    AUTHORIZATION, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, USER_AGENT,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

const BUNDLED_TOOLCHAIN_INDEX_JSON: &str = include_str!("../toolchain-index.json");
const TOOLCHAIN_INDEX_REPOSITORIES: [&str; 2] = ["i582/acton-public", "ton-blockchain/acton"];
const TOOLCHAIN_INDEX_FILE: &str = "toolchain-index.json";
const TOOLCHAIN_INDEX_CACHE_TTL_HOURS: i64 = 24;
#[cfg(debug_assertions)]
const TEST_TOOLCHAIN_GITHUB_API_BASE_ENV: &str = "ACTON_TEST_TOOLCHAIN_GITHUB_API_BASE";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CliToolchainSelector {
    pub acton: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedCliToolchain {
    pub selector: Option<CliToolchainSelector>,
    pub args: Vec<OsString>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct ToolchainResolveReport {
    pub source: &'static str,
    pub acton: String,
    pub tolk: String,
    pub current: bool,
    pub installed: bool,
    pub install_required: bool,
    pub path: Option<String>,
    pub yanked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yank_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolchainEnvironment {
    pub current_acton: String,
    pub current_tolk: String,
    pub current_exe: PathBuf,
    pub index: Option<ToolchainIndex>,
    pub index_warning: Option<String>,
    pub installed: BTreeMap<String, InstalledToolchain>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InstalledToolchain {
    pub binary_path: PathBuf,
    pub release: Option<ToolchainReleaseMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct ToolchainReleaseMetadata {
    pub schema: u32,
    pub acton: String,
    pub tolk: String,
    pub target_triple: String,
    #[serde(default)]
    pub yanked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yank_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct ToolchainIndex {
    schema: u32,
    generated_at: String,
    releases: Vec<ToolchainIndexRelease>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct ToolchainIndexRelease {
    pub acton: String,
    pub tolk: String,
    pub tag: String,
    #[serde(default)]
    pub stable: bool,
    #[serde(default)]
    pub yanked: bool,
    pub yank_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct ToolchainIndexCacheMeta {
    pub schema: u32,
    pub fetched_at: String,
    pub source_repo: String,
    pub source_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

struct ToolchainIndexLoad {
    index: ToolchainIndex,
    warning: Option<String>,
}

struct CachedToolchainIndex {
    index: ToolchainIndex,
    meta: Option<ToolchainIndexCacheMeta>,
}

enum RemoteToolchainIndex {
    Fresh {
        index: ToolchainIndex,
        meta: ToolchainIndexCacheMeta,
    },
    NotModified {
        meta: ToolchainIndexCacheMeta,
    },
}

enum RepoFetchResult<T> {
    Found(T),
    NotModified,
    Missing,
    Failed(String),
}

#[derive(Deserialize)]
struct GitHubContentResponse {
    content: String,
    encoding: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ToolchainRequest {
    None,
    CliActon { acton: String },
    ProjectActon { acton: String, tolk: Option<String> },
    ProjectTolk { tolk: String },
}

pub fn strip_leading_toolchain_selector(args: Vec<OsString>) -> Result<ParsedCliToolchain> {
    let Some(candidate) = args.get(1) else {
        return Ok(ParsedCliToolchain {
            selector: None,
            args,
        });
    };

    let Some(raw_selector) = candidate.to_str().filter(|arg| arg.starts_with('+')) else {
        return Ok(ParsedCliToolchain {
            selector: None,
            args,
        });
    };

    let acton = parse_cli_selector(raw_selector)?;
    let stripped_args = std::iter::once(args[0].clone())
        .chain(args.into_iter().skip(2))
        .collect();

    Ok(ParsedCliToolchain {
        selector: Some(CliToolchainSelector { acton }),
        args: stripped_args,
    })
}

pub fn ensure_selector_allowed_for_args(
    selector: Option<&CliToolchainSelector>,
    args: &[OsString],
) -> Result<()> {
    if selector.is_none() {
        return Ok(());
    }

    let command = first_command_arg(args);
    let Some(command) = command.as_deref() else {
        bail!("Toolchain selector must be followed by a project command");
    };

    match command {
        "up" => {
            bail!(
                "`acton +<version> up` is invalid. Use `acton up` for the global binary, or `acton toolchain install <version>` for a project toolchain."
            );
        }
        "help" | "toolchain" | "completions" | "version" => {
            bail!(
                "`acton +<version> {command}` is invalid because `{command}` does not run inside a project toolchain."
            );
        }
        _ => Ok(()),
    }
}

pub fn resolve_toolchain(
    config: Option<&ToolchainConfig>,
    selector: Option<&CliToolchainSelector>,
    environment: &ToolchainEnvironment,
) -> Result<ToolchainResolveReport> {
    let request = match selector {
        Some(selector) => ToolchainRequest::CliActon {
            acton: selector.acton.clone(),
        },
        None => project_request(config)?,
    };

    match request {
        ToolchainRequest::None => Ok(report_for_current("current", environment, false, None)),
        ToolchainRequest::CliActon { acton } => {
            resolve_acton_request("cli-plus", &acton, None, environment)
        }
        ToolchainRequest::ProjectActon { acton, tolk } => {
            let source = if tolk.is_some() {
                "project-acton-tolk"
            } else {
                "project-acton"
            };
            resolve_acton_request(source, &acton, tolk.as_deref(), environment)
        }
        ToolchainRequest::ProjectTolk { tolk } => resolve_tolk_request(&tolk, environment),
    }
}

pub fn load_project_toolchain_config() -> Result<Option<ToolchainConfig>> {
    let manifest_path = manifest_path();
    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let config = toml::from_str::<ActonConfig>(&content)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    Ok(config.toolchain)
}

impl ToolchainEnvironment {
    pub fn runtime() -> Result<Self> {
        let index = ToolchainIndex::load_best_effort()?;
        Ok(Self {
            current_acton: crate::build_info::PACKAGE_VERSION.to_owned(),
            current_tolk: crate::build_info::TOLK_VERSION.to_owned(),
            current_exe: env::current_exe().context("failed to resolve current executable")?,
            index: Some(index.index),
            index_warning: index.warning,
            installed: scan_installed_toolchains(),
        })
    }
}

impl ToolchainIndex {
    fn load_best_effort() -> Result<ToolchainIndexLoad> {
        let cached = match read_cached_toolchain_index() {
            Ok(cached) => cached,
            Err(err) => {
                eprintln!("Warning: failed to read cached toolchain index: {err}");
                None
            }
        };

        if let Some(cached) = cached.as_ref()
            && cached
                .meta
                .as_ref()
                .is_some_and(toolchain_index_cache_is_fresh)
        {
            return Ok(ToolchainIndexLoad {
                index: cached.index.clone(),
                warning: None,
            });
        }

        match fetch_remote_toolchain_index(cached.as_ref().and_then(|cached| cached.meta.as_ref()))
        {
            Ok(RemoteToolchainIndex::Fresh { index, meta }) => {
                let warning = write_toolchain_index_cache(&index, &meta).err().map(|err| {
                    format!("fetched toolchain index but failed to update cache: {err}")
                });
                Ok(ToolchainIndexLoad { index, warning })
            }
            Ok(RemoteToolchainIndex::NotModified { meta }) => {
                if let Some(cached) = cached {
                    let warning = write_toolchain_index_cache(&cached.index, &meta)
                        .err()
                        .map(|err| {
                            format!("refreshed toolchain index metadata but failed to update cache: {err}")
                        });
                    Ok(ToolchainIndexLoad {
                        index: cached.index,
                        warning,
                    })
                } else {
                    bundled_toolchain_index_load(Some(
                        "remote toolchain index returned 304 but no local cache exists".to_owned(),
                    ))
                }
            }
            Err(err) => {
                if let Some(cached) = cached {
                    return Ok(ToolchainIndexLoad {
                        index: cached.index,
                        warning: Some(format!(
                            "failed to refresh toolchain index, using cached index: {err}"
                        )),
                    });
                }

                bundled_toolchain_index_load(Some(format!(
                    "failed to fetch toolchain index, using bundled index: {err}"
                )))
            }
        }
    }

    fn from_json(json: &str) -> Result<Self> {
        let index: Self =
            serde_json::from_str(json).context("failed to parse toolchain index JSON")?;
        index.validate()?;
        Ok(index)
    }

    pub fn from_json_for_tests(json: &str) -> Result<Self> {
        Self::from_json(json)
    }

    fn to_pretty_json(&self) -> Result<String> {
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        Ok(json)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            bail!("unsupported toolchain index schema {}", self.schema);
        }

        parse_toolchain_index_generated_at(&self.generated_at)?;

        for release in &self.releases {
            parse_exact_semver("toolchain index acton version", &release.acton)?;
            parse_exact_semver("toolchain index tolk version", &release.tolk)?;
            if release.tag != format!("v{}", release.acton) {
                bail!(
                    "toolchain index tag for Acton {} must be v{}",
                    release.acton,
                    release.acton
                );
            }
        }

        Ok(())
    }

    fn read_from_path(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path)
            .with_context(|| format!("failed to read toolchain index {}", path.display()))?;
        Self::from_json(&json)
    }

    fn release_for_acton(&self, acton: &str) -> Option<&ToolchainIndexRelease> {
        self.releases.iter().find(|release| release.acton == acton)
    }

    pub fn releases(&self) -> &[ToolchainIndexRelease] {
        &self.releases
    }

    fn newest_supported_for_tolk(&self, tolk: &str) -> Result<Option<&ToolchainIndexRelease>> {
        let mut matches = self
            .releases
            .iter()
            .filter(|release| release.tolk == tolk && release.stable && !release.yanked)
            .map(|release| {
                parse_exact_semver("toolchain index acton version", &release.acton)
                    .map(|version| (version, release))
            })
            .collect::<Result<Vec<_>>>()?;

        matches.sort_by(|(left, _), (right, _)| left.cmp(right));
        Ok(matches.pop().map(|(_, release)| release))
    }

    fn known_acton_versions(&self) -> Vec<&str> {
        self.releases
            .iter()
            .map(|release| release.acton.as_str())
            .collect()
    }

    fn known_tolk_versions(&self) -> Vec<&str> {
        let mut versions = self
            .releases
            .iter()
            .map(|release| release.tolk.as_str())
            .collect::<Vec<_>>();
        versions.sort_unstable();
        versions.dedup();
        versions
    }

    fn supported_acton_versions_for_tolk(&self, tolk: &str) -> Vec<&str> {
        let mut releases = self
            .releases
            .iter()
            .filter(|release| release.tolk == tolk && release.stable && !release.yanked)
            .filter_map(|release| {
                parse_exact_semver("toolchain index acton version", &release.acton)
                    .ok()
                    .map(|version| (version, release.acton.as_str()))
            })
            .collect::<Vec<_>>();
        releases.sort_by(|(left, _), (right, _)| left.cmp(right));
        releases.into_iter().map(|(_, acton)| acton).collect()
    }
}

fn bundled_toolchain_index_load(warning: Option<String>) -> Result<ToolchainIndexLoad> {
    Ok(ToolchainIndexLoad {
        index: ToolchainIndex::from_json(BUNDLED_TOOLCHAIN_INDEX_JSON)?,
        warning,
    })
}

fn read_cached_toolchain_index() -> Result<Option<CachedToolchainIndex>> {
    let Some(index_path) = toolchain_index_cache_path() else {
        return Ok(None);
    };
    if !index_path.exists() {
        return Ok(None);
    }

    let index = ToolchainIndex::read_from_path(&index_path)?;
    let meta = toolchain_index_meta_cache_path()
        .filter(|path| path.exists())
        .and_then(|path| match read_toolchain_index_cache_meta(&path) {
            Ok(meta) => Some(meta),
            Err(err) => {
                eprintln!("Warning: failed to read cached toolchain index metadata: {err}");
                None
            }
        });

    Ok(Some(CachedToolchainIndex { index, meta }))
}

fn read_toolchain_index_cache_meta(path: &Path) -> Result<ToolchainIndexCacheMeta> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read toolchain index metadata {}", path.display()))?;
    let meta: ToolchainIndexCacheMeta =
        serde_json::from_str(&json).context("failed to parse toolchain index metadata JSON")?;
    if meta.schema != 1 {
        bail!(
            "unsupported toolchain index metadata schema {}",
            meta.schema
        );
    }
    Ok(meta)
}

fn write_toolchain_index_cache(
    index: &ToolchainIndex,
    meta: &ToolchainIndexCacheMeta,
) -> Result<()> {
    let store_dir = toolchain_store_dir()?;
    fs::create_dir_all(&store_dir)?;

    let index_path = store_dir.join("index.json");
    let meta_path = store_dir.join("index-meta.json");

    fs::write(&index_path, index.to_pretty_json()?)
        .with_context(|| format!("failed to write {}", index_path.display()))?;

    let mut meta_json = serde_json::to_string_pretty(meta)?;
    meta_json.push('\n');
    fs::write(&meta_path, meta_json)
        .with_context(|| format!("failed to write {}", meta_path.display()))?;

    Ok(())
}

fn toolchain_index_cache_is_fresh(meta: &ToolchainIndexCacheMeta) -> bool {
    parse_toolchain_index_generated_at(&meta.fetched_at)
        .map(|fetched_at| Utc::now().signed_duration_since(fetched_at))
        .is_ok_and(|age| age <= ChronoDuration::hours(TOOLCHAIN_INDEX_CACHE_TTL_HOURS))
}

fn parse_toolchain_index_generated_at(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid RFC3339 timestamp `{value}`"))?
        .with_timezone(&Utc))
}

fn fetch_remote_toolchain_index(
    cached_meta: Option<&ToolchainIndexCacheMeta>,
) -> Result<RemoteToolchainIndex> {
    let client = Client::new();
    let token = env::var("GITHUB_TOKEN").ok();
    let mut errors = Vec::new();

    for repo in TOOLCHAIN_INDEX_REPOSITORIES {
        match fetch_remote_toolchain_index_from_repo(&client, &token, repo, cached_meta) {
            RepoFetchResult::Found(index) => return Ok(index),
            RepoFetchResult::NotModified => {
                let mut meta = cached_meta.cloned().context(
                    "remote toolchain index was not modified but no cache metadata exists",
                )?;
                meta.fetched_at = current_timestamp();
                return Ok(RemoteToolchainIndex::NotModified { meta });
            }
            RepoFetchResult::Missing => {}
            RepoFetchResult::Failed(err) => errors.push(err),
        }
    }

    if errors.is_empty() {
        bail!(
            "{} was not found in {}",
            TOOLCHAIN_INDEX_FILE,
            TOOLCHAIN_INDEX_REPOSITORIES.join(", ")
        );
    }

    bail!("{}", errors.join("; "))
}

fn fetch_remote_toolchain_index_from_repo(
    client: &Client,
    token: &Option<String>,
    repo: &str,
    cached_meta: Option<&ToolchainIndexCacheMeta>,
) -> RepoFetchResult<RemoteToolchainIndex> {
    let url = format!(
        "{}/contents/{}",
        github_api_base_for_repo(repo),
        TOOLCHAIN_INDEX_FILE
    );
    let mut request = client
        .get(&url)
        .header(USER_AGENT, crate::build_info::user_agent());

    if let Some(token) = token {
        request = request.header(AUTHORIZATION, format!("token {token}"));
    }

    if let Some(meta) = cached_meta
        && meta.source_repo == repo
    {
        if let Some(etag) = meta.etag.as_deref() {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = meta.last_modified.as_deref() {
            request = request.header(IF_MODIFIED_SINCE, last_modified);
        }
    }

    let response = match request.send() {
        Ok(response) => response,
        Err(err) => {
            return RepoFetchResult::Failed(format!(
                "could not reach GitHub contents API for {repo}: {err}"
            ));
        }
    };

    if response.status().as_u16() == 304 {
        return RepoFetchResult::NotModified;
    }
    if response.status().as_u16() == 404 {
        return RepoFetchResult::Missing;
    }
    if !response.status().is_success() {
        return RepoFetchResult::Failed(format!(
            "GitHub contents API returned {} for {repo}",
            response.status()
        ));
    }

    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let body: GitHubContentResponse = match response.json() {
        Ok(body) => body,
        Err(err) => {
            return RepoFetchResult::Failed(format!(
                "GitHub contents API returned invalid JSON for {repo}: {err}"
            ));
        }
    };

    let json = match decode_github_content(&body) {
        Ok(json) => json,
        Err(err) => {
            return RepoFetchResult::Failed(format!(
                "GitHub contents API returned invalid {TOOLCHAIN_INDEX_FILE} for {repo}: {err}"
            ));
        }
    };

    let index = match ToolchainIndex::from_json(&json) {
        Ok(index) => index,
        Err(err) => {
            return RepoFetchResult::Failed(format!(
                "GitHub contents API returned invalid toolchain index for {repo}: {err}"
            ));
        }
    };

    let meta = ToolchainIndexCacheMeta {
        schema: 1,
        fetched_at: current_timestamp(),
        source_repo: repo.to_owned(),
        source_ref: "default".to_owned(),
        etag,
        last_modified,
    };

    RepoFetchResult::Found(RemoteToolchainIndex::Fresh { index, meta })
}

fn decode_github_content(body: &GitHubContentResponse) -> Result<String> {
    if body.encoding != "base64" {
        bail!("unsupported content encoding `{}`", body.encoding);
    }

    let encoded = body.content.lines().collect::<String>();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("failed to decode base64 content")?;

    String::from_utf8(bytes).context("toolchain index content is not UTF-8")
}

fn github_api_base_for_repo(repo: &str) -> String {
    if let Some(base) = test_toolchain_github_api_base_override() {
        let base = base.trim().trim_end_matches('/');
        if !base.is_empty() {
            return format!("{base}/repos/{repo}");
        }
    }

    format!("https://api.github.com/repos/{repo}")
}

#[cfg(debug_assertions)]
fn test_toolchain_github_api_base_override() -> Option<String> {
    env::var(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV).ok()
}

#[cfg(not(debug_assertions))]
fn test_toolchain_github_api_base_override() -> Option<String> {
    None
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn project_request(config: Option<&ToolchainConfig>) -> Result<ToolchainRequest> {
    let Some(config) = config else {
        return Ok(ToolchainRequest::None);
    };

    let acton = config
        .acton
        .as_deref()
        .map(parse_project_acton_version)
        .transpose()?;
    let tolk = config
        .tolk
        .as_deref()
        .map(parse_project_tolk_version)
        .transpose()?;

    match (acton, tolk) {
        (None, None) => {
            bail!(
                "Acton.toml has an empty [toolchain] section. Set `acton = \"0.3.0\"` or `tolk = \"1.3.0\"`, or remove the section."
            );
        }
        (Some(acton), tolk) => Ok(ToolchainRequest::ProjectActon { acton, tolk }),
        (None, Some(tolk)) => Ok(ToolchainRequest::ProjectTolk { tolk }),
    }
}

fn resolve_acton_request(
    source: &'static str,
    acton: &str,
    requested_tolk: Option<&str>,
    environment: &ToolchainEnvironment,
) -> Result<ToolchainResolveReport> {
    let release = environment
        .index
        .as_ref()
        .and_then(|index| index.release_for_acton(acton));
    let installed = environment.installed.get(acton);
    let installed_metadata = installed.and_then(|installed| installed.release.as_ref());

    if let Some(release) = release
        && release.yanked
    {
        bail_yanked_release(release)?;
    }
    if let Some(metadata) = installed_metadata
        && metadata.yanked
    {
        bail_yanked_metadata(acton, metadata.yank_reason.as_deref())?;
    }

    let bundled_tolk = release
        .map(|release| release.tolk.as_str())
        .or_else(|| installed_metadata.map(|metadata| metadata.tolk.as_str()))
        .or_else(|| {
            (acton == environment.current_acton).then_some(environment.current_tolk.as_str())
        })
        .with_context(|| unknown_acton_message(acton, environment.index.as_ref()))?;

    if let Some(requested_tolk) = requested_tolk
        && bundled_tolk != requested_tolk
    {
        bail!(
            "{}",
            conflicting_project_pins_message(
                acton,
                requested_tolk,
                bundled_tolk,
                environment.index.as_ref()
            )
        );
    }

    Ok(report_for_acton(
        source,
        acton,
        bundled_tolk,
        release,
        installed,
        environment,
    ))
}

fn resolve_tolk_request(
    requested_tolk: &str,
    environment: &ToolchainEnvironment,
) -> Result<ToolchainResolveReport> {
    if environment.current_tolk == requested_tolk
        && !current_release_is_yanked(environment.index.as_ref(), environment)
    {
        return Ok(report_for_current("project-tolk", environment, false, None));
    }

    let index = environment.index.as_ref().with_context(|| {
        format!(
            "Could not resolve Tolk {requested_tolk} because the toolchain index is unavailable"
        )
    })?;

    let release = index
        .newest_supported_for_tolk(requested_tolk)?
        .with_context(|| unknown_tolk_message(requested_tolk, Some(index)))?;

    Ok(report_for_acton(
        "project-tolk",
        &release.acton,
        &release.tolk,
        Some(release),
        environment.installed.get(&release.acton),
        environment,
    ))
}

fn report_for_current(
    source: &'static str,
    environment: &ToolchainEnvironment,
    yanked: bool,
    yank_reason: Option<String>,
) -> ToolchainResolveReport {
    ToolchainResolveReport {
        source,
        acton: environment.current_acton.clone(),
        tolk: environment.current_tolk.clone(),
        current: true,
        installed: true,
        install_required: false,
        path: Some(environment.current_exe.display().to_string()),
        yanked,
        yank_reason,
    }
}

fn report_for_acton(
    source: &'static str,
    acton: &str,
    tolk: &str,
    release: Option<&ToolchainIndexRelease>,
    installed: Option<&InstalledToolchain>,
    environment: &ToolchainEnvironment,
) -> ToolchainResolveReport {
    let local_metadata = installed.and_then(|installed| installed.release.as_ref());
    let yanked = release.is_some_and(|release| release.yanked)
        || local_metadata.is_some_and(|metadata| metadata.yanked);
    let yank_reason = release
        .and_then(|release| release.yank_reason.clone())
        .or_else(|| local_metadata.and_then(|metadata| metadata.yank_reason.clone()));

    if acton == environment.current_acton {
        return report_for_current(source, environment, yanked, yank_reason);
    }

    ToolchainResolveReport {
        source,
        acton: acton.to_owned(),
        tolk: tolk.to_owned(),
        current: false,
        installed: installed.is_some(),
        install_required: installed.is_none(),
        path: installed.map(|installed| installed.binary_path.display().to_string()),
        yanked,
        yank_reason,
    }
}

fn current_release_is_yanked(
    index: Option<&ToolchainIndex>,
    environment: &ToolchainEnvironment,
) -> bool {
    index
        .and_then(|index| index.release_for_acton(&environment.current_acton))
        .is_some_and(|release| release.yanked)
}

fn bail_yanked_release(release: &ToolchainIndexRelease) -> Result<()> {
    match release.yank_reason.as_deref() {
        Some(reason) if !reason.trim().is_empty() => {
            bail!("Acton {} is yanked: {reason}", release.acton)
        }
        _ => bail!("Acton {} is yanked", release.acton),
    }
}

fn bail_yanked_metadata(acton: &str, reason: Option<&str>) -> Result<()> {
    match reason {
        Some(reason) if !reason.trim().is_empty() => {
            bail!("Acton {acton} is yanked: {reason}")
        }
        _ => bail!("Acton {acton} is yanked"),
    }
}

fn conflicting_project_pins_message(
    acton: &str,
    requested_tolk: &str,
    bundled_tolk: &str,
    index: Option<&ToolchainIndex>,
) -> String {
    let mut message = format!(
        "Acton.toml requests acton {acton} and tolk {requested_tolk}, but acton {acton} ships tolk {bundled_tolk}."
    );

    if let Some(index) = index {
        let known = index.supported_acton_versions_for_tolk(requested_tolk);
        if let Some(recommended) = known.last() {
            message.push_str(&format!(
                "\nKnown Acton releases for Tolk {requested_tolk}: {}.\nRecommended fix:\n  [toolchain]\n  acton = \"{recommended}\"\n  tolk = \"{requested_tolk}\"\n\nOr remove the `acton` pin and keep `tolk = \"{requested_tolk}\"` so Acton can choose the newest supported release.\nThen run `acton toolchain install`.",
                known.join(", ")
            ));
            return message;
        }
    }

    message.push_str(&format!(
        "\nChoose one fix:\n  1. Change `[toolchain].acton` to an Acton release that ships Tolk {requested_tolk}.\n  2. Remove `[toolchain].acton` and keep `tolk = \"{requested_tolk}\"` so Acton can choose the newest supported release.\nThen run `acton toolchain install`."
    ));
    message
}

fn unknown_acton_message(acton: &str, index: Option<&ToolchainIndex>) -> String {
    let mut message = format!("Unknown Acton toolchain version {acton}.");
    if let Some(index) = index {
        let known = index.known_acton_versions();
        if !known.is_empty() {
            message.push_str(&format!("\nKnown Acton versions: {}", known.join(", ")));
        }
    }
    message
}

fn unknown_tolk_message(tolk: &str, index: Option<&ToolchainIndex>) -> String {
    let mut message = format!("No supported Acton release ships Tolk {tolk}.");
    if let Some(index) = index {
        let known = index.known_tolk_versions();
        if !known.is_empty() {
            message.push_str(&format!("\nKnown Tolk versions: {}", known.join(", ")));
        }
    }
    message
}

fn parse_cli_selector(raw_selector: &str) -> Result<String> {
    let version = raw_selector.trim_start_matches('+');
    if version.is_empty() {
        bail!("Toolchain selector must include an Acton version, for example `+0.3.0`");
    }

    if version.contains('/') || version.contains('\\') {
        bail!("Toolchain selector must be an exact Acton version, got `{raw_selector}`");
    }

    parse_exact_semver("toolchain selector", version).map(|version| version.to_string())
}

pub fn normalize_explicit_acton_version(raw_version: &str) -> Result<String> {
    parse_project_acton_version(raw_version)
}

fn parse_project_acton_version(raw_version: &str) -> Result<String> {
    let raw_version = raw_version.trim();
    if raw_version == "trunk" {
        bail!(
            "Project toolchains do not support `acton = \"trunk\"`. Use an exact release such as `acton = \"0.3.0\"`."
        );
    }

    if raw_version.contains('/') || raw_version.contains('\\') {
        bail!("[toolchain].acton must be an exact Acton version, got `{raw_version}`");
    }

    let version = raw_version.strip_prefix('v').unwrap_or(raw_version);
    parse_exact_semver("[toolchain].acton", version).map(|version| version.to_string())
}

fn parse_project_tolk_version(raw_version: &str) -> Result<String> {
    let raw_version = raw_version.trim();
    parse_exact_semver("[toolchain].tolk", raw_version).map(|version| version.to_string())
}

fn parse_exact_semver(field: &str, value: &str) -> Result<Version> {
    let version =
        Version::parse(value).with_context(|| format!("{field} must be an exact X.Y.Z version"))?;

    if !version.pre.is_empty() || !version.build.is_empty() {
        bail!("{field} must not include pre-release or build metadata: {value}");
    }

    Ok(version)
}

fn first_command_arg(args: &[OsString]) -> Option<String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if is_help_or_version_flag(arg) {
            return Some(help_or_version_command_name(arg).to_owned());
        }

        let arg = arg.to_string_lossy();
        match arg.as_ref() {
            "--color" | "--manifest-path" | "--project-root" => {
                let _ = iter.next();
            }
            _ if arg.starts_with("--color=")
                || arg.starts_with("--manifest-path=")
                || arg.starts_with("--project-root=") => {}
            _ if arg.starts_with('-') => {}
            _ => return Some(arg.into_owned()),
        }
    }
    None
}

fn is_help_or_version_flag(arg: &OsStr) -> bool {
    matches!(
        arg.to_str(),
        Some("-h" | "--help" | "-V" | "--version" | "help")
    )
}

fn help_or_version_command_name(arg: &OsStr) -> &'static str {
    match arg.to_str() {
        Some("-V" | "--version") => "version",
        _ => "help",
    }
}

pub fn toolchain_index_cache_path() -> Option<PathBuf> {
    Some(optional_toolchain_store_dir()?.join("index.json"))
}

pub fn toolchain_index_meta_cache_path() -> Option<PathBuf> {
    Some(optional_toolchain_store_dir()?.join("index-meta.json"))
}

fn scan_installed_toolchains() -> BTreeMap<String, InstalledToolchain> {
    let Some(store_dir) = optional_toolchain_store_dir() else {
        return BTreeMap::new();
    };

    let Ok(entries) = fs::read_dir(store_dir) else {
        return BTreeMap::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let version = entry.file_name().to_string_lossy().to_string();
            if parse_exact_semver("installed Acton version", &version).is_err() {
                return None;
            }

            let binary_path = entry.path().join(acton_binary_name());
            binary_path.is_file().then(|| {
                let release = read_installed_toolchain_metadata(&entry.path())
                    .ok()
                    .flatten();
                (
                    version,
                    InstalledToolchain {
                        binary_path,
                        release,
                    },
                )
            })
        })
        .collect()
}

fn read_installed_toolchain_metadata(dir: &Path) -> Result<Option<ToolchainReleaseMetadata>> {
    let path = dir.join("release.json");
    if !path.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&path)
        .with_context(|| format!("failed to read toolchain metadata {}", path.display()))?;
    let metadata: ToolchainReleaseMetadata =
        serde_json::from_str(&json).context("failed to parse toolchain metadata JSON")?;
    if metadata.schema != 1 {
        bail!("unsupported toolchain metadata schema {}", metadata.schema);
    }
    parse_exact_semver("toolchain metadata acton version", &metadata.acton)?;
    parse_exact_semver("toolchain metadata tolk version", &metadata.tolk)?;
    Ok(Some(metadata))
}

pub fn toolchain_store_dir() -> Result<PathBuf> {
    optional_toolchain_store_dir()
        .context("Could not determine HOME directory for Acton toolchain store")
}

pub fn installed_toolchain_dir(acton_version: &str) -> Result<PathBuf> {
    Ok(toolchain_store_dir()?.join(acton_version))
}

pub fn installed_toolchain_binary_path(acton_version: &str) -> Result<PathBuf> {
    Ok(installed_toolchain_dir(acton_version)?.join(acton_binary_name()))
}

fn optional_toolchain_store_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".acton").join("toolchains"))
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = env::var_os("HOME");

    home.map(PathBuf::from)
}

const fn acton_binary_name() -> &'static str {
    if cfg!(windows) { "acton.exe" } else { "acton" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn release(acton: &str, tolk: &str, yanked: bool) -> ToolchainIndexRelease {
        ToolchainIndexRelease {
            acton: acton.to_owned(),
            tolk: tolk.to_owned(),
            tag: format!("v{acton}"),
            stable: true,
            yanked,
            yank_reason: yanked.then(|| "broken release".to_owned()),
        }
    }

    fn environment(index: ToolchainIndex) -> ToolchainEnvironment {
        ToolchainEnvironment {
            current_acton: "0.3.1".to_owned(),
            current_tolk: "1.3.0".to_owned(),
            current_exe: PathBuf::from("/bin/acton"),
            index: Some(index),
            index_warning: None,
            installed: BTreeMap::new(),
        }
    }

    fn sample_index() -> ToolchainIndex {
        ToolchainIndex {
            schema: 1,
            generated_at: "2026-04-24T00:00:00Z".to_owned(),
            releases: vec![
                release("0.3.0", "1.2.0", false),
                release("0.3.1", "1.3.0", false),
                release("0.3.2", "1.3.0", true),
                release("0.3.3", "1.3.0", false),
            ],
        }
    }

    #[test]
    fn strips_leading_cli_selector() {
        let parsed =
            strip_leading_toolchain_selector(os_args(&["acton", "+0.3.0", "test"])).unwrap();

        assert_eq!(
            parsed.selector,
            Some(CliToolchainSelector {
                acton: "0.3.0".to_owned()
            })
        );
        assert_eq!(parsed.args, os_args(&["acton", "test"]));
    }

    #[test]
    fn selector_must_be_first_argument() {
        let parsed =
            strip_leading_toolchain_selector(os_args(&["acton", "test", "+0.3.0"])).unwrap();

        assert_eq!(parsed.selector, None);
        assert_eq!(parsed.args, os_args(&["acton", "test", "+0.3.0"]));
    }

    #[test]
    fn rejects_partial_cli_selector() {
        let err = strip_leading_toolchain_selector(os_args(&["acton", "+0.3", "test"]))
            .expect_err("partial selector must fail");

        assert!(err.to_string().contains("toolchain selector"));
    }

    #[test]
    fn rejects_selector_for_up_command() {
        let parsed = strip_leading_toolchain_selector(os_args(&["acton", "+0.3.0", "up"])).unwrap();
        let err = ensure_selector_allowed_for_args(parsed.selector.as_ref(), &parsed.args)
            .expect_err("up command must reject selector");

        assert!(err.to_string().contains("acton +<version> up"));
    }

    #[test]
    fn project_acton_request_current_version_selects_current_binary() {
        let env = environment(sample_index());
        let config = ToolchainConfig {
            acton: Some("0.3.1".to_owned()),
            tolk: None,
        };

        let report = resolve_toolchain(Some(&config), None, &env).unwrap();

        assert_eq!(report.source, "project-acton");
        assert!(report.current);
        assert!(!report.install_required);
    }

    #[test]
    fn cli_selector_overrides_project_toolchain() {
        let env = environment(sample_index());
        let config = ToolchainConfig {
            acton: Some("0.3.0".to_owned()),
            tolk: Some("1.2.0".to_owned()),
        };
        let selector = CliToolchainSelector {
            acton: "0.3.1".to_owned(),
        };

        let report = resolve_toolchain(Some(&config), Some(&selector), &env).unwrap();

        assert_eq!(report.source, "cli-plus");
        assert_eq!(report.acton, "0.3.1");
        assert_eq!(report.tolk, "1.3.0");
    }

    #[test]
    fn project_tolk_request_uses_current_binary_when_it_matches() {
        let env = environment(sample_index());
        let config = ToolchainConfig {
            acton: None,
            tolk: Some("1.3.0".to_owned()),
        };

        let report = resolve_toolchain(Some(&config), None, &env).unwrap();

        assert_eq!(report.source, "project-tolk");
        assert_eq!(report.acton, "0.3.1");
        assert!(report.current);
    }

    #[test]
    fn project_tolk_request_selects_newest_non_yanked_release() {
        let mut env = environment(sample_index());
        env.current_tolk = "1.2.0".to_owned();
        let config = ToolchainConfig {
            acton: None,
            tolk: Some("1.3.0".to_owned()),
        };

        let report = resolve_toolchain(Some(&config), None, &env).unwrap();

        assert_eq!(report.acton, "0.3.3");
        assert!(!report.installed);
        assert!(report.install_required);
    }

    #[test]
    fn project_acton_tolk_conflict_fails() {
        let env = environment(sample_index());
        let config = ToolchainConfig {
            acton: Some("0.3.0".to_owned()),
            tolk: Some("1.3.0".to_owned()),
        };

        let err =
            resolve_toolchain(Some(&config), None, &env).expect_err("conflicting pins must fail");

        assert!(err.to_string().contains("ships tolk 1.2.0"));
    }

    #[test]
    fn yanked_explicit_acton_fails() {
        let env = environment(sample_index());
        let config = ToolchainConfig {
            acton: Some("0.3.2".to_owned()),
            tolk: None,
        };

        let err = resolve_toolchain(Some(&config), None, &env).expect_err("yanked Acton must fail");

        assert!(err.to_string().contains("is yanked"));
    }
}
