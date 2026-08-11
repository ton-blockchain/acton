use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tycho_types::{boc::Boc, cell::CellSlice};

use crate::{
    blockchain::{ToncenterClient, is_valid_code_hash, is_valid_hash, normalize_hash},
    config::{Config, TonNetwork},
};

pub const PAYMENT_COMMENT_PREFIX: &str = "acton-verify:v1:";
const HISTORY_PAGE_SIZE: usize = 1_000;
const PROCESSING_LEASE: Duration = Duration::from_mins(5);
const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PAYMENT_ATTEMPTS: u64 = 3;

#[derive(Clone, Debug, Serialize)]
pub struct PaymentQuote {
    pub payment_address: String,
    pub amount_nano: String,
    pub comment: String,
}

#[derive(Clone, Debug)]
pub struct PaymentClaim {
    pub transaction_hash: String,
    pub claim_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentAttemptOutcome {
    Consumed,
    Retryable,
}

#[async_trait]
pub trait PaymentVerifier: Send + Sync + 'static {
    fn quote(&self, code_hash: &str) -> PaymentQuote;
    fn is_ready(&self) -> bool;

    /// Rebuilds replay state from the payment wallet history.
    ///
    /// # Errors
    ///
    /// Returns an error when history or ledger access fails.
    async fn recover(&self) -> Result<(), PaymentError>;

    /// Validates and reserves one payment transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the payment is invalid, unavailable, or already used.
    async fn claim(
        &self,
        transaction_hash: &str,
        code_hash: &str,
    ) -> Result<PaymentClaim, PaymentError>;

    /// Records the final state of a claimed payment.
    ///
    /// # Errors
    ///
    /// Returns an error when ledger access fails or the claim state changed.
    fn finish(
        &self,
        claim: &PaymentClaim,
        outcome: PaymentAttemptOutcome,
    ) -> Result<(), PaymentError>;
}

#[async_trait]
pub trait PaymentBlockchainClient: Send + Sync + 'static {
    async fn transaction_by_hash(
        &self,
        transaction_hash: &str,
    ) -> Result<Option<PaymentTransaction>, PaymentError>;

    async fn transactions(
        &self,
        account: &str,
        limit: usize,
        offset: usize,
        sort: HistorySort,
    ) -> Result<Vec<PaymentTransaction>, PaymentError>;
}

#[derive(Clone, Copy, Debug)]
pub enum HistorySort {
    Ascending,
    Descending,
}

impl HistorySort {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "asc",
            Self::Descending => "desc",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PaymentTransaction {
    pub account: String,
    pub hash: String,
    pub lt: u64,
    pub timestamp: u64,
    pub emulated: bool,
    pub finality: String,
    pub aborted: bool,
    pub incoming: Option<PaymentMessage>,
}

#[derive(Clone, Debug)]
pub struct PaymentMessage {
    pub destination: Option<String>,
    pub value: Option<u64>,
    pub bounced: bool,
    pub comment: Option<String>,
}

pub struct OnchainPaymentVerifier {
    client: Arc<dyn PaymentBlockchainClient>,
    ledger: PaymentLedger,
    payment_address: String,
    min_amount_nano: u64,
    ready: AtomicBool,
}

impl OnchainPaymentVerifier {
    /// Opens the payment ledger and configures the testnet payment verifier.
    ///
    /// # Errors
    ///
    /// Returns an error when payment configuration is missing or invalid, or when the ledger
    /// cannot be opened.
    pub fn from_config(config: &Config) -> Result<Self, PaymentError> {
        if config.network() != TonNetwork::Testnet {
            return Err(PaymentError::UnsupportedNetwork);
        }
        let payment_address = config
            .payment_address()
            .ok_or(PaymentError::MissingConfiguration("payment.address"))?;
        validate_payment_address(payment_address)?;
        let min_amount_nano = config
            .payment_min_amount_nano()
            .filter(|amount| *amount > 0)
            .ok_or(PaymentError::MissingConfiguration(
                "payment.min_amount_nano",
            ))?;

        Ok(Self::new(
            Arc::new(ToncenterClient::from_config(config)),
            PaymentLedger::open(config.payment_ledger_path())?,
            payment_address.to_owned(),
            min_amount_nano,
        ))
    }

    #[must_use]
    pub fn new(
        client: Arc<dyn PaymentBlockchainClient>,
        ledger: PaymentLedger,
        payment_address: String,
        min_amount_nano: u64,
    ) -> Self {
        Self {
            client,
            ledger,
            payment_address,
            min_amount_nano,
            ready: AtomicBool::new(false),
        }
    }

    async fn load_history(&self) -> Result<Vec<RecoveredPayment>, PaymentError> {
        let newest = self
            .client
            .transactions(&self.payment_address, 1, 0, HistorySort::Descending)
            .await?;
        let Some(tip_hash) = newest.first().map(|transaction| transaction.hash.clone()) else {
            return Ok(Vec::new());
        };

        let mut offset = 0;
        let mut recovered = BTreeMap::new();
        let mut reached_tip = false;
        let mut previous_transaction = None::<(u64, String)>;

        while !reached_tip {
            let page = self
                .client
                .transactions(
                    &self.payment_address,
                    HISTORY_PAGE_SIZE,
                    offset,
                    HistorySort::Ascending,
                )
                .await?;
            let page_len = page.len();

            for transaction in page {
                let transaction_key = (transaction.lt, transaction.hash.clone());
                if previous_transaction.as_ref().is_some_and(|previous| {
                    transaction_key.0 < previous.0
                        || (transaction_key.0 == previous.0
                            && transaction_key.1.as_str() <= previous.1.as_str())
                }) {
                    return Err(PaymentError::HistoryChangedDuringRecovery);
                }
                previous_transaction = Some(transaction_key);
                reached_tip |= transaction.hash == tip_hash;
                if let Some(payment) = self.recovered_payment(transaction) {
                    recovered.insert(payment.transaction_hash.clone(), payment);
                }
            }

            if reached_tip || page_len < HISTORY_PAGE_SIZE {
                break;
            }
            offset = offset
                .checked_add(page_len)
                .ok_or(PaymentError::HistoryOffsetOverflow)?;
        }

        if !reached_tip {
            return Err(PaymentError::HistoryChangedDuringRecovery);
        }

        Ok(recovered.into_values().collect())
    }

