//! Pull progress uses Docker's layer events without assuming a fixed image size.

use super::DockerNetwork;
use crate::OperationProgress;
use std::{collections::BTreeMap, io::SeekFrom};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

impl DockerNetwork {
    pub(crate) async fn pull_progress(&self) -> Option<OperationProgress> {
        let mut file = tokio::fs::File::open(&self.startup_log_file).await.ok()?;
        let size = file.metadata().await.ok()?.len();

        // Docker's non-TTY output reports layer transitions, not byte updates.
        // Keep polling bounded even if the daemon emits excessive diagnostics.
        file.seek(SeekFrom::Start(size.saturating_sub(256 * 1024)))
            .await
            .ok()?;
        let mut bytes = Vec::new();
        file.take(256 * 1024).read_to_end(&mut bytes).await.ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let mut layers = BTreeMap::new();
        let mut detail = String::new();

        for line in text.lines() {
            let Some((id, state)) = line.trim().split_once(": ") else {
                continue;
            };

            if id.len() != 12 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }

            let complete = matches!(state, "Pull complete" | "Already exists");
            layers.insert(id, complete);
            detail = format!("{id}: {state}");
        }

        if layers.is_empty() {
            return None;
        }

        Some(OperationProgress {
            completed: layers.values().filter(|complete| **complete).count() as u64,
            // More layers can still be announced, so this is a count, not a bar.
            total: None,
            unit: "layers ready".to_owned(),
            detail,
        })
    }
}
