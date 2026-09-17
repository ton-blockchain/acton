//! Pull progress uses Docker's layer events without assuming a fixed image size.

use super::DockerNetwork;
use crate::OperationProgress;
impl DockerNetwork {
    pub(crate) async fn pull_progress(&self) -> Option<OperationProgress> {
        self.pull_progress.read().await.clone()
    }
}
