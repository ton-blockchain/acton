//! Administrative operations are detached from HTTP request lifetimes.

use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tokio::sync::RwLock;
use ton_hardfork::request::AccountEdit;
use uuid::Uuid;

/// A durable mutation whose ID identifies both its payload and retry history.
/// Reusing the ID with different edits is rejected, even after completion.
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AdminRequest {
    Accounts {
        id: String,
        #[schema(value_type = Vec<serde_json::Value>)]
        edits: Vec<AccountEdit>,
    },
}

impl AdminRequest {
    pub(crate) fn check_retry(&self, previous: &Self) -> Result<(), Error> {
        let value = |request: &Self| {
            serde_json::to_value(request).map_err(|error| Error::invalid(error.to_string()))
        };

        if value(self)? != value(previous)? {
            return Err(Error::Conflict {
                code: "admin_id_reused",
                message: "This operation id belongs to a different request".into(),
            });
        }

        Ok(())
    }

    /// Returns the client-generated key used to reconcile retries after a lost response.
    #[must_use]
    pub fn id(&self) -> &str {
        let Self::Accounts { id, .. } = self;
        id
    }

    /// Rejects invalid transport payloads before acquiring the network mutation lock.
    /// State-dependent constraints are checked later against the paused chain.
    pub fn validate(&self) -> Result<(), Error> {
        let fail = |e: String| Error::Conflict {
            code: "invalid_admin_request",
            message: e,
        };

        Uuid::parse_str(self.id()).map_err(|e| fail(e.to_string()))?;

        let Self::Accounts { edits, .. } = self;
        if edits.is_empty() || edits.len() > 100 {
            return Err(fail("An operation must contain 1–100 edits".into()));
        }

        let mut seen = BTreeSet::new();
        for edit in edits {
            let address = edit.validate().map_err(|e| fail(e.to_string()))?;

            if !seen.insert(address.to_string()) {
                return Err(fail(format!("Duplicate account: {address}")));
            }
        }

        Ok(())
    }
}

/// Service-owned progress that remains observable when the submitting client disconnects.
/// A terminal record is retained for retries, not automatically resubmitted on restart.
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
    /// Includes rollback in the active lifetime so callers cannot mutate during recovery.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.finished_at.is_none()
    }
}

pub(crate) async fn phase(operation: &RwLock<Option<AdminOperation>>, phase: &str) {
    if let Some(op) = operation.write().await.as_mut() {
        phase.clone_into(&mut op.phase);
    }
}
