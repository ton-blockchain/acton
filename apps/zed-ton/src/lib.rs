use std::fs;

use semver::Version;
use zed::settings::LspSettings;
use zed_extension_api as zed;

const ACTON_REPOSITORY: &str = "ton-blockchain/acton";
const DEFAULT_ARGUMENTS: &[&str] = &["ls", "--stdio"];

struct LanguageServerBinary {
    path: String,
    arguments: Vec<String>,
    environment: Vec<(String, String)>,
}

#[derive(Default)]
struct TonExtension {
    cached_binary_path: Option<String>,
}

impl TonExtension {
    fn language_server_binary(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<LanguageServerBinary> {
        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let configured_binary = settings.binary;
        let arguments = configured_binary
            .as_ref()
            .and_then(|binary| binary.arguments.clone())
            .unwrap_or_else(default_arguments);
        let mut environment: Vec<_> = configured_binary
            .as_ref()
            .and_then(|binary| binary.env.clone())
            .unwrap_or_default()
            .into_iter()
            .collect();
        environment.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let path = if let Some(path) = configured_binary.and_then(|binary| binary.path) {
            path
        } else if let Some(path) = worktree.which("acton") {
            path
        } else {
            self.managed_binary_path(language_server_id)?
        };

        Ok(LanguageServerBinary {
            path,
            arguments,
            environment,
        })
    }

    fn managed_binary_path(
        &mut self,
        language_server_id: &zed::LanguageServerId,
    ) -> zed::Result<String> {
        if let Some(path) = self
            .cached_binary_path
            .as_ref()
            .filter(|path| is_file(path))
        {
            return Ok(path.clone());
        }

        let installed_binary = latest_installed_binary(".");
        let (os, architecture) = zed::current_platform();
        let asset_name = release_asset_name(os, architecture)?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = match zed::latest_github_release(
            ACTON_REPOSITORY,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        ) {
            Ok(release) => release,
            Err(error) => {
                return installed_binary.ok_or_else(|| {
                    format!(
                        "failed to check the latest Acton release and no managed installation is available: {error}"
                    )
                });
            }
        };

        let version_directory = format!("acton-{}", release.version);
        let binary_path = format!("{version_directory}/acton");
        let fallback_binary = installed_binary.filter(|path| path != &binary_path);
        if !is_file(&binary_path) {
            let Some(asset) = release.assets.iter().find(|asset| asset.name == asset_name) else {
                return fallback_binary.ok_or_else(|| {
                    format!(
                        "Acton {} does not provide the required release asset `{asset_name}`",
                        release.version
                    )
                });
            };

            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            if let Err(error) = zed::download_file(
                &asset.download_url,
                &version_directory,
                zed::DownloadedFileType::GzipTar,
            ) {
                return fallback_binary.ok_or_else(|| {
                    format!("failed to download Acton {}: {error}", release.version)
                });
            }
        }
        if let Err(error) = zed::make_file_executable(&binary_path) {
            return fallback_binary
                .ok_or_else(|| format!("failed to make `{binary_path}` executable: {error}"));
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    fn lsp_settings(
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<LspSettings> {
        LspSettings::for_worktree(language_server_id.as_ref(), worktree)
    }
}

impl zed::Extension for TonExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        match self.language_server_binary(language_server_id, worktree) {
            Ok(binary) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::None,
                );
                Ok(zed::Command {
                    command: binary.path,
                    args: binary.arguments,
                    env: binary.environment,
                })
            }
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::Failed(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(Self::lsp_settings(language_server_id, worktree)?.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        Ok(Self::lsp_settings(language_server_id, worktree)?.settings)
    }
}

fn default_arguments() -> Vec<String> {
    DEFAULT_ARGUMENTS
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
}

fn release_asset_name(os: zed::Os, architecture: zed::Architecture) -> zed::Result<String> {
    let architecture = match architecture {
        zed::Architecture::Aarch64 => "aarch64",
        zed::Architecture::X8664 => "x86_64",
        zed::Architecture::X86 => {
            return Err(
                "Acton does not publish 32-bit binaries; configure `lsp.ton-ls.binary.path`"
                    .to_owned(),
            );
        }
    };
    let operating_system = match os {
        zed::Os::Mac => "apple-darwin",
        zed::Os::Linux => "unknown-linux-gnu",
        zed::Os::Windows => {
            return Err(
                "Acton does not publish Windows binaries; install Acton manually and configure `lsp.ton-ls.binary.path`"
                    .to_owned(),
            );
        }
    };
    Ok(format!("acton-{architecture}-{operating_system}.tar.gz"))
}

fn latest_installed_binary(directory: &str) -> Option<String> {
    fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name().into_string().ok()?;
            let version =
                Version::parse(file_name.strip_prefix("acton-")?.trim_start_matches('v')).ok()?;
            let binary_path = format!("{directory}/{file_name}/acton");
            is_file(&binary_path).then_some((version, binary_path))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, path)| path)
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

zed::register_extension!(TonExtension);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_release_assets() {
        assert_eq!(
            release_asset_name(zed::Os::Mac, zed::Architecture::Aarch64).unwrap(),
            "acton-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            release_asset_name(zed::Os::Linux, zed::Architecture::X8664).unwrap(),
            "acton-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn rejects_platforms_without_official_binaries() {
        assert!(release_asset_name(zed::Os::Windows, zed::Architecture::X8664).is_err());
        assert!(release_asset_name(zed::Os::Linux, zed::Architecture::X86).is_err());
    }
}
