//! Studio adapter for the Tolk compiler and the new Acton verifier protocol.
//! Previews retain reviewed bytes; neither preparation nor status checks access a wallet.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acton_studio::{
    EnvironmentRuntimeError, PublicTonNetwork, StartVerificationRequest, VerificationCandidate,
    VerificationFile, VerificationFuture, VerificationMessage, VerificationOperation,
    VerificationPayment, VerificationPaymentRequest, VerificationPhase, VerificationPreview,
    VerificationRuntime, VerificationStatus,
};
use serde::Deserialize;

use super::{
    ActonConfig, Boc, Context, FromStr, HashBytes, NewVerifierPaymentQuote, Path, TonAddress,
    UploadPart, anyhow, build_verify_http_client, collect_verification_sources,
    compile_verification_contract, configured_project_root, format_ton_address,
    new_verifier_backend, new_verifier_compile_params, new_verifier_sources,
    normalize_source_path_for_verifier, source_files_from_source_map, take_new_verifier_ticket,
    truncate_for_display, upload_new_verifier_sources, wait_for_new_verifier_payment,
};

const PREVIEW_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_PREVIEWS: usize = 16;
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Serializes native compilation and bounds in-memory source snapshots for one Studio project.
#[derive(Default)]
pub(crate) struct ProjectVerificationRuntime {
    previews: Arc<Mutex<BTreeMap<String, StoredPreview>>>,
    compilation: Arc<tokio::sync::Mutex<()>>,
}

struct StoredPreview {
    created: Instant,
    network: PublicTonNetwork,
    preview: VerificationPreview,
    sources: BTreeMap<String, SourceSnapshot>,
    submission: Option<(String, String)>,
    operation: Option<VerificationOperation>,
    quote: Option<NewVerifierPaymentQuote>,
    payment_message_hash: Option<String>,
    payment_transaction_hash: Option<String>,
}

#[derive(Clone)]
struct SourceSnapshot {
    code_hash: HashBytes,
    parts: Vec<UploadPart>,
    sources_json: String,
    compile_params_json: String,
}