    fn recovered_payment(&self, transaction: PaymentTransaction) -> Option<RecoveredPayment> {
        let incoming = valid_incoming_message(&transaction, &self.payment_address)?;
        let comment = incoming.comment.as_deref()?;
        let code_hash = code_hash_from_comment(comment)?;
        let amount_nano = incoming.value?;
        if amount_nano < self.min_amount_nano {
            return None;
        }

        Some(RecoveredPayment {
            transaction_hash: transaction.hash,
            code_hash,
            amount_nano,
            lt: transaction.lt,
            transaction_time: transaction.timestamp,
        })
    }

    fn validate_for_claim(
        &self,
        transaction: &PaymentTransaction,
        code_hash: &str,
    ) -> Result<(), PaymentError> {
        let incoming = valid_incoming_message(transaction, &self.payment_address)
            .ok_or(PaymentError::InvalidTransaction)?;
        let amount = incoming.value.ok_or(PaymentError::MissingAmount)?;
        if amount < self.min_amount_nano {
            return Err(PaymentError::InsufficientAmount {
                expected: self.min_amount_nano,
                actual: amount,
            });
        }

        let expected_comment = payment_comment(code_hash);
        let actual_comment = incoming.comment.as_deref().unwrap_or_default();
        if actual_comment != expected_comment {
            return Err(PaymentError::CodeHashMismatch);
        }

        Ok(())
    }
}

#[async_trait]
impl PaymentVerifier for OnchainPaymentVerifier {
    fn quote(&self, code_hash: &str) -> PaymentQuote {
        PaymentQuote {
            payment_address: self.payment_address.clone(),
            amount_nano: self.min_amount_nano.to_string(),
            comment: payment_comment(code_hash),
        }
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    async fn recover(&self) -> Result<(), PaymentError> {
        self.ready.store(false, Ordering::Release);
        let payments = self.load_history().await?;
        self.ledger.merge_with_consumed(&payments)?;
        self.ready.store(true, Ordering::Release);
        tracing::info!(
            payment_count = payments.len(),
            payment_address = %self.payment_address,
            "payment ledger recovered from testnet history"
        );
        Ok(())
    }

    async fn claim(
        &self,
        transaction_hash: &str,
        code_hash: &str,
    ) -> Result<PaymentClaim, PaymentError> {
        if !self.is_ready() {
            return Err(PaymentError::RecoveryInProgress);
        }
        let transaction_hash = normalize_hash(transaction_hash);
        self.ledger.precheck(&transaction_hash, code_hash)?;
        let transaction = self
            .client
            .transaction_by_hash(&transaction_hash)
            .await?
            .ok_or(PaymentError::TransactionNotFound)?;
        if transaction.hash != transaction_hash {
            return Err(PaymentError::TransactionHashMismatch {
                expected: transaction_hash,
                actual: transaction.hash,
            });
        }
        self.validate_for_claim(&transaction, code_hash)?;

        let incoming = transaction
            .incoming
            .as_ref()
            .ok_or(PaymentError::InvalidTransaction)?;
        let recovered = RecoveredPayment {
            transaction_hash: transaction.hash,
            code_hash: code_hash.to_owned(),
            amount_nano: incoming.value.ok_or(PaymentError::MissingAmount)?,
            lt: transaction.lt,
            transaction_time: transaction.timestamp,
        };
        self.ledger.reserve(&recovered)
    }

    fn finish(
        &self,
        claim: &PaymentClaim,
        outcome: PaymentAttemptOutcome,
    ) -> Result<(), PaymentError> {
        self.ledger.finish(claim, outcome)
    }
}

fn valid_incoming_message<'a>(
    transaction: &'a PaymentTransaction,
    payment_address: &str,
) -> Option<&'a PaymentMessage> {
    if transaction.emulated
        || transaction.finality != "finalized"
        || transaction.aborted
        || !addresses_equal(&transaction.account, payment_address)
    {
        return None;
    }

    let incoming = transaction.incoming.as_ref()?;
    if incoming.bounced
        || !incoming
            .destination
            .as_deref()
            .is_some_and(|destination| addresses_equal(destination, payment_address))
    {
        return None;
    }
    Some(incoming)
}

const fn addresses_equal(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn validate_payment_address(address: &str) -> Result<(), PaymentError> {
    let Some((workchain, hash)) = address.split_once(':') else {
        return Err(PaymentError::InvalidPaymentAddress);
    };
    if workchain != "0" || hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PaymentError::InvalidPaymentAddress);
    }
    Ok(())
}

#[must_use]
pub fn payment_comment(code_hash: &str) -> String {
    format!("{PAYMENT_COMMENT_PREFIX}{code_hash}")
}

fn code_hash_from_comment(comment: &str) -> Option<String> {
    let code_hash = comment.strip_prefix(PAYMENT_COMMENT_PREFIX)?;
    is_valid_code_hash(code_hash).then(|| code_hash.to_ascii_lowercase())
}

struct RecoveredPayment {
    transaction_hash: String,
    code_hash: String,
    amount_nano: u64,
    lt: u64,
    transaction_time: u64,
}

pub struct PaymentLedger {
    connection: Mutex<Connection>,
}

