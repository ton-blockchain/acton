use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::EnvironmentRuntimeError;

const COMPOSE_TEMPLATE: &str = include_str!("../assets/full-ton-network.compose.yaml");
const DEFAULT_MYLOCALACTON_IMAGE: &str =
    "ghcr.io/i582/mylocalacton:sha-bf02368cf822b311aa89ba8bc599fa0a6b90accb";
const COMPOSE_WAIT_TIMEOUT_SECONDS: u16 = 600;
const DOCKER_CONFIG_DIRECTORY: &str = "docker-pull-config";
const STARTUP_LOG_FILE: &str = "startup.log";
const STARTUP_ERROR_LINES: usize = 12;
const DOCKER_METADATA_TIMEOUT: Duration = Duration::from_secs(10);
const COMPOSE_STOP_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const COMPOSE_DELETE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(crate) struct FullTonNetworkDriver {
    compose_file: PathBuf,
    docker_target: DockerTarget,
    isolated_docker_config_dir: Option<PathBuf>,
    image: String,
    project_name: String,
    startup_log_file: PathBuf,
}

enum DockerTarget {
    Context(String),
    Host(String),
}

pub(crate) struct IsolatedPullTarget {
    docker_host: String,
    platform: String,
}

impl FullTonNetworkDriver {
    pub(crate) async fn materialize(
        data_dir: &Path,
        environment_id: &str,
        api_v2_port: u16,
        api_v3_port: u16,
        validators: u16,
    ) -> Result<Self, EnvironmentRuntimeError> {
        let image = std::env::var("ACTON_STUDIO_MYLOCALACTON_IMAGE")
            .unwrap_or_else(|_| DEFAULT_MYLOCALACTON_IMAGE.to_owned());
        validate_image_reference(&image)?;
        let docker_target = resolve_docker_target().await?;

        let compose_file = data_dir.join("compose.yaml");
        let isolated_docker_config_dir = if image == DEFAULT_MYLOCALACTON_IMAGE {
            let path = data_dir.join(DOCKER_CONFIG_DIRECTORY);
            tokio::fs::create_dir_all(&path).await.map_err(|error| {
                EnvironmentRuntimeError::Internal {
                    code: "environment_storage_failed",
                    message: format!(
                        "Failed to create isolated Docker pull configuration at {}: {error}",
                        path.display()
                    ),
                }
            })?;
            Some(path)
        } else {
            None
        };
        let compose = render_compose(&image, api_v2_port, api_v3_port, validators);
        tokio::fs::write(&compose_file, compose)
            .await
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_storage_failed",
                message: format!(
                    "Failed to write the full TON network definition at {}: {error}",
                    compose_file.display()
                ),
            })?;

        Ok(Self {
            compose_file,
            docker_target,
            isolated_docker_config_dir,
            image,
            project_name: compose_project_name(environment_id),
            startup_log_file: data_dir.join(STARTUP_LOG_FILE),
        })
    }

    pub(crate) fn spawn_normal_pull(&self) -> Result<Child, EnvironmentRuntimeError> {
        let mut command = self.normal_pull_command();
        self.spawn_logged(
            &mut command,
            true,
            "pull the full TON network image with Docker",
        )
    }

    pub(crate) fn spawn_image_inspect(&self) -> Result<Child, EnvironmentRuntimeError> {
        let mut command = self.image_inspect_command();
        self.spawn_logged(
            &mut command,
            true,
            "inspect the full TON network image with Docker",
        )
    }

    pub(crate) fn spawn_isolated_pull(
        &self,
        target: &IsolatedPullTarget,
    ) -> Result<Child, EnvironmentRuntimeError> {
        let mut command = self.isolated_pull_command(target).ok_or_else(|| {
            EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: "The isolated Docker pull is unavailable for a custom image".to_owned(),
            }
        })?;
        self.spawn_logged(
            &mut command,
            false,
            "pull the public full TON network image with an isolated Docker configuration",
        )
    }

    pub(crate) fn spawn_compose_up(&self) -> Result<Child, EnvironmentRuntimeError> {
        let mut command = self.compose_command();
        command
            .arg("up")
            .arg("-d")
            .arg("--wait")
            .arg("--wait-timeout")
            .arg(COMPOSE_WAIT_TIMEOUT_SECONDS.to_string());
        self.spawn_logged(
            &mut command,
            false,
            "start the full TON network with Docker Compose",
        )
    }

    fn spawn_logged(
        &self,
        command: &mut Command,
        truncate: bool,
        operation: &str,
    ) -> Result<Child, EnvironmentRuntimeError> {
        let stdout = OpenOptions::new()
            .create(true)
            .truncate(truncate)
            .append(!truncate)
            .write(true)
            .open(&self.startup_log_file)
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!(
                    "Failed to open Docker startup log at {}: {error}",
                    self.startup_log_file.display()
                ),
            })?;
        let stderr = stdout
            .try_clone()
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!(
                    "Failed to open Docker startup log at {}: {error}",
                    self.startup_log_file.display()
                ),
            })?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        command
            .spawn()
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!("Failed to {operation}: {error}"),
            })
    }

    pub(crate) async fn startup_failure_message(
        &self,
        operation: &str,
        status: ExitStatus,
    ) -> String {
        let base_message = format!("Docker exited with {status} while trying to {operation}");
        let Ok(output) = tokio::fs::read_to_string(&self.startup_log_file).await else {
            return base_message;
        };
        let lines = output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(STARTUP_ERROR_LINES)
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return base_message;
        }
        let details = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
        format!("{base_message}:\n{details}")
    }

    pub(crate) async fn stop(&self) -> Result<(), EnvironmentRuntimeError> {
        self.run_compose(
            ["stop"],
            "stop",
            "environment_stop_failed",
            COMPOSE_STOP_TIMEOUT,
        )
        .await
    }

    pub(crate) async fn delete(&self) -> Result<(), EnvironmentRuntimeError> {
        self.run_compose(
            ["down", "--volumes", "--remove-orphans"],
            "delete",
            "environment_delete_failed",
            COMPOSE_DELETE_TIMEOUT,
        )
        .await
    }

    pub(crate) async fn isolated_pull_target(
        &self,
    ) -> Result<Option<IsolatedPullTarget>, EnvironmentRuntimeError> {
        if self.isolated_docker_config_dir.is_none() {
            return Ok(None);
        }

        let docker_host = match &self.docker_target {
            DockerTarget::Host(host) => host.clone(),
            DockerTarget::Context(context) => {
                self.docker_text([
                    "context",
                    "inspect",
                    "--format",
                    "{{.Endpoints.docker.Host}}",
                    context,
                ])
                .await?
            }
        };
        if !docker_host.starts_with("unix://") {
            return Err(EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!(
                    "The isolated image pull is unsafe for Docker endpoint {docker_host}"
                ),
            });
        }

        let server_platform = self
            .docker_text(["version", "--format", "{{.Server.Os}}/{{.Server.Arch}}"])
            .await?;
        let platform = match server_platform.as_str() {
            "linux/arm64" | "linux/aarch64" => "linux/arm64",
            "linux/amd64" | "linux/x86_64" => "linux/amd64",
            _ => {
                return Err(EnvironmentRuntimeError::Internal {
                    code: "environment_start_failed",
                    message: format!(
                        "The isolated image pull does not support Docker platform {server_platform}"
                    ),
                });
            }
        };

        Ok(Some(IsolatedPullTarget {
            docker_host,
            platform: platform.to_owned(),
        }))
    }

    fn normal_pull_command(&self) -> Command {
        let mut command = self.docker_command();
        command.arg("pull").arg(&self.image);
        command
    }

    fn image_inspect_command(&self) -> Command {
        let mut command = self.docker_command();
        command
            .arg("image")
            .arg("inspect")
            .arg("--format")
            .arg("{{.Id}}")
            .arg(&self.image);
        command
    }

    fn isolated_pull_command(&self, target: &IsolatedPullTarget) -> Option<Command> {
        let config_dir = self.isolated_docker_config_dir.as_ref()?;
        let mut command = Command::new("docker");
        command
            .arg("--config")
            .arg(config_dir)
            .arg("--host")
            .arg(&target.docker_host)
            .arg("pull")
            .arg("--platform")
            .arg(&target.platform)
            .arg(&self.image);
        Some(command)
    }

    fn compose_command(&self) -> Command {
        let mut command = self.docker_command();
        command
            .arg("compose")
            .arg("-p")
            .arg(&self.project_name)
            .arg("-f")
            .arg(&self.compose_file);
        command
    }

    fn docker_command(&self) -> Command {
        let mut command = Command::new("docker");
        match &self.docker_target {
            DockerTarget::Context(context) => {
                command.arg("--context").arg(context);
            }
            DockerTarget::Host(host) => {
                command.arg("--host").arg(host);
            }
        }
        command
    }

    async fn docker_text<I, S>(&self, args: I) -> Result<String, EnvironmentRuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.docker_command();
        command.args(args);
        docker_text(command).await
    }

    async fn run_compose<I, S>(
        &self,
        args: I,
        operation: &str,
        code: &'static str,
        operation_timeout: Duration,
    ) -> Result<(), EnvironmentRuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.compose_command();
        command.args(args).stdin(Stdio::null()).kill_on_drop(true);
        let output = timeout(operation_timeout, command.output())
            .await
            .map_err(|_| EnvironmentRuntimeError::Internal {
                code,
                message: format!(
                    "Timed out after {} seconds while trying to {operation} the full TON network with Docker Compose",
                    operation_timeout.as_secs()
                ),
            })?
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code,
                message: format!(
                    "Failed to {operation} the full TON network with Docker Compose: {error}"
                ),
            })?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = stderr.trim();
        Err(EnvironmentRuntimeError::Internal {
            code,
            message: if details.is_empty() {
                format!(
                    "Docker Compose could not {operation} the full TON network ({})",
                    output.status
                )
            } else {
                format!("Docker Compose could not {operation} the full TON network: {details}")
            },
        })
    }
}