impl VerificationRuntime for ProjectVerificationRuntime {
    fn status(
        &self,
        _network: PublicTonNetwork,
        code_hash: String,
    ) -> VerificationFuture<'_, VerificationStatus> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || public_status(&code_hash))
                .await
                .map_err(worker_error)?
                .map_err(verification_error)
        })
    }

    fn preview(
        &self,
        network: PublicTonNetwork,
        code_hash: String,
    ) -> VerificationFuture<'_, VerificationPreview> {
        let previews = Arc::clone(&self.previews);
        let compilation = Arc::clone(&self.compilation);

        Box::pin(async move {
            let guard = compilation.lock_owned().await;
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                let started = Instant::now();
                let config = ActonConfig::load()?;
                let status = public_status(&code_hash)?;
                let compiler_version = tolk_compiler::native_tolk_version()?.version;
                let mut candidates = Vec::new();
                let mut sources = BTreeMap::new();

                // An already published hash needs neither local compilation nor wallet access.
                if !status.verified {
                    for (id, contract) in config.contracts().into_iter().flatten() {
                        let path = contract.absolute_source_path(configured_project_root());
                        if path.extension() != Some("tolk".as_ref()) {
                            continue;
                        }

                        let mut candidate = VerificationCandidate {
                            contract_id: id.clone(),
                            source_path: normalize_source_path_for_verifier(
                                &path,
                                configured_project_root(),
                            ),
                            code_hash: None,
                            matches: false,
                            error: None,
                            files: Vec::new(),
                        };
                        match capture_sources(&config, &path) {
                            Ok(snapshot) => {
                                candidate.code_hash = Some(snapshot.code_hash.to_string());
                                candidate.matches =
                                    snapshot.code_hash.to_string() == status.code_hash;
                                candidate.files = snapshot
                                    .parts
                                    .iter()
                                    .map(|part| VerificationFile {
                                        path: part.field_name.clone(),
                                        size_bytes: part.bytes.len(),
                                    })
                                    .collect();
                                if candidate.matches {
                                    sources.insert(id.clone(), snapshot);
                                }
                            }
                            Err(error) => candidate.error = Some(format!("{error:#}")),
                        }
                        candidates.push(candidate);
                    }
                }

                let quote = if sources.is_empty() {
                    None
                } else {
                    take_new_verifier_ticket(&status.code_hash)?
                };
                let payment = quote.as_ref().map(payment_details).transpose()?;
                let status = VerificationStatus {
                    verified: status.verified || (!sources.is_empty() && quote.is_none()),
                    ..status
                };

                let preview = VerificationPreview {
                    id: format!("{:032x}", rand::random::<u128>()),
                    status,
                    compiler_version,
                    candidates,
                    payment,
                };
                let mut stored = previews
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // Keep paid attempts available for retries, including failures after payment.
                stored.retain(|_, item| item.created.elapsed() < PREVIEW_TTL || is_running(item));
                if stored.len() >= MAX_PREVIEWS {
                    let oldest = stored
                        .iter()
                        .filter(|(_, item)| !is_running(item))
                        .min_by_key(|(_, item)| item.created)
                        .map(|(id, _)| id.clone());
                    if let Some(oldest) = oldest {
                        stored.remove(&oldest);
                    } else {
                        anyhow::bail!(
                            "Too many verification operations are running; wait for one to finish"
                        );
                    }
                }
                stored.insert(
                    preview.id.clone(),
                    StoredPreview {
                        created: Instant::now(),
                        network,
                        preview: preview.clone(),
                        sources,
                        submission: None,
                        operation: None,
                        quote,
                        payment_message_hash: None,
                        payment_transaction_hash: None,
                    },
                );
                drop(stored);

                log::info!(
                    target: "acton::verification",
                    "operation=verification_preview target={} duration_ms={} outcome=complete",
                    code_hash,
                    started.elapsed().as_millis(),
                );
                Ok(preview)
            })
            .await
            .map_err(worker_error)?
            .map_err(verification_error)
        })
    }

    fn start(
        &self,
        network: PublicTonNetwork,
        request: StartVerificationRequest,
    ) -> VerificationFuture<'_, VerificationOperation> {
        let previews = Arc::clone(&self.previews);

        Box::pin(async move {
            let sender =
                TonAddress::from_str(&request.sender_address).map_err(verification_error)?;
            // The payment chain is Testnet for both target networks.
            let sender = format_ton_address(&sender, true);
            let mut stored = previews
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let item = stored
                .get_mut(&request.preview_id)
                .filter(|item| item.network == network)
                .ok_or_else(expired_preview)?;
            let submission = (request.contract_id.clone(), sender);

            if let Some(existing) = &item.operation {
                if item.submission.as_ref() != Some(&submission) {
                    return Err(EnvironmentRuntimeError::Conflict {
                        code: "verification_already_started",
                        message: "This preview was already submitted with another contract or wallet; check the sources again".to_owned(),
                    });
                }
                return Ok(existing.clone());
            }
            if item.created.elapsed() >= PREVIEW_TTL {
                return Err(expired_preview());
            }
            if !item.sources.contains_key(&request.contract_id) {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "verification_code_mismatch",
                    message: "Select a project contract whose compiled code matches the requested code hash".to_owned(),
                });
            }

            let quote = item.quote.as_ref().ok_or_else(expired_preview)?;
            let payment = payment_details(quote).map_err(verification_error)?;
            let mut body = tycho_types::cell::CellBuilder::new();
            body.store_u32(0).map_err(verification_error)?;
            body.store_raw(
                quote.comment.as_bytes(),
                u16::try_from(quote.comment.len() * 8).map_err(verification_error)?,
            )
            .map_err(verification_error)?;
            let payload = Boc::encode_base64(body.build().map_err(verification_error)?);
            let operation = VerificationOperation {
                id: request.preview_id,
                phase: VerificationPhase::Ready,
                message: Some(VerificationMessage {
                    address: payment.address,
                    amount: payment.amount,
                    payload,
                }),
                error: None,
            };
            item.submission = Some(submission);
            item.operation = Some(operation.clone());
            drop(stored);

            Ok(operation)
        })
    }

    fn complete_payment(
        &self,
        network: PublicTonNetwork,
        id: String,
        request: VerificationPaymentRequest,
    ) -> VerificationFuture<'_, VerificationOperation> {
        let previews = Arc::clone(&self.previews);

        Box::pin(async move {
            let message_hash = crate::transaction_hash::toncenter_transaction_hash_hex(
                request.message_hash.trim().trim_start_matches("0x"),
            )
            .map_err(|_| EnvironmentRuntimeError::InvalidRequest {
                code: "verification_invalid_payment_hash",
                message: "The wallet returned an invalid payment message hash".to_owned(),
            })?;
            let (snapshot, quote, transaction_hash, code_hash, operation) = {
                let mut stored = previews
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let item = stored
                    .get_mut(&id)
                    .filter(|item| item.network == network)
                    .ok_or_else(expired_preview)?;
                let existing = item.operation.as_ref().ok_or_else(expired_preview)?;

                if let Some(previous) = &item.payment_message_hash {
                    if previous != &message_hash {
                        return Err(EnvironmentRuntimeError::Conflict {
                            code: "verification_payment_changed",
                            message: "This operation already has a payment; retry it without sending another transaction".to_owned(),
                        });
                    }
                    if existing.phase != VerificationPhase::Failed {
                        return Ok(existing.clone());
                    }
                }

                let contract_id = &item.submission.as_ref().ok_or_else(expired_preview)?.0;
                let snapshot = item
                    .sources
                    .get(contract_id)
                    .cloned()
                    .ok_or_else(expired_preview)?;
                let quote = item.quote.clone().ok_or_else(expired_preview)?;
                let operation = VerificationOperation {
                    id: id.clone(),
                    phase: VerificationPhase::ConfirmingPayment,
                    message: None,
                    error: None,
                };
                item.payment_message_hash = Some(message_hash.clone());
                item.operation = Some(operation.clone());
                let transaction_hash = item.payment_transaction_hash.clone();
                let code_hash = item.preview.status.code_hash.clone();
                drop(stored);

                (snapshot, quote, transaction_hash, code_hash, operation)
            };

            // Own the upload after HTTP completion; closing the dialog must not cancel it.
            tokio::spawn(async move {
                let started = Instant::now();
                let progress = Arc::clone(&previews);
                let worker_id = id.clone();

                log::info!(
                    target: "acton::verification",
                    "operation=verify_source target={id} phase=confirming_payment outcome=running"
                );

                let result = tokio::task::spawn_blocking(move || {
                    let transaction_hash = match transaction_hash {
                        Some(hash) => hash,
                        None => {
                            let config = ActonConfig::load()?;
                            let message_hash = HashBytes::from_str(&message_hash)?;
                            wait_for_new_verifier_payment(&config, &quote, &message_hash)?
                        }
                    };
                    {
                        let mut stored = progress
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let item = stored
                            .get_mut(&worker_id)
                            .context("Verification operation was lost")?;
                        item.payment_transaction_hash = Some(transaction_hash.clone());
                        if let Some(operation) = &mut item.operation {
                            operation.phase = VerificationPhase::UploadingSources;
                        }
                        drop(stored);
                    }

                    log::info!(
                        target: "acton::verification",
                        "operation=verify_source target={worker_id} phase=uploading_sources outcome=running"
                    );

                    upload_new_verifier_sources(
                        &new_verifier_backend(),
                        &code_hash,
                        &snapshot.parts,
                        &snapshot.sources_json,
                        &snapshot.compile_params_json,
                        &transaction_hash,
                    )
                })
                .await;
                let result = result
                    .map_err(|error| anyhow!("Verification worker failed: {error}"))
                    .and_then(std::convert::identity);
                let mut stored = previews
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(operation) =
                    stored.get_mut(&id).and_then(|item| item.operation.as_mut())
                {
                    match result {
                        Ok(()) => operation.phase = VerificationPhase::Verified,
                        Err(error) => {
                            operation.phase = VerificationPhase::Failed;
                            operation.error = Some(format!("{error:#}"));
                        }
                    }
                    log::info!(
                        target: "acton::verification",
                        "operation=verify_source target={id} duration_ms={} outcome={:?}",
                        started.elapsed().as_millis(),
                        operation.phase
                    );
                }
            });

            Ok(operation)
        })
    }

    fn operation(
        &self,
        network: PublicTonNetwork,
        id: String,
    ) -> VerificationFuture<'_, VerificationOperation> {
        Box::pin(async move {
            let stored = self
                .previews
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stored
                .get(&id)
                .filter(|item| item.network == network)
                .and_then(|item| item.operation.clone())
                .ok_or_else(expired_preview)
        })
    }
}