impl PaymentLedger {
    /// Opens a persistent payment ledger.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory, database, or schema cannot be initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PaymentError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| PaymentError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an in-memory payment ledger.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema cannot be initialized.
    pub fn in_memory() -> Result<Self, PaymentError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, PaymentError> {
        connection.execute_batch(
            r"
            pragma foreign_keys = on;
            create table if not exists payment_transactions (
              transaction_hash text primary key,
              code_hash text not null,
              amount_nano integer not null,
              lt integer not null,
              transaction_time integer not null,
              state text not null check (state in ('processing', 'retryable', 'consumed')),
              updated_at integer not null,
              claim_version integer not null
            );
            ",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, PaymentError> {
        self.connection.lock().map_err(|_| PaymentError::LedgerLock)
    }

    fn precheck(&self, transaction_hash: &str, code_hash: &str) -> Result<(), PaymentError> {
        let now = now_unix_seconds()?;
        let connection = self.connection()?;
        let existing = existing_payment(&connection, transaction_hash)?;
        drop(connection);

        let Some(existing) = existing else {
            return Ok(());
        };
        if existing.code_hash != code_hash {
            return Err(PaymentError::AlreadyUsed);
        }
        match existing.state.as_str() {
            "retryable" if existing.claim_version < MAX_PAYMENT_ATTEMPTS => Ok(()),
            "processing"
                if now.saturating_sub(existing.updated_at) >= PROCESSING_LEASE.as_secs()
                    && existing.claim_version < MAX_PAYMENT_ATTEMPTS =>
            {
                Ok(())
            }
            "processing"
                if now.saturating_sub(existing.updated_at) < PROCESSING_LEASE.as_secs() =>
            {
                Err(PaymentError::InProgress)
            }
            _ => Err(PaymentError::AlreadyUsed),
        }
    }

    fn merge_with_consumed(&self, payments: &[RecoveredPayment]) -> Result<(), PaymentError> {
        let now = now_unix_seconds()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "update payment_transactions set state = 'consumed', updated_at = ?1",
            [u64_to_i64("updated_at", now)?],
        )?;
        for payment in payments {
            transaction.execute(
                r"
                insert into payment_transactions (
                  transaction_hash, code_hash, amount_nano, lt, transaction_time,
                  state, updated_at, claim_version
                ) values (?1, ?2, ?3, ?4, ?5, 'consumed', ?6, 0)
                on conflict(transaction_hash) do update set
                  code_hash = excluded.code_hash,
                  amount_nano = excluded.amount_nano,
                  lt = excluded.lt,
                  transaction_time = excluded.transaction_time,
                  state = 'consumed',
                  updated_at = excluded.updated_at
                ",
                params![
                    payment.transaction_hash,
                    payment.code_hash,
                    u64_to_i64("amount_nano", payment.amount_nano)?,
                    u64_to_i64("lt", payment.lt)?,
                    u64_to_i64("transaction_time", payment.transaction_time)?,
                    u64_to_i64("updated_at", now)?,
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        Ok(())
    }

    fn reserve(&self, payment: &RecoveredPayment) -> Result<PaymentClaim, PaymentError> {
        let now = now_unix_seconds()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let existing = existing_payment(&transaction, &payment.transaction_hash)?;
        let claim_version;

        match existing {
            None => {
                claim_version = 1;
                transaction.execute(
                    r"
                    insert into payment_transactions (
                      transaction_hash, code_hash, amount_nano, lt, transaction_time,
                      state, updated_at, claim_version
                    ) values (?1, ?2, ?3, ?4, ?5, 'processing', ?6, ?7)
                    ",
                    params![
                        payment.transaction_hash,
                        payment.code_hash,
                        u64_to_i64("amount_nano", payment.amount_nano)?,
                        u64_to_i64("lt", payment.lt)?,
                        u64_to_i64("transaction_time", payment.transaction_time)?,
                        u64_to_i64("updated_at", now)?,
                        u64_to_i64("claim_version", claim_version)?,
                    ],
                )?;
            }
            Some(existing) if existing.code_hash != payment.code_hash => {
                return Err(PaymentError::AlreadyUsed);
            }
            Some(existing) if existing.state == "retryable" => {
                if existing.claim_version >= MAX_PAYMENT_ATTEMPTS {
                    return Err(PaymentError::AlreadyUsed);
                }
                claim_version = next_claim_version(existing.claim_version)?;
                reclaim_payment(
                    &transaction,
                    &payment.transaction_hash,
                    "retryable",
                    existing.claim_version,
                    claim_version,
                    now,
                )?;
            }
            Some(existing) if existing.state == "processing" => {
                let lease_expired =
                    now.saturating_sub(existing.updated_at) >= PROCESSING_LEASE.as_secs();
                if !lease_expired {
                    return Err(PaymentError::InProgress);
                }
                if existing.claim_version >= MAX_PAYMENT_ATTEMPTS {
                    return Err(PaymentError::AlreadyUsed);
                }
                claim_version = next_claim_version(existing.claim_version)?;
                reclaim_payment(
                    &transaction,
                    &payment.transaction_hash,
                    "processing",
                    existing.claim_version,
                    claim_version,
                    now,
                )?;
            }
            Some(_) => return Err(PaymentError::AlreadyUsed),
        }

        transaction.commit()?;
        drop(connection);
        Ok(PaymentClaim {
            transaction_hash: payment.transaction_hash.clone(),
            claim_version,
        })
    }

    fn finish(
        &self,
        claim: &PaymentClaim,
        outcome: PaymentAttemptOutcome,
    ) -> Result<(), PaymentError> {
        let state = match outcome {
            PaymentAttemptOutcome::Consumed => "consumed",
            PaymentAttemptOutcome::Retryable => "retryable",
        };
        let now = now_unix_seconds()?;
        let connection = self.connection()?;
        let updated = connection.execute(
            r"
            update payment_transactions
            set state = ?2, updated_at = ?3
            where transaction_hash = ?1 and state = 'processing' and claim_version = ?4
            ",
            params![
                claim.transaction_hash,
                state,
                u64_to_i64("updated_at", now)?,
                u64_to_i64("claim_version", claim.claim_version)?,
            ],
        )?;
        drop(connection);
        if updated != 1 {
            return Err(PaymentError::LedgerInvariant);
        }
        Ok(())
    }
}

struct ExistingPayment {
    code_hash: String,
    state: String,
    updated_at: u64,
    claim_version: u64,
}

fn existing_payment(
    connection: &Connection,
    transaction_hash: &str,
) -> Result<Option<ExistingPayment>, PaymentError> {
    Ok(connection
        .query_row(
            r"
            select code_hash, state, updated_at, claim_version
            from payment_transactions
            where transaction_hash = ?1
            ",
            params![transaction_hash],
            |row| {
                Ok(ExistingPayment {
                    code_hash: row.get(0)?,
                    state: row.get(1)?,
                    updated_at: row.get(2)?,
                    claim_version: row.get(3)?,
                })
            },
        )
        .optional()?)
}

fn next_claim_version(current: u64) -> Result<u64, PaymentError> {
    current.checked_add(1).ok_or(PaymentError::IntegerOverflow {
        field: "claim_version",
        value: current,
    })
}

fn reclaim_payment(
    transaction: &rusqlite::Transaction<'_>,
    transaction_hash: &str,
    expected_state: &str,
    expected_claim_version: u64,
    claim_version: u64,
    updated_at: u64,
) -> Result<(), PaymentError> {
    let updated = transaction.execute(
        r"
        update payment_transactions
        set state = 'processing', updated_at = ?4, claim_version = ?5
        where transaction_hash = ?1 and state = ?2 and claim_version = ?3
        ",
        params![
            transaction_hash,
            expected_state,
            u64_to_i64("claim_version", expected_claim_version)?,
            u64_to_i64("updated_at", updated_at)?,
            u64_to_i64("claim_version", claim_version)?,
        ],
    )?;
    if updated != 1 {
        return Err(PaymentError::LedgerInvariant);
    }
    Ok(())
}

fn now_unix_seconds() -> Result<u64, PaymentError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn u64_to_i64(field: &'static str, value: u64) -> Result<i64, PaymentError> {
    i64::try_from(value).map_err(|_| PaymentError::IntegerOverflow { field, value })
}

#[async_trait]
impl PaymentBlockchainClient for ToncenterClient {
    async fn transaction_by_hash(
        &self,
        transaction_hash: &str,
    ) -> Result<Option<PaymentTransaction>, PaymentError> {
        let response = self
            .toncenter_request("/api/v3/transactions")
            .query(&[("hash", transaction_hash), ("limit", "2")])
            .timeout(PROVIDER_REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(PaymentError::Transport)?;
        let status = response.status();
        let body = response.text().await.map_err(PaymentError::Transport)?;
        if !status.is_success() {
            return Err(PaymentError::Provider {
                status: status.as_u16(),
                body,
            });
        }
        let mut transactions = serde_json::from_str::<TransactionsResponse>(&body)
            .map_err(PaymentError::MalformedProviderResponse)?
            .transactions
            .into_iter()
            .map(PaymentTransaction::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        if transactions.len() > 1 {
            return Err(PaymentError::AmbiguousTransactionHash);
        }
        Ok(transactions.pop())
    }

    async fn transactions(
        &self,
        account: &str,
        limit: usize,
        offset: usize,
        sort: HistorySort,
    ) -> Result<Vec<PaymentTransaction>, PaymentError> {
        let response = self
            .toncenter_request("/api/v3/transactions")
            .query(&[
                ("account", account.to_owned()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
                ("sort", sort.as_str().to_owned()),
            ])
            .timeout(PROVIDER_REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(PaymentError::Transport)?;
        let status = response.status();
        let body = response.text().await.map_err(PaymentError::Transport)?;
        if !status.is_success() {
            return Err(PaymentError::Provider {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str::<TransactionsResponse>(&body)
            .map_err(PaymentError::MalformedProviderResponse)?
            .transactions
            .into_iter()
            .map(PaymentTransaction::try_from)
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct TransactionsResponse {
    transactions: Vec<ToncenterTransaction>,
}

#[derive(Debug, Deserialize)]
struct ToncenterTransaction {
    account: String,
    hash: String,
    lt: String,
    #[serde(default)]
    now: u64,
    #[serde(default)]
    emulated: Option<bool>,
    finality: String,
    description: ToncenterTransactionDescription,
    #[serde(default)]
    in_msg: Option<ToncenterMessage>,
}

#[derive(Debug, Deserialize)]
struct ToncenterTransactionDescription {
    #[serde(default)]
    aborted: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ToncenterMessage {
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    bounced: Option<bool>,
    #[serde(default)]
    message_content: Option<ToncenterMessageContent>,
}

#[derive(Debug, Deserialize)]
struct ToncenterMessageContent {
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    decoded: Option<Value>,
}

impl TryFrom<ToncenterTransaction> for PaymentTransaction {
    type Error = PaymentError;

    fn try_from(transaction: ToncenterTransaction) -> Result<Self, Self::Error> {
        let hash = normalize_hash(&transaction.hash);
        if !is_valid_hash(&hash) {
            return Err(PaymentError::InvalidTransactionHash(transaction.hash));
        }
        Ok(Self {
            account: transaction.account,
            hash,
            lt: transaction
                .lt
                .parse()
                .map_err(|_| PaymentError::InvalidLogicalTime(transaction.lt))?,
            timestamp: transaction.now,
            emulated: transaction.emulated.unwrap_or(true),
            finality: transaction.finality,
            aborted: transaction.description.aborted.unwrap_or(true),
            incoming: transaction
                .in_msg
                .map(PaymentMessage::try_from)
                .transpose()?,
        })
    }
}

impl TryFrom<ToncenterMessage> for PaymentMessage {
    type Error = PaymentError;

    fn try_from(message: ToncenterMessage) -> Result<Self, Self::Error> {
        let value = message
            .value
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| PaymentError::InvalidAmount(value))
            })
            .transpose()?;
        let comment = message.message_content.as_ref().and_then(parse_comment);
        Ok(Self {
            destination: message.destination,
            value,
            bounced: message.bounced.unwrap_or(true),
            comment,
        })
    }
}

fn parse_comment(content: &ToncenterMessageContent) -> Option<String> {
    content
        .decoded
        .as_ref()
        .and_then(|decoded| {
            decoded
                .get("comment")
                .or_else(|| decoded.get("text"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
        .or_else(|| content.body.as_deref().and_then(parse_comment_boc))
}

fn parse_comment_boc(body: &str) -> Option<String> {
    let cell = Boc::decode_base64(body).ok()?;
    let mut slice = cell.as_slice().ok()?;
    (slice.load_u32().ok()? == 0).then_some(())?;
    String::from_utf8(parse_snake_bytes(&mut slice)?).ok()
}

fn parse_snake_bytes(slice: &mut CellSlice<'_>) -> Option<Vec<u8>> {
    let mut result = load_aligned_bytes(slice)?;
    let mut next = match slice.size_refs() {
        0 => return Some(result),
        1 => slice.load_reference_cloned().ok()?,
        _ => return None,
    };

    loop {
        let mut next_slice = next.as_slice().ok()?;
        result.extend(load_aligned_bytes(&mut next_slice)?);
        match next_slice.size_refs() {
            0 => return Some(result),
            1 => next = next_slice.load_reference_cloned().ok()?,
            _ => return None,
        }
    }
}

fn load_aligned_bytes(slice: &mut CellSlice<'_>) -> Option<Vec<u8>> {
    let bit_len = slice.size_bits();
    if !bit_len.is_multiple_of(8) {
        return None;
    }
    let mut bytes = vec![0; usize::from(bit_len / 8)];
    slice.load_raw(&mut bytes, bit_len).ok()?;
    Some(bytes)
}

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("the Acton verifier supports only TON testnet")]
    UnsupportedNetwork,
    #[error("missing required verifier configuration: {0}")]
    MissingConfiguration(&'static str),
    #[error("payment.address must be a raw basechain address in the form 0:<64 hex chars>")]
    InvalidPaymentAddress,
    #[error("payment_recovery_in_progress: payment history is still being recovered")]
    RecoveryInProgress,
    #[error("payment_not_found: transaction was not found on TON testnet")]
    TransactionNotFound,
    #[error("payment_invalid: transaction is not a finalized incoming payment")]
    InvalidTransaction,
    #[error("payment_invalid: transaction has no GRAM amount")]
    MissingAmount,
    #[error("payment_insufficient: expected at least {expected} nanoGRAM, received {actual}")]
    InsufficientAmount { expected: u64, actual: u64 },
    #[error(
        "payment_code_hash_mismatch: transaction comment does not match the requested code hash"
    )]
    CodeHashMismatch,
    #[error("payment_used: transaction has already been used")]
    AlreadyUsed,
    #[error("payment_in_progress: transaction is already being processed")]
    InProgress,
    #[error("payment provider transport error: {0}")]
    Transport(reqwest::Error),
    #[error("payment provider API error: status={status}, body={body}")]
    Provider { status: u16, body: String },
    #[error("payment provider returned malformed JSON: {0}")]
    MalformedProviderResponse(serde_json::Error),
    #[error("payment provider returned more than one transaction for a transaction hash")]
    AmbiguousTransactionHash,
    #[error("payment provider returned invalid logical time: {0}")]
    InvalidLogicalTime(String),
    #[error("payment provider returned invalid transaction hash: {0}")]
    InvalidTransactionHash(String),
    #[error("payment provider returned transaction {actual} for requested hash {expected}")]
    TransactionHashMismatch { expected: String, actual: String },
    #[error("payment provider returned invalid GRAM amount: {0}")]
    InvalidAmount(String),
    #[error("payment history changed before the captured tip was reached")]
    HistoryChangedDuringRecovery,
    #[error("payment history pagination offset overflowed")]
    HistoryOffsetOverflow,
    #[error("payment ledger lock is poisoned")]
    LedgerLock,
    #[error("payment ledger state changed unexpectedly")]
    LedgerInvariant,
    #[error("payment ledger integer overflow for {field}: {value}")]
    IntegerOverflow { field: &'static str, value: u64 },
    #[error("failed to create payment ledger directory {path}: {source}")]
    CreateDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("payment ledger SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("system clock error: {0}")]
    Clock(#[from] SystemTimeError),
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use rusqlite::OptionalExtension;
    use serde_json::json;

    use super::{
        HISTORY_PAGE_SIZE, HistorySort, MAX_PAYMENT_ATTEMPTS, OnchainPaymentVerifier,
        PaymentAttemptOutcome, PaymentBlockchainClient, PaymentError, PaymentLedger,
        PaymentMessage, PaymentTransaction, PaymentVerifier, ToncenterTransaction,
        code_hash_from_comment, payment_comment,
    };

    const CODE_HASH: &str = "af8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c";
    const OTHER_CODE_HASH: &str =
        "bf8f72e22d3dd6eec1f312693c026e4d1751e2dfec9b3f6577e8c8b3a668947c";
    const PAYMENT_ADDRESS: &str =
        "0:1111111111111111111111111111111111111111111111111111111111111111";
    const OTHER_PAYMENT_ADDRESS: &str =
        "0:2222222222222222222222222222222222222222222222222222222222222222";

    struct MockBlockchainClient {
        history: Vec<PaymentTransaction>,
        transactions: Vec<PaymentTransaction>,
        history_error: bool,
        lookup_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl PaymentBlockchainClient for MockBlockchainClient {
        async fn transaction_by_hash(
            &self,
            transaction_hash: &str,
        ) -> Result<Option<PaymentTransaction>, PaymentError> {
            self.lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(self
                .transactions
                .iter()
                .find(|transaction| transaction.hash == transaction_hash)
                .cloned())
        }

        async fn transactions(
            &self,
            account: &str,
            limit: usize,
            offset: usize,
            sort: HistorySort,
        ) -> Result<Vec<PaymentTransaction>, PaymentError> {
            if self.history_error {
                return Err(PaymentError::HistoryChangedDuringRecovery);
            }
            let mut transactions = self
                .history
                .iter()
                .filter(|transaction| transaction.account == account)
                .cloned()
                .collect::<Vec<_>>();
            transactions.sort_by_key(|transaction| transaction.lt);
            if matches!(sort, HistorySort::Descending) {
                transactions.reverse();
            }
            Ok(transactions.into_iter().skip(offset).take(limit).collect())
        }
    }

    struct FixedTransactionClient {
        transaction: PaymentTransaction,
    }

    #[async_trait]
    impl PaymentBlockchainClient for FixedTransactionClient {
        async fn transaction_by_hash(
            &self,
            _transaction_hash: &str,
        ) -> Result<Option<PaymentTransaction>, PaymentError> {
            Ok(Some(self.transaction.clone()))
        }

        async fn transactions(
            &self,
            _account: &str,
            _limit: usize,
            _offset: usize,
            _sort: HistorySort,
        ) -> Result<Vec<PaymentTransaction>, PaymentError> {
            Ok(Vec::new())
        }
    }

    struct ScriptedHistoryClient {
        newest: PaymentTransaction,
        pages: Mutex<VecDeque<Vec<PaymentTransaction>>>,
    }

    #[async_trait]
    impl PaymentBlockchainClient for ScriptedHistoryClient {
        async fn transaction_by_hash(
            &self,
            _transaction_hash: &str,
        ) -> Result<Option<PaymentTransaction>, PaymentError> {
            Ok(None)
        }

        async fn transactions(
            &self,
            _account: &str,
            _limit: usize,
            _offset: usize,
            sort: HistorySort,
        ) -> Result<Vec<PaymentTransaction>, PaymentError> {
            if matches!(sort, HistorySort::Descending) {
                return Ok(vec![self.newest.clone()]);
            }
            Ok(self
                .pages
                .lock()
                .expect("scripted pages mutex should not be poisoned")
                .pop_front()
                .unwrap_or_default())
        }
    }

    fn payment(transaction_hash: &str, code_hash: &str, amount_nano: u64) -> PaymentTransaction {
        PaymentTransaction {
            account: PAYMENT_ADDRESS.to_owned(),
            hash: transaction_hash.to_owned(),
            lt: transaction_hash.len() as u64,
            timestamp: 1_728_000_000,
            emulated: false,
            finality: "finalized".to_owned(),
            aborted: false,
            incoming: Some(PaymentMessage {
                destination: Some(PAYMENT_ADDRESS.to_owned()),
                value: Some(amount_nano),
                bounced: false,
                comment: Some(payment_comment(code_hash)),
            }),
        }
    }

    fn verifier(
        history: Vec<PaymentTransaction>,
        transactions: Vec<PaymentTransaction>,
    ) -> OnchainPaymentVerifier {
        verifier_with_lookup_count(history, transactions).0
    }

    fn verifier_with_lookup_count(
        history: Vec<PaymentTransaction>,
        transactions: Vec<PaymentTransaction>,
    ) -> (OnchainPaymentVerifier, Arc<AtomicUsize>) {
        let lookup_count = Arc::new(AtomicUsize::new(0));
        let verifier = OnchainPaymentVerifier::new(
            Arc::new(MockBlockchainClient {
                history,
                transactions,
                history_error: false,
                lookup_count: Arc::clone(&lookup_count),
            }),
            PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
            PAYMENT_ADDRESS.to_owned(),
            10,
        );
        (verifier, lookup_count)
    }

    fn verifier_with_client(client: Arc<dyn PaymentBlockchainClient>) -> OnchainPaymentVerifier {
        OnchainPaymentVerifier::new(
            client,
            PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
            PAYMENT_ADDRESS.to_owned(),
            10,
        )
    }

    #[test]
    fn payment_comment_round_trips_code_hash() {
        let comment = payment_comment(CODE_HASH);
        assert_eq!(code_hash_from_comment(&comment).as_deref(), Some(CODE_HASH));
    }

    #[test]
    fn payment_comment_rejects_non_canonical_hash() {
        assert_eq!(code_hash_from_comment("acton-verify:v1:not-a-hash"), None);
    }

    #[test]
    fn payment_comment_decodes_from_a_text_comment_boc() {
        let comment = payment_comment(CODE_HASH);
        let mut builder = tycho_types::cell::CellBuilder::new();
        builder
            .store_u32(0)
            .expect("text comment opcode should store");
        builder
            .store_raw(comment.as_bytes(), (comment.len() * 8) as u16)
            .expect("text comment should store");
        let body = builder.build().expect("text comment cell should build");

        assert_eq!(
            super::parse_comment_boc(&tycho_types::boc::Boc::encode_base64(body)),
            Some(comment)
        );
    }

    #[test]
    fn toncenter_transaction_hash_is_normalized_to_hex() {
        let transaction = serde_json::from_value::<ToncenterTransaction>(json!({
            "account": PAYMENT_ADDRESS,
            "hash": "oH2VGnArkQ1fZbcQyozpZnvQ89gDz4SOAfdXRKCNOUs=",
            "lt": "1",
            "now": 1_728_000_000,
            "emulated": false,
            "finality": "finalized",
            "description": {"aborted": false}
        }))
        .expect("Toncenter transaction should deserialize");

        let transaction = PaymentTransaction::try_from(transaction)
            .expect("Toncenter transaction should normalize");
        assert_eq!(
            transaction.hash,
            "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b"
        );
    }

    #[test]
    fn toncenter_safety_flags_are_required() {
        let transaction = json!({
            "account": PAYMENT_ADDRESS,
            "hash": "1111111111111111111111111111111111111111111111111111111111111111",
            "lt": "1",
            "now": 1_728_000_000,
            "emulated": false,
            "finality": "finalized",
            "description": {"aborted": false},
            "in_msg": {
                "destination": PAYMENT_ADDRESS,
                "value": "10",
                "bounced": false,
                "message_content": {
                    "decoded": {"comment": payment_comment(CODE_HASH)}
                }
            }
        });
        let is_valid_payment = |value| {
            let transaction = serde_json::from_value::<ToncenterTransaction>(value)
                .expect("nullable safety flags should deserialize");
            let transaction = PaymentTransaction::try_from(transaction)
                .expect("transaction should convert after deserialization");
            super::valid_incoming_message(&transaction, PAYMENT_ADDRESS).is_some()
        };
        assert!(is_valid_payment(transaction.clone()));

        for field in ["emulated", "aborted", "bounced"] {
            for missing in [true, false] {
                let mut unsafe_transaction = transaction.clone();
                let object = match field {
                    "emulated" => unsafe_transaction
                        .as_object_mut()
                        .expect("transaction should be an object"),
                    "aborted" => unsafe_transaction["description"]
                        .as_object_mut()
                        .expect("description should be an object"),
                    "bounced" => unsafe_transaction["in_msg"]
                        .as_object_mut()
                        .expect("incoming message should be an object"),
                    _ => unreachable!("all safety fields should be covered"),
                };
                if missing {
                    object.remove(field);
                } else {
                    object.insert(field.to_owned(), serde_json::Value::Null);
                }
                assert!(
                    !is_valid_payment(unsafe_transaction),
                    "{field} must fail closed when {}",
                    if missing { "missing" } else { "null" }
                );
            }
        }
    }

    #[tokio::test]
    async fn claim_rejects_a_provider_transaction_with_another_hash() {
        let requested_hash = "1111111111111111111111111111111111111111111111111111111111111111";
        let returned_hash = "2222222222222222222222222222222222222222222222222222222222222222";
        let verifier = verifier_with_client(Arc::new(FixedTransactionClient {
            transaction: payment(returned_hash, CODE_HASH, 10),
        }));
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");

        assert!(matches!(
            verifier.claim(requested_hash, CODE_HASH).await,
            Err(PaymentError::TransactionHashMismatch { expected, actual })
                if expected == requested_hash && actual == returned_hash
        ));
    }

    #[tokio::test]
    async fn recovery_consumes_funded_protocol_payments_and_ignores_dust() {
        let full_payment = payment("full-payment", CODE_HASH, 10);
        let insufficient_payment = payment("insufficient-payment", OTHER_CODE_HASH, 1);
        let verifier = verifier(
            vec![full_payment.clone(), insufficient_payment.clone()],
            vec![full_payment, insufficient_payment],
        );

        verifier
            .recover()
            .await
            .expect("payment history recovery should succeed");

        assert!(verifier.is_ready());
        let recovered_state = {
            let connection = verifier
                .ledger
                .connection()
                .expect("payment ledger should be readable");
            let full_state = connection
                .query_row(
                    "select state from payment_transactions where transaction_hash = ?1",
                    ["full-payment"],
                    |row| row.get::<_, String>(0),
                )
                .expect("funded historical payment should be present in the ledger");
            let dust_state = connection
                .query_row(
                    "select state from payment_transactions where transaction_hash = ?1",
                    ["insufficient-payment"],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .expect("dust payment lookup should succeed");
            drop(connection);
            (full_state, dust_state)
        };
        assert_eq!(recovered_state, ("consumed".to_owned(), None));
        assert!(matches!(
            verifier.claim("full-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
        assert!(matches!(
            verifier
                .claim("insufficient-payment", OTHER_CODE_HASH)
                .await,
            Err(PaymentError::InsufficientAmount { .. })
        ));
    }

    #[tokio::test]
    async fn payment_can_retry_only_after_a_server_failure() {
        let verifier = verifier(Vec::new(), vec![payment("new-payment", CODE_HASH, 10)]);
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");

        let first_claim = verifier
            .claim("new-payment", CODE_HASH)
            .await
            .expect("new payment should be reserved");
        assert!(matches!(
            verifier.claim("new-payment", CODE_HASH).await,
            Err(PaymentError::InProgress)
        ));

        verifier
            .finish(&first_claim, PaymentAttemptOutcome::Retryable)
            .expect("server failure should make the payment retryable");
        let retry_claim = verifier
            .claim("new-payment", CODE_HASH)
            .await
            .expect("retryable payment should be reserved again");
        verifier
            .finish(&retry_claim, PaymentAttemptOutcome::Consumed)
            .expect("deterministic result should consume the payment");

        assert!(matches!(
            verifier.claim("new-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
    }

    #[tokio::test]
    async fn retryable_payment_has_a_bounded_claim_budget() {
        let (verifier, lookup_count) =
            verifier_with_lookup_count(Vec::new(), vec![payment("bounded-payment", CODE_HASH, 10)]);
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");

        for claim_version in 1..=MAX_PAYMENT_ATTEMPTS {
            let claim = verifier
                .claim("bounded-payment", CODE_HASH)
                .await
                .expect("payment attempt within the budget should be reserved");
            assert_eq!(claim.claim_version, claim_version);
            verifier
                .finish(&claim, PaymentAttemptOutcome::Retryable)
                .expect("failed attempt should become retryable");
        }

        assert!(matches!(
            verifier.claim("bounded-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
        assert_eq!(
            lookup_count.load(Ordering::Relaxed),
            usize::try_from(MAX_PAYMENT_ATTEMPTS).expect("attempt limit should fit usize")
        );
    }

    #[tokio::test]
    async fn expired_processing_lease_allows_the_payment_to_retry() {
        let verifier = verifier(Vec::new(), vec![payment("leased-payment", CODE_HASH, 10)]);
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");
        let stale_claim = verifier
            .claim("leased-payment", CODE_HASH)
            .await
            .expect("new payment should be reserved");

        {
            let connection = verifier
                .ledger
                .connection()
                .expect("payment ledger should be writable");
            connection
                .execute(
                    "update payment_transactions set updated_at = 0 where transaction_hash = ?1",
                    ["leased-payment"],
                )
                .expect("processing lease should be made stale");
        }

        let retry = verifier
            .claim("leased-payment", CODE_HASH)
            .await
            .expect("expired processing lease should be reclaimable");
        assert!(matches!(
            verifier.finish(&stale_claim, PaymentAttemptOutcome::Retryable),
            Err(PaymentError::LedgerInvariant)
        ));
        verifier
            .finish(&retry, PaymentAttemptOutcome::Consumed)
            .expect("reclaimed payment should be consumable");
    }

    #[tokio::test]
    async fn local_replay_state_rejects_without_another_provider_lookup() {
        let (verifier, lookup_count) =
            verifier_with_lookup_count(Vec::new(), vec![payment("known-payment", CODE_HASH, 10)]);
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");

        let claim = verifier
            .claim("known-payment", CODE_HASH)
            .await
            .expect("new payment should be reserved");
        assert_eq!(lookup_count.load(Ordering::Relaxed), 1);
        assert!(matches!(
            verifier.claim("known-payment", CODE_HASH).await,
            Err(PaymentError::InProgress)
        ));
        assert_eq!(lookup_count.load(Ordering::Relaxed), 1);

        verifier
            .finish(&claim, PaymentAttemptOutcome::Consumed)
            .expect("payment should be consumed");
        assert!(matches!(
            verifier.claim("known-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
        assert_eq!(lookup_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn empty_recovery_consumes_and_preserves_known_replay_state() {
        let verifier = verifier(
            Vec::new(),
            vec![
                payment("processing-payment", CODE_HASH, 10),
                payment("retryable-payment", CODE_HASH, 10),
            ],
        );
        verifier
            .recover()
            .await
            .expect("initial empty recovery should succeed");
        let processing = verifier
            .claim("processing-payment", CODE_HASH)
            .await
            .expect("processing payment should be reserved");
        let retryable = verifier
            .claim("retryable-payment", CODE_HASH)
            .await
            .expect("retryable payment should be reserved");
        verifier
            .finish(&retryable, PaymentAttemptOutcome::Retryable)
            .expect("payment should become retryable");

        verifier
            .recover()
            .await
            .expect("empty recovery should preserve local replay state");
        assert!(matches!(
            verifier.claim("processing-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
        assert!(matches!(
            verifier.claim("retryable-payment", CODE_HASH).await,
            Err(PaymentError::AlreadyUsed)
        ));
        assert!(matches!(
            verifier.finish(&processing, PaymentAttemptOutcome::Retryable),
            Err(PaymentError::LedgerInvariant)
        ));

        let consumed_count = verifier
            .ledger
            .connection()
            .expect("payment ledger should be readable")
            .query_row(
                "select count(*) from payment_transactions where state = 'consumed'",
                [],
                |row| row.get::<_, usize>(0),
            )
            .expect("consumed count should be readable");
        assert_eq!(consumed_count, 2);
    }

    #[tokio::test]
    async fn payment_comment_must_match_the_requested_code_hash() {
        let verifier = verifier(
            Vec::new(),
            vec![payment("other-code-payment", OTHER_CODE_HASH, 10)],
        );
        verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");

        assert!(matches!(
            verifier.claim("other-code-payment", CODE_HASH).await,
            Err(PaymentError::CodeHashMismatch)
        ));
    }

    #[tokio::test]
    async fn claim_rejects_missing_insufficient_and_non_final_payments() {
        let mut pending = payment("pending-payment", CODE_HASH, 10);
        pending.finality = "unfinalized".to_owned();
        let mut emulated = payment("emulated-payment", CODE_HASH, 10);
        emulated.emulated = true;
        let mut aborted = payment("aborted-payment", CODE_HASH, 10);
        aborted.aborted = true;
        let mut bounced = payment("bounced-payment", CODE_HASH, 10);
        bounced
            .incoming
            .as_mut()
            .expect("payment should have an incoming message")
            .bounced = true;
        let mut wrong_account = payment("wrong-account-payment", CODE_HASH, 10);
        wrong_account.account = OTHER_PAYMENT_ADDRESS.to_owned();
        let mut wrong_destination = payment("wrong-destination-payment", CODE_HASH, 10);
        wrong_destination
            .incoming
            .as_mut()
            .expect("payment should have an incoming message")
            .destination = Some(OTHER_PAYMENT_ADDRESS.to_owned());
        let mut missing_incoming = payment("missing-incoming-payment", CODE_HASH, 10);
        missing_incoming.incoming = None;

        for invalid in [
            pending,
            emulated,
            aborted,
            bounced,
            wrong_account,
            wrong_destination,
            missing_incoming,
        ] {
            let transaction_hash = invalid.hash.clone();
            let verifier = verifier(Vec::new(), vec![invalid]);
            verifier
                .recover()
                .await
                .expect("empty payment history recovery should succeed");
            assert!(matches!(
                verifier.claim(&transaction_hash, CODE_HASH).await,
                Err(PaymentError::InvalidTransaction)
            ));
        }

        let small_payment_verifier =
            verifier(Vec::new(), vec![payment("small-payment", CODE_HASH, 9)]);
        small_payment_verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");
        assert!(matches!(
            small_payment_verifier
                .claim("small-payment", CODE_HASH)
                .await,
            Err(PaymentError::InsufficientAmount {
                expected: 10,
                actual: 9
            })
        ));
        assert!(matches!(
            small_payment_verifier
                .claim("missing-payment", CODE_HASH)
                .await,
            Err(PaymentError::TransactionNotFound)
        ));

        let mut missing_amount = payment("missing-amount-payment", CODE_HASH, 10);
        missing_amount
            .incoming
            .as_mut()
            .expect("payment should have an incoming message")
            .value = None;
        let missing_amount_verifier = verifier(Vec::new(), vec![missing_amount]);
        missing_amount_verifier
            .recover()
            .await
            .expect("empty payment history recovery should succeed");
        assert!(matches!(
            missing_amount_verifier
                .claim("missing-amount-payment", CODE_HASH)
                .await,
            Err(PaymentError::MissingAmount)
        ));
    }

    #[tokio::test]
    async fn recovery_reads_all_history_pages() {
        let history = (0..=HISTORY_PAGE_SIZE)
            .map(|index| {
                let mut transaction = payment(&format!("payment-{index}"), CODE_HASH, 10);
                transaction.lt = index as u64;
                transaction
            })
            .collect::<Vec<_>>();
        let verifier = verifier(history, Vec::new());

        verifier
            .recover()
            .await
            .expect("paginated payment history recovery should succeed");

        let consumed_count = verifier
            .ledger
            .connection()
            .expect("payment ledger should be readable")
            .query_row(
                "select count(*) from payment_transactions where state = 'consumed'",
                [],
                |row| row.get::<_, usize>(0),
            )
            .expect("consumed payment count should be readable");
        assert_eq!(consumed_count, HISTORY_PAGE_SIZE + 1);
    }

    #[tokio::test]
    async fn recovery_rejects_out_of_order_history() {
        let mut first = payment("first", CODE_HASH, 10);
        first.lt = 1;
        let mut second = payment("second", CODE_HASH, 10);
        second.lt = 2;
        let verifier = verifier_with_client(Arc::new(ScriptedHistoryClient {
            newest: second.clone(),
            pages: Mutex::new(VecDeque::from([vec![second, first]])),
        }));

        assert!(matches!(
            verifier.recover().await,
            Err(PaymentError::HistoryChangedDuringRecovery)
        ));
        assert!(!verifier.is_ready());
    }

    #[tokio::test]
    async fn recovery_rejects_a_repeated_full_page_without_progress() {
        let page = (0..HISTORY_PAGE_SIZE)
            .map(|index| {
                let mut transaction = payment(&format!("old-payment-{index}"), CODE_HASH, 10);
                transaction.lt = index as u64;
                transaction
            })
            .collect::<Vec<_>>();
        let mut newest = payment("newest-payment", CODE_HASH, 10);
        newest.lt = HISTORY_PAGE_SIZE as u64;
        let verifier = verifier_with_client(Arc::new(ScriptedHistoryClient {
            newest,
            pages: Mutex::new(VecDeque::from([page.clone(), page])),
        }));

        assert!(matches!(
            verifier.recover().await,
            Err(PaymentError::HistoryChangedDuringRecovery)
        ));
        assert!(!verifier.is_ready());
    }

    #[tokio::test]
    async fn failed_recovery_keeps_payment_verifier_unready() {
        let verifier = OnchainPaymentVerifier::new(
            Arc::new(MockBlockchainClient {
                history: Vec::new(),
                transactions: Vec::new(),
                history_error: true,
                lookup_count: Arc::new(AtomicUsize::new(0)),
            }),
            PaymentLedger::in_memory().expect("in-memory payment ledger should open"),
            PAYMENT_ADDRESS.to_owned(),
            10,
        );

        assert!(matches!(
            verifier.recover().await,
            Err(PaymentError::HistoryChangedDuringRecovery)
        ));
        assert!(!verifier.is_ready());
    }
}
