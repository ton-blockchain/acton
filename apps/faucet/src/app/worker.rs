use apalis::prelude::Data;
use faucet_valkey::{
    AmountWindowDecision, AntifraudModule, SentAmountWindowDecision, SuccessfulClaimWindowDecision,
};
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder};
use rston::models::{
    CurrencyCollection, IntMsgInfo, MsgInfo, OwnedMessage, StdAddr, StdAddrFormat,
};
use rston::wallet::{SendMsgFlags, WalletMessage};
use std::time::Duration as StdDuration;
use time::OffsetDateTime;
use toncenter::ToncenterClient;
use tracing::{error, info, warn};

use crate::antifraud::{
    audit::{AuditSubject, PayoutAudit},
    subject,
};
use crate::auth::github::FaucetTier;
use crate::blockchain::wallet::Wallet;
use crate::handlers::CreateClaim;

use super::AppState;

#[tracing::instrument(
    name = "claim",
    skip_all,
    fields(request_id = %task.request_id)
)]
pub(super) async fn send_claim(task: CreateClaim, state: Data<AppState>) -> anyhow::Result<()> {
    let wallet = state.wallet.as_ref();
    let client = state.client.as_ref();
    let amount = state.config.faucet.amount;
    let payout_audit = payout_audit(&task, amount)?;

    info!("Processing claim for address: {}", task.address);

    if !can_process_successful_claim_window(&state, &task).await? {
        return Ok(());
    }

    if !can_process_subnet_amount_window(&state, &task, amount).await? {
        return Ok(());
    }

    wait_for_sent_amount_window(&state, &task.address, amount).await?;

    let max_retries = state.config.worker.max_retries;

    for attempt in 0..=max_retries {
        let status = process_send_tokens(
            wallet,
            client,
            &task.address,
            amount,
            &state.config.faucet.message,
        )
        .await;

        match status {
            Ok(_) => {
                let paid_at = OffsetDateTime::now_utc().unix_timestamp();
                record_payout_audit(&state, &payout_audit, paid_at).await;
                record_successful_claim(&state, &task).await;
                record_sent_subnet_amount(&state, &task, amount).await;
                match state.valkey.add_sent_amount(amount).await {
                    Ok(total_sent_nanocoins) => {
                        info!(
                            address = %task.address,
                            amount,
                            total_sent_nanocoins,
                            "Recorded sent amount in Valkey"
                        );
                    }
                    Err(err) => {
                        warn!(
                            address = %task.address,
                            amount,
                            error = %err,
                            "Failed to record sent amount in Valkey"
                        );
                    }
                }
                info!("Successfully sent claim to {}", task.address);
                return Ok(());
            }
            Err(err) => {
                if attempt < max_retries {
                    let delay =
                        exponential_backoff(state.config.worker.retry_base_delay_ms, attempt);
                    warn!(
                        address = %task.address,
                        attempt = attempt + 1,
                        max_attempts = max_retries + 1,
                        retry_in_ms = delay.as_millis(),
                        error = %err,
                        "Claim send attempt failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                error!(
                    address = %task.address,
                    attempts = max_retries + 1,
                    error = %err,
                    "Failed to send claim after retries"
                );
                anyhow::bail!("Failed to send claim: {}", err);
            }
        }
    }

    unreachable!("send_claim loop should always return");
}

fn payout_audit(task: &CreateClaim, amount: u64) -> anyhow::Result<PayoutAudit> {
    let mut subjects = vec![
        AuditSubject::wallet(task.address.clone())?,
        AuditSubject::ip(task.client_ip),
        AuditSubject::device_uid(&task.device_uid)?,
    ];

    if let Some(github_user_id) = task.github_user_id {
        subjects.push(AuditSubject::github_user_id(github_user_id));
    }

    PayoutAudit::new(task.request_id.clone(), amount, &task.client_kind, subjects)
}

async fn record_payout_audit(state: &AppState, payout: &PayoutAudit, paid_at: i64) {
    let mut attempt = 0;
    loop {
        match state.antifraud_audit.record_payout(payout, paid_at).await {
            Ok(()) => {
                info!(paid_at, "Recorded successful payout in antifraud audit");
                return;
            }
            Err(err) => {
                let delay = exponential_backoff(state.config.worker.retry_base_delay_ms, attempt);
                warn!(
                    attempt = attempt.saturating_add(1),
                    retry_in_ms = delay.as_millis(),
                    error = %err,
                    "Failed to record successful payout in antifraud audit; retrying without resending"
                );
                tokio::time::sleep(delay).await;
                attempt = attempt.saturating_add(1);
            }
        }
    }
}

async fn can_process_subnet_amount_window(
    state: &AppState,
    task: &CreateClaim,
    amount: u64,
) -> anyhow::Result<bool> {
    let Some(window) = state.antifraud.subnet_amount_window() else {
        return Ok(true);
    };
    let Some(subject) = task.subnet_amount_window_subject.as_deref() else {
        return Ok(true);
    };

    if let Err(err) = state.antifraud.check_subnet_amount_window_transfer(amount) {
        error!(
            module = AntifraudModule::SubnetAmountWindow.name(),
            reason = "claim-amount-exceeds-window-limit",
            address = %task.address,
            subject,
            amount,
            max_amount = window.max_amount,
            error = ?err,
            "Antifraud module triggered"
        );
        state
            .record_antifraud_trigger(AntifraudModule::SubnetAmountWindow, &task.address)
            .await;
        return Ok(false);
    }

    match state
        .valkey
        .check_subnet_amount_window(subject, amount, window.max_amount, window.window_seconds)
        .await?
    {
        AmountWindowDecision::Allowed {
            current,
            attempted,
            max,
            window_seconds,
        } => {
            info!(
                address = %task.address,
                subject,
                current_sent_nanocoins = current,
                attempted_amount = attempted,
                max_amount = max,
                window_seconds,
                "Subnet amount sliding window allows send"
            );
            Ok(true)
        }
        AmountWindowDecision::Limited {
            current,
            attempted,
            max,
            window_seconds,
            retry_after_ms,
        } => {
            warn!(
                module = AntifraudModule::SubnetAmountWindow.name(),
                reason = "subnet-amount-window-limit-reached",
                address = %task.address,
                subject,
                current_sent_nanocoins = current,
                attempted_amount = attempted,
                max_amount = max,
                window_seconds,
                retry_after_ms,
                "Antifraud module triggered"
            );
            state
                .record_antifraud_trigger(AntifraudModule::SubnetAmountWindow, &task.address)
                .await;
            Ok(false)
        }
    }
}

async fn record_sent_subnet_amount(state: &AppState, task: &CreateClaim, amount: u64) {
    let Some(window) = state.antifraud.subnet_amount_window() else {
        return;
    };
    let Some(subject) = task.subnet_amount_window_subject.as_deref() else {
        return;
    };

    match state
        .valkey
        .record_subnet_amount_window(subject, amount, window.window_seconds)
        .await
    {
        Ok(total) => {
            info!(
                address = %task.address,
                subject,
                amount,
                sent_in_window_nanocoins = total,
                window_seconds = window.window_seconds,
                "Recorded sent amount for subnet in Valkey"
            );
        }
        Err(err) => {
            warn!(
                address = %task.address,
                subject,
                amount,
                error = %err,
                "Failed to record sent amount for subnet in Valkey"
            );
        }
    }
}

// TODO: вынести куда-то
async fn can_process_successful_claim_window(
    state: &AppState,
    task: &CreateClaim,
) -> anyhow::Result<bool> {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return Ok(true);
    };

    let max_requests = if task.max_requests == 0 {
        window.max_requests
    } else {
        task.max_requests
    };
    let address_key = normalized_address_key(&task.address)?;

    if !claim_window_allows(
        state,
        &address_key,
        max_requests,
        window.window_seconds,
        &task.address,
    )
    .await?
    {
        return Ok(false);
    }

    if let Some(github_user_id) = task.github_user_id
        && !claim_window_allows(
            state,
            &subject::github(github_user_id),
            max_requests,
            window.window_seconds,
            &task.address,
        )
        .await?
    {
        return Ok(false);
    }

    if let Some(device_subject) = task.device_window_subject.as_deref()
        && !claim_window_allows(
            state,
            device_subject,
            max_requests,
            window.window_seconds,
            &task.address,
        )
        .await?
    {
        return Ok(false);
    }

    if task.tier == FaucetTier::Guest
        && let Some(client_subject) = task.client_window_subject.as_deref()
        && !claim_window_allows(
            state,
            client_subject,
            window.max_requests,
            window.window_seconds,
            &task.address,
        )
        .await?
    {
        return Ok(false);
    }

    Ok(true)
}

