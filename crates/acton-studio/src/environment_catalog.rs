use std::sync::Arc;

use toncenter_keys::{TONCENTER_MAINNET_API_KEY_ENV, TONCENTER_TESTNET_API_KEY_ENV};

use crate::environment::{
    CreateEnvironmentRequest, CreateEnvironmentSnapshotRequest, CreateFullTonNodeRequest,
    EnvironmentConfig, EnvironmentEndpoints, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentSnapshot, EnvironmentSnapshotOperation, EnvironmentStatus,
    PublicTonNetwork, RemoveFullTonNodeRequest, StudioEnvironment, UpdateEnvironmentRequest,
};

pub const TESTNET_ENVIRONMENT_ID: &str = "testnet";
pub const MAINNET_ENVIRONMENT_ID: &str = "mainnet";
pub const PUBLIC_TON_ENVIRONMENT_IDS: [&str; 2] = [TESTNET_ENVIRONMENT_ID, MAINNET_ENVIRONMENT_ID];

#[derive(Clone, Copy, Debug)]
pub(crate) struct PublicTonNetworkDescriptor {
    pub(crate) network: PublicTonNetwork,
    pub(crate) environment_id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) api_v2_endpoint: &'static str,
    pub(crate) api_v3_endpoint: &'static str,
    pub(crate) api_key_environment_variable: &'static str,
}

pub(crate) const PUBLIC_TON_NETWORKS: [PublicTonNetworkDescriptor; 2] = [
    PublicTonNetworkDescriptor {
        network: PublicTonNetwork::Testnet,
        environment_id: TESTNET_ENVIRONMENT_ID,
        display_name: "Testnet",
        api_v2_endpoint: "https://testnet.toncenter.com/api/v2",
        api_v3_endpoint: "https://testnet.toncenter.com/api/v3",
        api_key_environment_variable: TONCENTER_TESTNET_API_KEY_ENV,
    },
    PublicTonNetworkDescriptor {
        network: PublicTonNetwork::Mainnet,
        environment_id: MAINNET_ENVIRONMENT_ID,
        display_name: "Mainnet",
        api_v2_endpoint: "https://toncenter.com/api/v2",
        api_v3_endpoint: "https://toncenter.com/api/v3",
        api_key_environment_variable: TONCENTER_MAINNET_API_KEY_ENV,
    },
];

fn public_ton_network_descriptor_by_id(
    environment_id: &str,
) -> Option<&'static PublicTonNetworkDescriptor> {
    PUBLIC_TON_NETWORKS
        .iter()
        .find(|descriptor| descriptor.environment_id == environment_id)
}

pub(crate) struct EnvironmentCatalogRuntime {
    managed: Arc<dyn EnvironmentRuntime>,
}

impl EnvironmentCatalogRuntime {
    pub(crate) fn new(managed: Arc<dyn EnvironmentRuntime>) -> Self {
        Self { managed }
    }
}

