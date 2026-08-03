use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::StudioEnvironment;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioWallet {
    pub name: String,
    pub address: String,
    pub public_key: String,
    pub version: String,
    pub wallet_id: i32,
    pub workchain: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignWalletRequest {
    pub bytes: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignWalletResponse {
    pub signature: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WalletRuntimeError {
    #[error("{message}")]
    InvalidRequest { code: &'static str, message: String },
    #[error("Wallet {wallet_name} was not found")]
    NotFound { wallet_name: String },
    #[error("{message}")]
    Internal { code: &'static str, message: String },
}

pub type WalletRuntimeFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, WalletRuntimeError>> + Send + 'a>>;

pub trait WalletRuntime: Send + Sync {
    fn list(&self, environment: &StudioEnvironment) -> WalletRuntimeFuture<'_, Vec<StudioWallet>>;

    fn sign(
        &self,
        environment: &StudioEnvironment,
        wallet_name: &str,
        bytes: Vec<u8>,
    ) -> WalletRuntimeFuture<'_, [u8; 64]>;
}

pub(crate) struct EmptyWalletRuntime;

impl WalletRuntime for EmptyWalletRuntime {
    fn list(&self, _environment: &StudioEnvironment) -> WalletRuntimeFuture<'_, Vec<StudioWallet>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn sign(
        &self,
        _environment: &StudioEnvironment,
        wallet_name: &str,
        _bytes: Vec<u8>,
    ) -> WalletRuntimeFuture<'_, [u8; 64]> {
        let wallet_name = wallet_name.to_owned();
        Box::pin(async move { Err(WalletRuntimeError::NotFound { wallet_name }) })
    }
}
