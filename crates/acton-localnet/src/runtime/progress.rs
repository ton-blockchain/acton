//! Progress is measured by the service so HTTP and CLI observe the same work.

use super::Context;
use crate::{Error, OperationProgress, docker::DockerNetwork};
use std::time::Duration;

impl Context {
    pub(super) async fn progress(&mut self, progress: OperationProgress) -> Result<(), Error> {
        if self.operation.progress.as_ref() != Some(&progress) {
            self.operation.progress = Some(progress);
            self.publish().await?;
        }

        Ok(())
    }

    /// Observes long Docker work without cancelling it when a client disconnects.
    /// Sampling failures do not fail the operation; its own process result remains
    /// authoritative. Startup cancellation is owned by `wait_child`.
    pub(super) async fn observe<T>(
        &mut self,
        driver: &DockerNetwork,
        work: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        tokio::pin!(work);
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let nodes = self.entry.record.read().await.nodes.clone();

        // Keep polling in its own future so a slow Docker metadata request cannot
        // delay process completion or a graceful startup cancellation.
        let updates = async {
            loop {
                interval.tick().await;
                let progress = match self.operation.phase.as_str() {
                    "pullingImage" => driver.pull_progress().await,
                    "startingContainers" | "joiningNode" => {
                        driver.container_progress(&nodes, false).await
                    }
                    "stopping" => driver.container_progress(&nodes, true).await,
                    _ => None,
                };

                if let Some(progress) = progress {
                    self.progress(progress).await?;
                }
            }
        };

        tokio::select! {
            result = &mut work => result,
            result = updates => result,
        }
    }
}