fn is_running(item: &StoredPreview) -> bool {
    item.operation.as_ref().is_some_and(|op| {
        (item.payment_message_hash.is_some() && op.phase != VerificationPhase::Verified)
            || matches!(
                op.phase,
                VerificationPhase::UploadingSources | VerificationPhase::ConfirmingPayment
            )
    })
}

fn capture_sources(config: &ActonConfig, path: &Path) -> anyhow::Result<SourceSnapshot> {
    let (code, source_map) = compile_verification_contract(config, path)?;
    let code = Boc::decode_base64(code)?;
    let (parts, paths) = collect_verification_sources(&source_files_from_source_map(&source_map))?;
    anyhow::ensure!(
        !parts.is_empty(),
        "The compiler did not produce source files"
    );
    anyhow::ensure!(
        parts.iter().map(|part| part.bytes.len()).sum::<usize>() <= MAX_SOURCE_BYTES,
        "Source files exceed the 8 MiB verification limit"
    );
    Ok(SourceSnapshot {
        code_hash: *code.repr_hash(),
        parts,
        sources_json: serde_json::to_string(&new_verifier_sources(&paths)?)?,
        compile_params_json: serde_json::to_string(&new_verifier_compile_params(
            config,
            &tolk_compiler::native_tolk_version()?.version,
        )?)?,
    })
}