async fn claim_window_allows(
    state: &AppState,
    subject: &str,
    max_requests: u32,
    window_seconds: u64,
    address: &str,
) -> anyhow::Result<bool> {
    match state
        .valkey
        .check_successful_claim_window(subject, max_requests, window_seconds)
        .await?
    {
        SuccessfulClaimWindowDecision::Allowed {
            current,
            max,
            window_seconds,
        } => {
            info!(
                address = %address,
                subject,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                "Successful claim window allows send"
            );
            Ok(true)
        }
        SuccessfulClaimWindowDecision::Limited {
            current,
            max,
            window_seconds,
            retry_after_ms,
        } => {
            warn!(
                module = AntifraudModule::SuccessfulClaimWindow.name(),
                reason = "successful-claim-window-limit-reached",
                address = %address,
                subject,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                retry_after_ms,
                "Antifraud module triggered"
            );
            state
                .record_antifraud_trigger(AntifraudModule::SuccessfulClaimWindow, address)
                .await;
            Ok(false)
        }
    }
}

async fn record_successful_claim(state: &AppState, task: &CreateClaim) {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return;
    };

    let address_key = match normalized_address_key(&task.address) {
        Ok(address_key) => address_key,
        Err(err) => {
            warn!(
                address = %task.address,
                error = %err,
                "Failed to normalize successful claim address"
            );
            return;
        }
    };

    record_successful_claim_subject(state, &address_key, &task.address, window.window_seconds)
        .await;
    if let Some(github_user_id) = task.github_user_id {
        record_successful_claim_subject(
            state,
            &subject::github(github_user_id),
            &task.address,
            window.window_seconds,
        )
        .await;
    }
    if let Some(device_subject) = task.device_window_subject.as_deref() {
        record_successful_claim_subject(
            state,
            device_subject,
            &task.address,
            window.window_seconds,
        )
        .await;
    }
    // Record authenticated sends against the client subject as well. If the user
    // later drops the bearer token, the stricter guest check still sees them.
    if let Some(client_subject) = task.client_window_subject.as_deref() {
        record_successful_claim_subject(
            state,
            client_subject,
            &task.address,
            window.window_seconds,
        )
        .await;
    }
}