async fn resolve_docker_target() -> Result<DockerTarget, EnvironmentRuntimeError> {
    match std::env::var("DOCKER_CONTEXT") {
        Ok(context) if !context.is_empty() => return Ok(DockerTarget::Context(context)),
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(EnvironmentRuntimeError::InvalidRequest {
                code: "environment_docker_context_invalid",
                message: "DOCKER_CONTEXT must contain valid UTF-8".to_owned(),
            });
        }
    }

    match std::env::var("DOCKER_HOST") {
        Ok(host) if !host.is_empty() => return Ok(DockerTarget::Host(host)),
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(EnvironmentRuntimeError::InvalidRequest {
                code: "environment_docker_host_invalid",
                message: "DOCKER_HOST must contain valid UTF-8".to_owned(),
            });
        }
    }

    let mut command = Command::new("docker");
    command.args(["context", "show"]);
    docker_text(command).await.map(DockerTarget::Context)
}

async fn docker_text(mut command: Command) -> Result<String, EnvironmentRuntimeError> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    let output = timeout(DOCKER_METADATA_TIMEOUT, command.output())
        .await
        .map_err(|_| EnvironmentRuntimeError::Internal {
            code: "environment_start_failed",
            message: "Timed out while inspecting the active Docker context".to_owned(),
        })?
        .map_err(|error| EnvironmentRuntimeError::Internal {
            code: "environment_start_failed",
            message: format!("Failed to inspect the active Docker context: {error}"),
        })?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        return Err(EnvironmentRuntimeError::Internal {
            code: "environment_start_failed",
            message: format!("Docker context inspection failed: {}", details.trim()),
        });
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if value.is_empty() {
        return Err(EnvironmentRuntimeError::Internal {
            code: "environment_start_failed",
            message: "Docker context inspection returned an empty value".to_owned(),
        });
    }
    Ok(value)
}

