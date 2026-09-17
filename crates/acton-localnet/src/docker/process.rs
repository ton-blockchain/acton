//! Container execution and explicit lifecycle through the Docker Engine API.

#[cfg(test)]
mod tests;

use super::{
    DockerNetwork, LOCALTON_SNAPSHOT_DIR, LOCALTON_STATE_DIR, NETWORK_DELETE_TIMEOUT,
    NETWORK_STOP_TIMEOUT, prerequisites::api_error,
};
use crate::Error;
use bollard::{
    container::LogOutput,
    exec::StartExecResults,
    models::{ContainerCreateBody, ExecConfig, HostConfig},
    query_parameters::{
        AttachContainerOptions, ListVolumesOptions, RemoveContainerOptions, RemoveVolumeOptions,
    },
};
use futures::{Stream, TryStreamExt};
use std::{pin::Pin, time::Duration};
use tokio::io::{AsyncWrite, AsyncWriteExt};

#[derive(Default)]
pub(super) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Drains both channels while feeding stdin, so large requests cannot deadlock
/// against a tool that writes diagnostics before consuming its input.
async fn exchange(
    mut output: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
    mut input: Pin<Box<dyn AsyncWrite + Send>>,
    bytes: Option<&[u8]>,
) -> Result<(Output, std::io::Result<()>), Error> {
    let send = async {
        if let Some(bytes) = bytes {
            input.write_all(bytes).await?;
        }
        let closed = input.shutdown().await;
        if bytes.is_some() { closed } else { Ok(()) }
    };
    let receive = async {
        let mut result = Output::default();
        while let Some(frame) = output.try_next().await.map_err(api_error)? {
            let buffer = match &frame {
                LogOutput::StdErr { .. } => &mut result.stderr,
                _ => &mut result.stdout,
            };
            if buffer.len() + frame.as_ref().len() > 64 * 1024 * 1024 {
                return Err(Error::invalid("Localton tool output exceeded 64 MiB"));
            }
            buffer.extend_from_slice(frame.as_ref());
        }
        Ok(result)
    };
    let (sent, received) = tokio::join!(send, receive);
    Ok((received?, sent))
}