impl EnvironmentRuntime for EnvironmentCatalogRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        Box::pin(async move {
            let mut environments = self.managed.list().await?;
            environments.retain(|environment| {
                public_ton_network_descriptor_by_id(&environment.id).is_none()
            });
            let mut catalog = PUBLIC_TON_NETWORKS
                .iter()
                .map(public_environment)
                .collect::<Vec<_>>();
            catalog.append(&mut environments);
            Ok(catalog)
        })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(descriptor) = public_ton_network_descriptor_by_id(environment_id) {
            let environment = public_environment(descriptor);
            return Box::pin(async move { Ok(environment) });
        }
        self.managed.get(environment_id)
    }

    fn create(
        &self,
        request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        self.managed.create(request)
    }

    fn update(
        &self,
        environment_id: &str,
        request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "updated") {
            return error;
        }
        self.managed.update(environment_id, request)
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        if let Some(error) = lifecycle_unavailable(environment_id, "deleted") {
            return error;
        }
        self.managed.delete(environment_id)
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "stopped") {
            return error;
        }
        self.managed.stop(environment_id)
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "restarted") {
            return error;
        }
        self.managed.restart(environment_id)
    }

    fn add_full_ton_node(
        &self,
        environment_id: &str,
        request: CreateFullTonNodeRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "expanded") {
            return error;
        }
        self.managed.add_full_ton_node(environment_id, request)
    }

    fn remove_full_ton_node(
        &self,
        environment_id: &str,
        node_id: &str,
        request: RemoveFullTonNodeRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "contracted") {
            return error;
        }
        self.managed
            .remove_full_ton_node(environment_id, node_id, request)
    }

    fn leave_full_ton_validation(
        &self,
        environment_id: &str,
        node_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "changed") {
            return error;
        }
        self.managed
            .leave_full_ton_validation(environment_id, node_id)
    }

    fn enter_full_ton_validation(
        &self,
        environment_id: &str,
        node_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if let Some(error) = lifecycle_unavailable(environment_id, "changed") {
            return error;
        }
        self.managed
            .enter_full_ton_validation(environment_id, node_id)
    }

    fn start_admin(
        &self,
        id: &str,
        request: crate::AdminRequest,
    ) -> EnvironmentRuntimeFuture<'_, crate::AdminOperation> {
        self.managed.start_admin(id, request)
    }
    fn admin_operation(
        &self,
        id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Option<crate::AdminOperation>> {
        self.managed.admin_operation(id)
    }

    fn list_snapshots(
        &self,
        environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Vec<EnvironmentSnapshot>> {
        if let Some(error) = lifecycle_unavailable(environment_id, "snapshotted") {
            return error;
        }
        self.managed.list_snapshots(environment_id)
    }

    fn create_snapshot(
        &self,
        environment_id: &str,
        request: CreateEnvironmentSnapshotRequest,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        if let Some(error) = lifecycle_unavailable(environment_id, "snapshotted") {
            return error;
        }
        self.managed.create_snapshot(environment_id, request)
    }

    fn restore_snapshot(
        &self,
        environment_id: &str,
        snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        if let Some(error) = lifecycle_unavailable(environment_id, "restored") {
            return error;
        }
        self.managed.restore_snapshot(environment_id, snapshot_id)
    }

    fn delete_snapshot(
        &self,
        environment_id: &str,
        snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, ()> {
        if let Some(error) = lifecycle_unavailable(environment_id, "changed") {
            return error;
        }
        self.managed.delete_snapshot(environment_id, snapshot_id)
    }

    fn snapshot_operation(
        &self,
        environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Option<EnvironmentSnapshotOperation>> {
        if let Some(error) = lifecycle_unavailable(environment_id, "snapshotted") {
            return error;
        }
        self.managed.snapshot_operation(environment_id)
    }

    fn shutdown(&self) -> EnvironmentRuntimeFuture<'_, ()> {
        self.managed.shutdown()
    }
}

fn public_environment(descriptor: &PublicTonNetworkDescriptor) -> StudioEnvironment {
    StudioEnvironment::new_external(
        descriptor.environment_id,
        descriptor.display_name,
        EnvironmentStatus::Running,
        EnvironmentConfig::RemoteTonNetwork {
            network: descriptor.network,
        },
        EnvironmentEndpoints {
            api_v2: Some(descriptor.api_v2_endpoint.to_owned()),
            api_v3: Some(descriptor.api_v3_endpoint.to_owned()),
            config: None,
            control: None,
            observability: None,
        },
    )
}

fn lifecycle_unavailable<T>(
    environment_id: &str,
    action: &'static str,
) -> Option<EnvironmentRuntimeFuture<'static, T>> {
    let descriptor = public_ton_network_descriptor_by_id(environment_id)?;
    let display_name = descriptor.display_name;
    Some(Box::pin(async move {
        Err(EnvironmentRuntimeError::Conflict {
            code: "environment_lifecycle_unavailable",
            message: format!(
                "{display_name} is an external environment and cannot be {action} by Studio"
            ),
        })
    }))
}
