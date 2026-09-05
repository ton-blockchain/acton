//! Snapshots support for the localnet Docker runtime.

use super::{
    COMPOSE_DELETE_TIMEOUT, DockerNetwork, LOCALTON_SNAPSHOT_DIR, LOCALTON_STATE_DIR,
    SNAPSHOT_TIMEOUT,
};
use crate::{Error, Snapshot};
use std::ffi::OsStr;

impl DockerNetwork {
    pub(crate) async fn list_snapshots(&self) -> Result<Vec<Snapshot>, Error> {
        self.snapshot_json(["list"]).await
    }

    pub(crate) async fn create_snapshot(&self, name: Option<&str>) -> Result<Snapshot, Error> {
        let mut args = vec!["create"];
        if let Some(name) = name {
            args.extend(["--name", name]);
        }

        self.snapshot_json(args).await
    }

    pub(crate) async fn restore_snapshot(&self, snapshot_id: &str) -> Result<Snapshot, Error> {
        self.snapshot_json(["restore", snapshot_id]).await
    }

    pub(crate) async fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), Error> {
        let _: serde_json::Value = self.snapshot_json(["delete", snapshot_id]).await?;
        Ok(())
    }

    pub(crate) async fn reset_indexer(&self) -> Result<(), Error> {
        self.run_compose(
            ["down", "--remove-orphans"],
            "prepare to rebuild the index",
            "environment_snapshot_restore_failed",
            COMPOSE_DELETE_TIMEOUT,
        )
        .await?;
        for volume in ["postgres-data", "ton-index-workdir"] {
            let mut command = self.docker_command();
            command
                .arg("volume")
                .arg("rm")
                .arg("--force")
                .arg(format!("{}_{volume}", self.project_name));
            self.run_command(
                command,
                "remove derived index data",
                "environment_snapshot_restore_failed",
                COMPOSE_DELETE_TIMEOUT,
            )
            .await?;
        }

        Ok(())
    }

    async fn snapshot_json<T, I, S>(&self, args: I) -> Result<T, Error>
    where
        T: serde::de::DeserializeOwned,
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.compose_command();
        command
            .arg("run")
            .arg("--rm")
            .arg("--no-deps")
            .arg("localton")
            .arg("snapshot")
            .args(args)
            .arg("--state-dir")
            .arg(LOCALTON_STATE_DIR)
            .arg("--snapshot-dir")
            .arg(LOCALTON_SNAPSHOT_DIR);
        let output = self
            .command_output(
                command,
                "manage snapshots",
                "environment_snapshot_failed",
                SNAPSHOT_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&output.stdout).map_err(|error| Error::Internal {
            code: "environment_snapshot_failed",
            message: format!("localton returned invalid snapshot data: {error}"),
        })
    }
}
