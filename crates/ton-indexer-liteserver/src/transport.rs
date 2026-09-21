use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use futures::future::BoxFuture;
use service_pool::{Endpoint, Failure};
use tonutils::{
    liteclient::{client::LiteClient, types::LiteError},
    network_config::ConfigLiteServer,
};

use super::{LiteRequestStats, TonutilsLiteClient};

#[derive(Default)]
pub(super) struct Counters {
    pub(super) head: AtomicU64,
    pub(super) lookup: AtomicU64,
    pub(super) block: AtomicU64,
}

impl Counters {
    pub(super) fn snapshot(&self) -> LiteRequestStats {
        LiteRequestStats {
            get_masterchain_info: self.head.load(Ordering::Relaxed),
            lookup_block: self.lookup.load(Ordering::Relaxed),
            get_block: self.block.load(Ordering::Relaxed),
        }
    }
}

/// Connections are checked out for an entire attempt. Dropping an attempt closes
/// its connection, so a late response cannot be reused by another operation.
pub(super) struct Server {
    id: String,
    settings: ConfigLiteServer,
    idle: Mutex<Vec<LiteClient>>,
    pub(super) counters: Arc<Counters>,
}

impl Server {
    pub(super) fn new(settings: ConfigLiteServer, counters: Arc<Counters>) -> Self {
        Self {
            id: hex::encode(settings.public_key()),
            settings,
            idle: Mutex::new(Vec::new()),
            counters,
        }
    }

    pub(super) async fn request<T>(
        &self,
        call: impl for<'a> FnOnce(&'a mut LiteClient) -> BoxFuture<'a, Result<T, Failure>>,
    ) -> Result<T, Failure> {
        let cached = self
            .idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop();
        let mut client = match cached {
            Some(client) => client,
            None => LiteClient::connect_with_timeout(
                self.settings.socket_addr(),
                self.settings.public_key(),
                TonutilsLiteClient::CONNECT_TIMEOUT,
            )
            .await
            .map_err(classify)?
            .with_request_timeout(TonutilsLiteClient::REQUEST_TIMEOUT),
        };

        let result = call(&mut client).await;
        if result.is_ok() || matches!(result, Err(Failure::Unavailable(_))) {
            self.idle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(client);
        }
        result
    }
}

impl Endpoint for Server {
    fn id(&self) -> &str {
        &self.id
    }

    fn address(&self) -> String {
        self.settings.socket_addr().to_string()
    }
}

pub(super) fn classify(error: LiteError) -> Failure {
    match error {
        // TON's ErrorCode::notready means this server cannot supply the requested data.
        LiteError::ServerError(error) if error.code == 651 => {
            Failure::Unavailable(error.to_string())
        }
        LiteError::TlError(_) | LiteError::UnexpectedMessage => Failure::Invalid(error.to_string()),
        error => Failure::Retryable(error.to_string()),
    }
}
