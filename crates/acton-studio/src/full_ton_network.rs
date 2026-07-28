use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::EnvironmentRuntimeError;

const COMPOSE_TEMPLATE: &str = include_str!("../assets/full-ton-network.compose.yaml");
const DEFAULT_MYLOCALACTON_IMAGE: &str =
    "ghcr.io/i582/mylocalacton:sha-888cf44289a95d664c6a3fc739b4f2f0b733ab15";
const COMPOSE_WAIT_TIMEOUT_SECONDS: u16 = 600;
const DOCKER_CONFIG_DIRECTORY: &str = "docker-config";
const STARTUP_LOG_FILE: &str = "startup.log";
const STARTUP_ERROR_LINES: usize = 12;

pub(crate) struct FullTonNetworkDriver {
    compose_file: PathBuf,
    docker_config_dir: PathBuf,
    project_name: String,
    startup_log_file: PathBuf,
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
        let platform = docker_platform()?;

        let compose_file = data_dir.join("compose.yaml");
        let docker_config_dir = data_dir.join(DOCKER_CONFIG_DIRECTORY);
        tokio::fs::create_dir_all(&docker_config_dir)
            .await
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_storage_failed",
                message: format!(
                    "Failed to create isolated Docker configuration at {}: {error}",
                    docker_config_dir.display()
                ),
            })?;
        let compose = render_compose(&image, &platform, api_v2_port, api_v3_port, validators);
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
            docker_config_dir,
            project_name: compose_project_name(environment_id),
            startup_log_file: data_dir.join(STARTUP_LOG_FILE),
        })
    }

    pub(crate) fn spawn_up(&self) -> Result<Child, EnvironmentRuntimeError> {
        let stdout = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.startup_log_file)
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!(
                    "Failed to create Docker startup log at {}: {error}",
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
        let mut command = self.command();
        command
            .arg("up")
            .arg("-d")
            .arg("--wait")
            .arg("--wait-timeout")
            .arg(COMPOSE_WAIT_TIMEOUT_SECONDS.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        command
            .spawn()
            .map_err(|error| EnvironmentRuntimeError::Internal {
                code: "environment_start_failed",
                message: format!(
                    "Failed to start the full TON network with Docker Compose: {error}"
                ),
            })
    }

    pub(crate) async fn startup_failure_message(&self, status: ExitStatus) -> String {
        let fallback =
            format!("Docker Compose exited with {status} while starting the full TON network");
        let Ok(output) = tokio::fs::read_to_string(&self.startup_log_file).await else {
            return fallback;
        };
        let lines = output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(STARTUP_ERROR_LINES)
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return fallback;
        }
        let details = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
        format!("{fallback}:\n{details}")
    }

    pub(crate) async fn stop(&self) -> Result<(), EnvironmentRuntimeError> {
        self.run_compose(["stop"], "stop", "environment_stop_failed")
            .await
    }

    pub(crate) async fn delete(&self) -> Result<(), EnvironmentRuntimeError> {
        self.run_compose(
            ["down", "--volumes", "--remove-orphans"],
            "delete",
            "environment_delete_failed",
        )
        .await
    }

    fn command(&self) -> Command {
        let mut command = Command::new("docker");
        command
            .arg("--config")
            .arg(&self.docker_config_dir)
            .arg("compose")
            .arg("-p")
            .arg(&self.project_name)
            .arg("-f")
            .arg(&self.compose_file);
        command
    }

    async fn run_compose<I, S>(
        &self,
        args: I,
        operation: &str,
        code: &'static str,
    ) -> Result<(), EnvironmentRuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .await
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

fn render_compose(
    image: &str,
    platform: &str,
    api_v2_port: u16,
    api_v3_port: u16,
    validators: u16,
) -> String {
    COMPOSE_TEMPLATE
        .replace("__MYLOCALACTON_IMAGE__", image)
        .replace("__MYLOCALACTON_PLATFORM__", platform)
        .replace("__MYLOCALACTON_V2_PORT__", &api_v2_port.to_string())
        .replace("__MYLOCALACTON_V3_PORT__", &api_v3_port.to_string())
        .replace("__MYLOCALACTON_VALIDATORS__", &validators.to_string())
}

fn docker_platform() -> Result<String, EnvironmentRuntimeError> {
    let platform = std::env::var("ACTON_STUDIO_DOCKER_PLATFORM").unwrap_or_else(|_| {
        match std::env::consts::ARCH {
            "aarch64" => "linux/arm64",
            "x86_64" => "linux/amd64",
            architecture => architecture,
        }
        .to_owned()
    });
    match platform.as_str() {
        "linux/arm64" | "linux/amd64" => Ok(platform),
        _ => Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_platform_unsupported",
            message: format!(
                "Full TON networks support linux/arm64 and linux/amd64 images, not {platform}"
            ),
        }),
    }
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

    use super::{FullTonNetworkDriver, compose_project_name, render_compose};

    #[test]
    fn compose_definition_uses_environment_specific_runtime_values() {
        let compose = render_compose(
            "registry.example/ton:build-42",
            "linux/arm64",
            18180,
            18181,
            3,
        );
        let selected_lines = compose
            .lines()
            .filter(|line| {
                line.contains("registry.example")
                    || line.contains("platform:")
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
    platform: "linux/arm64"
      - "3"
      - "127.0.0.1:18180:18080"
    image: "registry.example/ton:build-42"
    platform: "linux/arm64"
    image: "registry.example/ton:build-42"
    platform: "linux/arm64"
    image: "registry.example/ton:build-42"
    platform: "linux/arm64"
      - "127.0.0.1:18181:8081"
    image: "registry.example/ton:build-42"
    platform: "linux/arm64""#]]
        .assert_eq(&actual);
    }

    #[test]
    fn compose_commands_use_the_environment_docker_config() {
        let driver = FullTonNetworkDriver {
            compose_file: PathBuf::from("/workspace/.studio/environment-1/compose.yaml"),
            docker_config_dir: PathBuf::from("/workspace/.studio/environment-1/docker-config"),
            project_name: "acton-studio-environment-1".to_owned(),
            startup_log_file: PathBuf::from("/workspace/.studio/environment-1/startup.log"),
        };
        let command = driver.command();
        let actual = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");

        expect![[r"--config
/workspace/.studio/environment-1/docker-config
compose
-p
acton-studio-environment-1
-f
/workspace/.studio/environment-1/compose.yaml"]]
        .assert_eq(&actual);
    }
}