async fn record_successful_claim_subject(
    state: &AppState,
    subject: &str,
    address: &str,
    window_seconds: u64,
) {
    match state
        .valkey
        .record_successful_claim(subject, window_seconds)
        .await
    {
        Ok(successful_claims) => {
            info!(
                address = %address,
                subject,
                successful_claims,
                window_seconds,
                "Recorded successful claim in Valkey"
            );
        }
        Err(err) => {
            warn!(
                address = %address,
                subject,
                error = %err,
                "Failed to record successful claim in Valkey"
            );
        }
    }
}

fn normalized_address_key(address: &str) -> anyhow::Result<String> {
    let (address, _) = StdAddr::from_str_ext(address, StdAddrFormat::any())?;
    Ok(address.to_string())
}

async fn wait_for_sent_amount_window(
    state: &AppState,
    address: &str,
    amount: u64,
) -> anyhow::Result<()> {
    let Some(window) = state.antifraud.sent_amount_window() else {
        return Ok(());
    };

    if let Err(err) = state.antifraud.check_sent_amount_window_transfer(amount) {
        error!(
            module = AntifraudModule::SentAmountWindow.name(),
            reason = "claim-amount-exceeds-window-limit",
            address = %address,
            amount,
            max_amount = window.max_amount,
            error = ?err,
            "Antifraud module triggered"
        );
        state
            .record_antifraud_trigger(AntifraudModule::SentAmountWindow, address)
            .await;
        anyhow::bail!("Claim amount exceeds sent amount window limit: {err:?}");
    }

    let mut trigger_recorded = false;
    loop {
        match state
            .valkey
            .reserve_sent_amount_window(amount, window.max_amount, window.window_seconds)
            .await?
        {
            SentAmountWindowDecision::Reserved(reservation) => {
                info!(
                    address = %address,
                    amount,
                    reserved_total_nanocoins = reservation.total,
                    max_amount = reservation.max,
                    window_seconds = reservation.window_seconds,
                    "Reserved sent amount sliding window"
                );
                return Ok(());
            }
            SentAmountWindowDecision::Limited {
                current,
                attempted,
                max,
                window_seconds,
                retry_after_ms,
            } => {
                if !trigger_recorded {
                    warn!(
                        module = AntifraudModule::SentAmountWindow.name(),
                        reason = "sent-amount-window-limit-reached",
                        address = %address,
                        current_sent_nanocoins = current,
                        attempted_amount = attempted,
                        max_amount = max,
                        window_seconds,
                        retry_after_ms,
                        "Antifraud module triggered"
                    );
                    state
                        .record_antifraud_trigger(AntifraudModule::SentAmountWindow, address)
                        .await;
                    trigger_recorded = true;
                }
                tokio::time::sleep(StdDuration::from_millis(retry_after_ms.max(1))).await;
            }
        }
    }
}