/// The registry publishes code once for all instances and networks using that hash.
/// A chain lookup is unnecessary and must not change the identity during source review.
fn public_status(code_hash: &str) -> anyhow::Result<VerificationStatus> {
    let code_hash = HashBytes::from_str(code_hash)
        .context("Enter a valid code hash")?
        .to_string();
    let backend = new_verifier_backend();
    let response = build_verify_http_client()?
        .get(format!("{backend}/api/v1/verification/status"))
        .query(&[("code_hash", &code_hash)])
        .send()
        .context("Failed to read Acton verifier status")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        anyhow::bail!(
            "Acton verifier status failed: HTTP {status}: {}",
            truncate_for_display(&body, 4000)
        );
    }
    let status: PublicVerificationStatus = response
        .json()
        .context("Invalid Acton verifier status response")?;
    super::ensure_ticket_code_hash(&code_hash, &status.code_hash)?;

    Ok(VerificationStatus {
        verifier_url: format!("{backend}/{code_hash}"),
        code_hash,
        verified: status.verified,
    })
}

#[derive(Deserialize)]
struct PublicVerificationStatus {
    code_hash: String,
    verified: bool,
}

fn payment_details(quote: &NewVerifierPaymentQuote) -> anyhow::Result<VerificationPayment> {
    let amount = quote
        .amount_nano
        .parse::<u64>()
        .context("Verifier returned an invalid payment amount")?;
    anyhow::ensure!(amount > 0, "Verifier returned a zero payment amount");
    let address = TonAddress::from_str(&quote.payment_address)?;

    Ok(VerificationPayment {
        address: format_ton_address(&address, true),
        amount: amount.to_string(),
        comment: quote.comment.clone(),
        network: PublicTonNetwork::Testnet,
    })
}

fn expired_preview() -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Conflict {
        code: "verification_preview_expired",
        message: "This verification preview expired; check the sources again".to_owned(),
    }
}

fn verification_error(error: impl std::fmt::Display) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "verification_failed",
        message: format!("{error:#}"),
    }
}

