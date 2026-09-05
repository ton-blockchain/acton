//! Administrative operations are detached from HTTP request lifetimes.
use crate::Error;
use serde::{Deserialize, Serialize};
use ton_hardfork::request::{AccountEdit, decode_cell};

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AdminRequest {
    Accounts {
        id: String,
        #[schema(value_type = Vec<serde_json::Value>)]
        edits: Vec<AccountEdit>,
    },
    Config {
        id: String,
        index: i32,
        boc: String,
    },
}
impl AdminRequest {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Accounts { id, .. } | Self::Config { id, .. } => id,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        let fail = |e: String| Error::Conflict {
            code: "invalid_admin_request",
            message: e,
        };
        uuid::Uuid::parse_str(self.id()).map_err(|e| fail(e.to_string()))?;
        match self {
            Self::Accounts { edits, .. } => {
                if edits.is_empty() || edits.len() > 100 {
                    return Err(fail("An operation must contain 1–100 edits".into()));
                }
                let mut seen = std::collections::BTreeSet::new();
                for edit in edits {
                    let address = edit.validate().map_err(|e| fail(e.to_string()))?;
                    if !seen.insert(address.to_string()) {
                        return Err(fail(format!("Duplicate account: {address}")));
                    }
                }
            }
            Self::Config { boc, .. } => {
                decode_cell(boc).map_err(|e| fail(e.to_string()))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminOperation {
    pub id: String,
    pub phase: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub block_seqno: Option<u32>,
}
impl AdminOperation {
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.finished_at.is_none()
    }
}

pub(crate) async fn phase(operation: &tokio::sync::RwLock<Option<AdminOperation>>, phase: &str) {
    if let Some(op) = operation.write().await.as_mut() {
        phase.clone_into(&mut op.phase);
    }
}