fn render_compose(image: &str, api_v2_port: u16, api_v3_port: u16, validators: u16) -> String {
    COMPOSE_TEMPLATE
        .replace("__MYLOCALACTON_IMAGE__", image)
        .replace("__MYLOCALACTON_V2_PORT__", &api_v2_port.to_string())
        .replace("__MYLOCALACTON_V3_PORT__", &api_v3_port.to_string())
        .replace("__MYLOCALACTON_VALIDATORS__", &validators.to_string())
}

fn compose_project_name(environment_id: &str) -> String {
    format!("acton-studio-{environment_id}")
}

fn validate_image_reference(image: &str) -> Result<(), EnvironmentRuntimeError> {
    if image.is_empty()
        || !image
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/:@-".contains(&byte))
    {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_image_invalid",
            message: "ACTON_STUDIO_MYLOCALACTON_IMAGE is not a valid container image reference"
                .to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use std::path::PathBuf;

    use super::{
        DEFAULT_MYLOCALACTON_IMAGE, DockerTarget, FullTonNetworkDriver, IsolatedPullTarget,
        compose_project_name, render_compose,
    };

    #[test]
    fn compose_definition_uses_environment_specific_runtime_values() {
        let compose = render_compose("registry.example/ton:build-42", 18180, 18181, 3);
        let selected_lines = compose
            .lines()
            .filter(|line| {
                line.contains("registry.example")
                    || line.contains("127.0.0.1:1818")
                    || line.contains("\"3\"")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let actual = format!(
            "project: {}\n{selected_lines}",
            compose_project_name("environment-7")
        );

        expect![[r#"project: acton-studio-environment-7
    image: "registry.example/ton:build-42"
      - "3"
      - "127.0.0.1:18180:18080"
    image: "registry.example/ton:build-42"
    image: "registry.example/ton:build-42"
    image: "registry.example/ton:build-42"
      - "127.0.0.1:18181:8081"
    image: "registry.example/ton:build-42""#]]
        .assert_eq(&actual);
        assert!(!compose.contains("platform:"));
    }

    #[test]
    fn lifecycle_commands_pin_the_environment_docker_context() {
        let driver = test_driver(DEFAULT_MYLOCALACTON_IMAGE, true);
        let command = driver.compose_command();
        let actual = command_args(&command);

        expect![[r"--context
desktop-linux
compose
-p
acton-studio-environment-1
-f
/workspace/.studio/environment-1/compose.yaml"]]
        .assert_eq(&actual);
    }

    #[test]
    fn isolated_pull_is_scoped_to_the_default_public_image() {
        let target = IsolatedPullTarget {
            docker_host: "unix:///docker.sock".to_owned(),
            platform: "linux/arm64".to_owned(),
        };
        let default_driver = test_driver(DEFAULT_MYLOCALACTON_IMAGE, true);
        let inspect = command_args(&default_driver.image_inspect_command());
        let normal = command_args(&default_driver.normal_pull_command());
        let isolated = command_args(
            &default_driver
                .isolated_pull_command(&target)
                .expect("default public image must support an isolated pull"),
        );
        let custom_driver = test_driver("registry.example/private/ton:42", false);
        let actual = format!(
            "INSPECT\n{inspect}\n\nNORMAL\n{normal}\n\nISOLATED\n{isolated}\n\nCUSTOM ISOLATED: {}",
            custom_driver.isolated_pull_command(&target).is_some()
        );

        expect![[r"INSPECT
--context
desktop-linux
image
inspect
--format
{{.Id}}
ghcr.io/i582/mylocalacton:sha-bf02368cf822b311aa89ba8bc599fa0a6b90accb

NORMAL
--context
desktop-linux
pull
ghcr.io/i582/mylocalacton:sha-bf02368cf822b311aa89ba8bc599fa0a6b90accb

ISOLATED
--config
/workspace/.studio/environment-1/docker-pull-config
--host
unix:///docker.sock
pull
--platform
linux/arm64
ghcr.io/i582/mylocalacton:sha-bf02368cf822b311aa89ba8bc599fa0a6b90accb

CUSTOM ISOLATED: false"]]
        .assert_eq(&actual);
    }

    fn test_driver(image: &str, supports_isolated_pull: bool) -> FullTonNetworkDriver {
        FullTonNetworkDriver {
            compose_file: PathBuf::from("/workspace/.studio/environment-1/compose.yaml"),
            docker_target: DockerTarget::Context("desktop-linux".to_owned()),
            isolated_docker_config_dir: supports_isolated_pull
                .then(|| PathBuf::from("/workspace/.studio/environment-1/docker-pull-config")),
            image: image.to_owned(),
            project_name: "acton-studio-environment-1".to_owned(),
            startup_log_file: PathBuf::from("/workspace/.studio/environment-1/startup.log"),
        }
    }

    fn command_args(command: &tokio::process::Command) -> String {
        command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
