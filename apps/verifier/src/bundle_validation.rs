use thiserror::Error;

use crate::{
    blockchain::{is_valid_hash, normalize_hash},
    source_bundle::{
        SourceBundleCompiler, SourceBundleError, SourceBundleFile, SourceBundleInput,
        SourceBundleSource, compute_source_bundle_hash,
    },
    source_storage::StoredSourceBundle,
};

/// Validates that a stored source bundle belongs to `code_hash` and has a
/// canonical bundle hash matching its manifest.
///
/// An optional payment transaction hash must use canonical lowercase
/// hexadecimal form.
///
/// # Errors
///
/// Returns an error when the manifest code hash differs from `code_hash`, the
/// canonical source bundle hash does not match the manifest, the payment
/// transaction hash is not canonical, or the canonical hash input cannot be
/// serialized.
pub fn validate_stored_bundle(
    bundle: &StoredSourceBundle,
    code_hash: &str,
) -> Result<(), StoredBundleValidationError> {
    let manifest = &bundle.manifest;
    if manifest.code_hash != code_hash {
        return Err(StoredBundleValidationError::CodeHashMismatch {
            code_hash: code_hash.to_owned(),
            source_bundle_hash: manifest.source_bundle_hash.clone(),
        });
    }

    if let Some(payment_tx_hash) = &manifest.payment_tx_hash
        && (!is_valid_hash(payment_tx_hash) || normalize_hash(payment_tx_hash) != *payment_tx_hash)
    {
        return Err(StoredBundleValidationError::InvalidPaymentTransactionHash(
            payment_tx_hash.clone(),
        ));
    }

    let computed_hash = compute_source_bundle_hash(SourceBundleInput {
        compiler: SourceBundleCompiler {
            language: &manifest.compiler.language,
            version: &manifest.compiler.version,
            entrypoint: &manifest.compiler.entrypoint,
            params: &manifest.compiler.params,
        },
        sources: bundle
            .files
            .iter()
            .map(|file| SourceBundleSource {
                path: &file.path,
                include_in_command: file.include_in_command,
                is_stdlib: file.is_stdlib,
                has_include_directives: file.has_include_directives,
            })
            .collect(),
        files: bundle
            .files
            .iter()
            .map(|file| SourceBundleFile {
                path: &file.path,
                bytes: file.content.as_bytes(),
            })
            .collect(),
    })?;

    if computed_hash != manifest.source_bundle_hash {
        return Err(StoredBundleValidationError::SourceBundleHashMismatch {
            source_bundle_hash: manifest.source_bundle_hash.clone(),
            computed_hash,
        });
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum StoredBundleValidationError {
    #[error("stored bundle {source_bundle_hash} does not belong to code hash {code_hash}")]
    CodeHashMismatch {
        code_hash: String,
        source_bundle_hash: String,
    },
    #[error(
        "stored bundle hash mismatch for {source_bundle_hash}: canonical hash is {computed_hash}"
    )]
    SourceBundleHashMismatch {
        source_bundle_hash: String,
        computed_hash: String,
    },
    #[error("stored bundle has invalid payment transaction hash: {0}")]
    InvalidPaymentTransactionHash(String),
    #[error(transparent)]
    SourceBundle(#[from] SourceBundleError),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        source_bundle::{SourceBundleCompiler, SourceBundleInput, compute_source_bundle_hash},
        source_storage::{CompilerMetadata, SourceBundleManifest},
    };

    const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PAYMENT_TX_HASH: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn accepts_canonical_payment_transaction_hash() {
        let bundle = stored_bundle(Some(PAYMENT_TX_HASH));

        assert!(validate_stored_bundle(&bundle, CODE_HASH).is_ok());
    }

    #[test]
    fn rejects_non_canonical_payment_transaction_hashes() {
        for payment_tx_hash in [
            PAYMENT_TX_HASH.to_ascii_uppercase(),
            "ASNFZ4mrze8BI0VniavN7wEjRWeJq83vASNFZ4mrze8=".to_owned(),
            "0123456789abcdef".to_owned(),
        ] {
            let bundle = stored_bundle(Some(&payment_tx_hash));
            let error = validate_stored_bundle(&bundle, CODE_HASH)
                .expect_err("non-canonical payment transaction hash should be rejected");

            assert!(matches!(
                error,
                StoredBundleValidationError::InvalidPaymentTransactionHash(value)
                    if value == payment_tx_hash
            ));
        }
    }

    fn stored_bundle(payment_tx_hash: Option<&str>) -> StoredSourceBundle {
        let compiler = CompilerMetadata {
            language: "tolk".to_owned(),
            version: "1.4.2".to_owned(),
            entrypoint: "main.tolk".to_owned(),
            params: json!({"compiler_version": "1.4.2"}),
        };
        let source_bundle_hash = compute_source_bundle_hash(SourceBundleInput {
            compiler: SourceBundleCompiler {
                language: &compiler.language,
                version: &compiler.version,
                entrypoint: &compiler.entrypoint,
                params: &compiler.params,
            },
            sources: Vec::new(),
            files: Vec::new(),
        })
        .expect("empty source bundle should be serializable");

        StoredSourceBundle {
            storage_revision: "revision".to_owned(),
            manifest: SourceBundleManifest {
                code_hash: CODE_HASH.to_owned(),
                source_bundle_hash,
                payment_tx_hash: payment_tx_hash.map(ToOwned::to_owned),
                verified_at: 1_600_000_000,
                compiler,
                source_map: None,
            },
            files: Vec::new(),
        }
    }
}