fn worker_error(error: tokio::task::JoinError) -> EnvironmentRuntimeError {
    verification_error(format!("Verification worker failed: {error}"))
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    fn reviewed_preview() -> StoredPreview {
        StoredPreview {
            created: Instant::now(),
            network: PublicTonNetwork::Testnet,
            preview: VerificationPreview {
                id: "preview".to_owned(),
                status: VerificationStatus {
                    code_hash: "22".repeat(32),
                    verified: false,
                    verifier_url: "https://verifier.acton.monster/code-hash".to_owned(),
                },
                compiler_version: "1.4.2".to_owned(),
                candidates: Vec::new(),
                payment: None,
            },
            sources: BTreeMap::new(),
            submission: None,
            operation: None,
            quote: None,
            payment_message_hash: None,
            payment_transaction_hash: None,
        }
    }

    #[tokio::test]
    async fn publication_rejects_network_mismatch_unmatched_code_and_expired_previews() {
        let runtime = ProjectVerificationRuntime::default();
        runtime
            .previews
            .lock()
            .unwrap()
            .insert("preview".to_owned(), reviewed_preview());
        let request = StartVerificationRequest {
            preview_id: "preview".to_owned(),
            contract_id: "Counter".to_owned(),
            sender_address: format!("0:{}", "33".repeat(32)),
        };

        let wrong_network = runtime
            .start(PublicTonNetwork::Mainnet, request.clone())
            .await
            .unwrap_err();
        let mismatched_code = runtime
            .start(PublicTonNetwork::Testnet, request.clone())
            .await
            .unwrap_err();

        runtime
            .previews
            .lock()
            .unwrap()
            .get_mut("preview")
            .unwrap()
            .created = Instant::now() - PREVIEW_TTL;
        let expired = runtime
            .start(PublicTonNetwork::Testnet, request)
            .await
            .unwrap_err();

        expect![[r#"
            [
                Conflict {
                    code: "verification_preview_expired",
                    message: "This verification preview expired; check the sources again",
                },
                InvalidRequest {
                    code: "verification_code_mismatch",
                    message: "Select a project contract whose compiled code matches the requested code hash",
                },
                Conflict {
                    code: "verification_preview_expired",
                    message: "This verification preview expired; check the sources again",
                },
            ]
        "#]].assert_debug_eq(&[wrong_network, mismatched_code, expired]);
    }

    #[tokio::test]
    async fn repeated_publication_returns_the_operation_without_reuploading_or_changing_payer() {
        let runtime = ProjectVerificationRuntime::default();
        let sender = format!("0:{}", "33".repeat(32));
        let mut stored = reviewed_preview();
        stored.submission = Some((
            "Counter".to_owned(),
            format_ton_address(&TonAddress::from_str(&sender).unwrap(), true),
        ));
        stored.operation = Some(VerificationOperation {
            id: "preview".to_owned(),
            phase: VerificationPhase::Ready,
            message: None,
            error: None,
        });
        runtime
            .previews
            .lock()
            .unwrap()
            .insert("preview".to_owned(), stored);

        let request = StartVerificationRequest {
            preview_id: "preview".to_owned(),
            contract_id: "Counter".to_owned(),
            sender_address: sender,
        };
        let repeated = runtime
            .start(PublicTonNetwork::Testnet, request.clone())
            .await
            .unwrap();
        let changed = runtime
            .start(
                PublicTonNetwork::Testnet,
                StartVerificationRequest {
                    sender_address: format!("0:{}", "44".repeat(32)),
                    ..request
                },
            )
            .await
            .unwrap_err();

        expect![[r#"
            (
                VerificationOperation {
                    id: "preview",
                    phase: Ready,
                    message: None,
                    error: None,
                },
                Conflict {
                    code: "verification_already_started",
                    message: "This preview was already submitted with another contract or wallet; check the sources again",
                },
            )
        "#]].assert_debug_eq(&(repeated, changed));
    }

    #[tokio::test]
    async fn payment_retry_preserves_reference_and_network_without_starting_another_upload() {
        let runtime = ProjectVerificationRuntime::default();
        let mut stored = reviewed_preview();
        stored.payment_message_hash = Some("33".repeat(32));
        stored.operation = Some(VerificationOperation {
            id: "preview".to_owned(),
            phase: VerificationPhase::ConfirmingPayment,
            message: None,
            error: None,
        });
        runtime
            .previews
            .lock()
            .unwrap()
            .insert("preview".to_owned(), stored);

        let mut results = Vec::new();
        for (network, hash) in [
            (PublicTonNetwork::Testnet, "33"),
            (PublicTonNetwork::Testnet, "44"),
            (PublicTonNetwork::Mainnet, "33"),
        ] {
            results.push(
                runtime
                    .complete_payment(
                        network,
                        "preview".to_owned(),
                        VerificationPaymentRequest {
                            message_hash: format!("0x{}", hash.repeat(32)),
                        },
                    )
                    .await,
            );
        }

        expect![[r#"
            [
                Ok(
                    VerificationOperation {
                        id: "preview",
                        phase: ConfirmingPayment,
                        message: None,
                        error: None,
                    },
                ),
                Err(
                    Conflict {
                        code: "verification_payment_changed",
                        message: "This operation already has a payment; retry it without sending another transaction",
                    },
                ),
                Err(
                    Conflict {
                        code: "verification_preview_expired",
                        message: "This verification preview expired; check the sources again",
                    },
                ),
            ]
        "#]].assert_debug_eq(&results);
    }
}
