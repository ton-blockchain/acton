use std::sync::Arc;

use crate::environment::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints, EnvironmentRuntime,
    EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus, PublicTonNetwork,
    StudioEnvironment, UpdateEnvironmentRequest,
};

pub const TESTNET_ENVIRONMENT_ID: &str = "testnet";

const TESTNET_API_V2_ENDPOINT: &str = "https://testnet.toncenter.com/api/v2";
const TESTNET_API_V3_ENDPOINT: &str = "https://testnet.toncenter.com/api/v3";

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
            environments.retain(|environment| environment.id != TESTNET_ENVIRONMENT_ID);
            environments.insert(0, testnet_environment());
            Ok(environments)
        })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if environment_id == TESTNET_ENVIRONMENT_ID {
            return Box::pin(async { Ok(testnet_environment()) });
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
        if environment_id == TESTNET_ENVIRONMENT_ID {
            return lifecycle_unavailable("updated");
        }
        self.managed.update(environment_id, request)
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        if environment_id == TESTNET_ENVIRONMENT_ID {
            return lifecycle_unavailable("deleted");
        }
        self.managed.delete(environment_id)
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if environment_id == TESTNET_ENVIRONMENT_ID {
            return lifecycle_unavailable("stopped");
        }
        self.managed.stop(environment_id)
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        if environment_id == TESTNET_ENVIRONMENT_ID {
            return lifecycle_unavailable("restarted");
        }
        self.managed.restart(environment_id)
    }

    fn shutdown(&self) -> EnvironmentRuntimeFuture<'_, ()> {
        self.managed.shutdown()
    }
}

fn testnet_environment() -> StudioEnvironment {
    StudioEnvironment::new_external(
        TESTNET_ENVIRONMENT_ID,
        "Testnet",
        EnvironmentStatus::Running,
        EnvironmentConfig::RemoteTonNetwork {
            network: PublicTonNetwork::Testnet,
        },
        EnvironmentEndpoints {
            api_v2: Some(TESTNET_API_V2_ENDPOINT.to_owned()),
            api_v3: Some(TESTNET_API_V3_ENDPOINT.to_owned()),
            control: None,
        },
    )
}

fn lifecycle_unavailable<T>(action: &'static str) -> EnvironmentRuntimeFuture<'static, T> {
    Box::pin(async move {
        Err(EnvironmentRuntimeError::Conflict {
            code: "environment_lifecycle_unavailable",
            message: format!("Testnet is an external environment and cannot be {action} by Studio"),
        })
    })
}