fn exponential_backoff(base_delay_ms: u64, attempt: u32) -> StdDuration {
    let multiplier = 1u64 << attempt.min(8);
    StdDuration::from_millis(base_delay_ms.saturating_mul(multiplier))
}

async fn process_send_tokens(
    wallet: &Wallet,
    client: &ToncenterClient,
    dest: &str,
    amount: u64,
    message: &str,
) -> anyhow::Result<()> {
    let (dest, _) = StdAddr::from_str_ext(dest, StdAddrFormat::any())?;

    let wallet_message = WalletMessage {
        mode: SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR,
        msg: build_message(wallet, amount, dest, message)?,
    };

    let seqno = client.get_wallet_seqno(&wallet.get_address()).await?;

    let expire_at = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        + 600) as u32;

    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![wallet_message], seqno, expire_at, false)?;

    client.send_boc(&Boc::encode_base64(external)).await?;

    Ok(())
}

fn build_message(
    wallet: &Wallet,
    amount: u64,
    dest: StdAddr,
    message: &str,
) -> anyhow::Result<Cell> {
    let message_info = IntMsgInfo {
        ihr_disabled: true,
        bounce: false,
        bounced: false,
        src: wallet.wallet.address.clone().into(),
        dst: dest.into(),
        value: CurrencyCollection::new(amount.into()),
        extra_flags: Default::default(),
        fwd_fee: Default::default(),
        created_at: 0,
        created_lt: 0,
    };

    let mut message_body_builder = CellBuilder::new();
    message_body_builder.store_u32(0)?;
    message_body_builder.store_raw(
        message.as_bytes(),
        u16::try_from(message.len().saturating_mul(8))?,
    )?;
    let message_body = message_body_builder.build()?;

    let message = OwnedMessage {
        info: MsgInfo::Int(message_info),
        init: None,
        body: message_body.into(),
        layout: None,
    };

    Ok(CellBuilder::build_from(message)?)
}