fn completed(
    output: Output,
    sent: std::io::Result<()>,
    exit: Option<i64>,
) -> Result<Output, Error> {
    // Prefer the tool's diagnostic over a broken pipe from an early exit.
    if exit != Some(0) {
        return Err(Error::Internal {
            code: "localton_command_failed",
            message: format!(
                "Localton tool exited with {exit:?}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    sent.map_err(|error| Error::Internal {
        code: "localton_stdin_failed",
        message: format!("Failed to send Localton tool input: {error}"),
    })?;
    Ok(output)
}

impl DockerNetwork {
    /// Executes inside an existing service. Dropping an exec connection does not
    /// kill its process; mutation callers must allow this bounded operation to finish.
    pub(super) fn exec<'a>(
        &'a self,
        service: &'a str,
        args: &'a [&'a str],
        input: Option<&'a [u8]>,
        duration: Duration,
    ) -> futures::future::BoxFuture<'a, Result<Output, Error>> {
        Box::pin(async move {
            self.operation(
                "exec_localton",
                "localton_command_failed",
                duration,
                async {
                    let client = self.client().await?;
                    let name = self.container_name(service);
                    let container = client
                        .inspect_container(&name, None)
                        .await
                        .map_err(api_error)?;
                    self.owned(container.config.as_ref().and_then(|c| c.labels.as_ref()))?;
                    let created = client
                        .create_exec(
                            &name,
                            ExecConfig {
                                attach_stdin: Some(input.is_some()),
                                attach_stdout: Some(true),
                                attach_stderr: Some(true),
                                cmd: Some(args.iter().map(|s| (*s).to_owned()).collect()),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(api_error)?;
                    let StartExecResults::Attached {
                        output,
                        input: stdin,
                    } = client
                        .start_exec(&created.id, None)
                        .await
                        .map_err(api_error)?
                    else {
                        return Err(Error::invalid("Docker did not attach to the Localton tool"));
                    };
                    let (output, sent) = exchange(output, stdin, input).await?;
                    loop {
                        let state = client.inspect_exec(&created.id).await.map_err(api_error)?;
                        if state.running != Some(true) {
                            return completed(output, sent, state.exit_code);
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                },
            )
            .await
        })
    }

    /// Offline tools mount only the selected node and snapshot volumes with no
    /// network. Explicit cleanup runs after success, failure, or operation timeout.
    pub(super) fn offline<'a>(
        &'a self,
        service: &'a str,
        args: &'a [&'a str],
        input: Option<&'a [u8]>,
        duration: Duration,
    ) -> futures::future::BoxFuture<'a, Result<Output, Error>> {
        Box::pin(async move {
            self.ensure_volume(&format!("{service}-state")).await?;
            self.ensure_volume("localton-snapshots").await?;
            let client = self.client().await?;
            let created = client
                .create_container(
                    None,
                    ContainerCreateBody {
                        image: Some(self.image.clone()),
                        entrypoint: Some(vec!["/usr/local/bin/localton".into()]),
                        cmd: Some(args.iter().map(|s| (*s).to_owned()).collect()),
                        labels: Some(self.labels(None)),
                        attach_stdout: Some(true),
                        attach_stderr: Some(true),
                        attach_stdin: Some(input.is_some()),
                        open_stdin: Some(input.is_some()),
                        stdin_once: Some(true),
                        host_config: Some(HostConfig {
                            network_mode: Some("none".into()),
                            binds: Some(vec![
                                format!(
                                    "{}_{}-state:{LOCALTON_STATE_DIR}",
                                    self.project_name, service
                                ),
                                format!(
                                    "{}_localton-snapshots:{LOCALTON_SNAPSHOT_DIR}",
                                    self.project_name
                                ),
                            ]),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .await
                .map_err(api_error)?;
            let result = self
                .operation(
                    "offline_localton",
                    "localton_command_failed",
                    duration,
                    async {
                        let attached = client
                            .attach_container(
                                &created.id,
                                Some(AttachContainerOptions {
                                    stream: true,
                                    stdout: true,
                                    stderr: true,
                                    stdin: input.is_some(),
                                    ..Default::default()
                                }),
                            )
                            .await
                            .map_err(api_error)?;
                        client
                            .start_container(&created.id, None)
                            .await
                            .map_err(api_error)?;
                        let (output, sent) =
                            exchange(attached.output, attached.input, input).await?;
                        let exit = match client.wait_container(&created.id, None).try_next().await {
                            Ok(Some(wait)) => wait.status_code,
                            Err(bollard::errors::Error::DockerContainerWaitError {
                                code, ..
                            }) => code,
                            Err(error) => return Err(api_error(error)),
                            Ok(None) => {
                                return Err(Error::invalid(
                                    "Docker returned no offline tool exit status",
                                ));
                            }
                        };
                        completed(output, sent, Some(exit))
                    },
                )
                .await;
            let cleanup = tokio::time::timeout(
                Duration::from_secs(60),
                client.remove_container(
                    &created.id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                ),
            )
            .await;
            match cleanup {
                Ok(Ok(())) => result,
                cleanup => Err(Error::Internal {
                    code: "localton_cleanup_failed",
                    message: format!(
                        "Offline container {} cleanup failed: {cleanup:?}; operation: {}",
                        created.id,
                        result
                            .as_ref()
                            .err()
                            .map_or_else(|| "completed".into(), ToString::to_string)
                    ),
                }),
            }
        })
    }

    pub(crate) fn stop(&self) -> futures::future::BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.operation(
                "stop_network",
                "environment_stop_failed",
                NETWORK_STOP_TIMEOUT,
                async {
                    let containers = self.containers().await?;
                    let services: Vec<_> = containers
                        .iter()
                        .filter_map(|c| c.labels.as_ref()?.get(super::engine::SERVICE_LABEL))
                        .filter(|s| s.as_str() != "localton")
                        .collect();
                    let results =
                        futures::future::join_all(services.iter().map(|s| self.stop_service(s)))
                            .await;
                    let owner = self.stop_service("localton").await;
                    // Attempt every stop even when one service fails.
                    for result in results {
                        result?;
                    }
                    owner
                },
            )
            .await
        })
    }

    pub(crate) fn delete(&self) -> futures::future::BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.operation(
                "delete_network",
                "environment_delete_failed",
                NETWORK_DELETE_TIMEOUT,
                async {
                    self.down().await?;
                    let client = self.client().await?;
                    let volumes = client
                        .list_volumes(Some(ListVolumesOptions {
                            filters: Some(self.filters()),
                        }))
                        .await
                        .map_err(api_error)?;
                    for volume in volumes.volumes.unwrap_or_default() {
                        self.owned(Some(&volume.labels))?;
                        client
                            .remove_volume(&volume.name, None::<RemoveVolumeOptions>)
                            .await
                            .map_err(api_error)?;
                    }
                    Ok(())
                },
            )
            .await
        })
    }
}
