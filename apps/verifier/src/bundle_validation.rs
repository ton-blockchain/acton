use thiserror::Error;

use crate::{
    source_bundle::{
        SourceBundleCompiler, SourceBundleError, SourceBundleFile, SourceBundleInput,
        SourceBundleSource, compute_source_bundle_hash,
    },
    source_storage::StoredSourceBundle,
};

/// Validates that a stored source bundle belongs to `code_hash` and has a
/// canonical bundle hash matching its manifest.
///
/// # Errors
///
/// Returns an error when the manifest code hash differs from `code_hash`, the
/// canonical source bundle hash does not match the manifest, or the canonical
/// hash input cannot be serialized.
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
    #[error(transparent)]
    SourceBundle(#[from] SourceBundleError),
}
