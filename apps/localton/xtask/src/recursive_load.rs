use std::{
    fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Output,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use tokio::{process::Command, time::sleep};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SPLIT_OVERHEAD_NANO: u128 = 30_000_000;
const MINIMUM_SPLIT_BALANCE_NANO: u128 = 90_000_000;
const MAX_TREE_DEPTH: u32 = 63;

struct WorkloadPaths {
    project: PathBuf,
    acton: PathBuf,
    localton: PathBuf,
}

struct PreparedRoot {
    address: String,
    message: PathBuf,
}

struct WorkloadEstimate {
    messages: u128,
    generations: u32,
}

#[derive(Debug)]
struct Account {
    state: String,
    balance_nano: u128,
}

impl WorkloadPaths {
    fn discover() -> Result<Self> {
        let xtask_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let localton_dir = xtask_dir
            .parent()
            .context("xtask manifest directory has no Localton parent")?;
        let repository = localton_dir
            .parent()
            .and_then(Path::parent)
            .context("Localton directory has no repository parent")?;

        Ok(Self {
            project: localton_dir.join("load-test"),
            acton: repository.join("target/debug/acton"),
            localton: localton_dir.join("target/debug/localton"),
        })
    }
}

/// Builds the deterministic root StateInit and unsigned external message.
///
/// The generated BoC belongs to the selected tree ID and is overwritten on a
/// later preparation. Callers must fund the printed address before submitting
/// the message because TON external-in messages cannot carry value.
pub(crate) async fn prepare(tree_id: u64) -> Result<()> {
    let root = prepare_root(tree_id).await?;
    println!(
        "{}\n  Address: {}\n  Tree ID: {}\n  BoC: {}",
        heading("Root message prepared"),
        root.address,
        tree_id,
        root.message.display()
    );
    Ok(())
}

async fn prepare_root(tree_id: u64) -> Result<PreparedRoot> {
    ensure!(tree_id > 0, "tree ID must be positive");

    let paths = WorkloadPaths::discover()?;
    let output = Command::new(&paths.acton)
        .args(["run", "build-root-message"])
        .env("TREE_ID", tree_id.to_string())
        .current_dir(&paths.project)
        .output()
        .await
        .with_context(|| format!("failed to execute {}", paths.acton.display()))?;
    let stdout = checked_output(output, "build recursive load root")?;

    let address = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Root address: "))
        .filter(|address| !address.is_empty())
        .context("Acton output did not contain a root address")?
        .to_owned();
    let message = paths.project.join("root-message.boc");
    ensure!(
        message.is_file(),
        "Acton did not create {}",
        message.display()
    );

    Ok(PreparedRoot { address, message })
}

/// Funds and deploys one recursive workload root, then waits for activation.
///
/// A rerun is safe only while the deterministic address is still uninitialized:
/// an exact pre-existing balance resumes submission, while an active account or
/// any other balance is rejected to avoid funding the wrong lifecycle state.
pub(crate) async fn run(
    amount: &str,
    tree_id: u64,
    state_dir: &Path,
    timeout_seconds: u64,
) -> Result<()> {
    ensure!(timeout_seconds > 0, "timeout must be positive");
    let expected_balance = parse_grams(amount)?;
    let paths = WorkloadPaths::discover()?;
    let root = prepare_root(tree_id).await?;
    let estimate = estimate_workload(expected_balance);

    println!(
        "{}\n  Root: {}\n  Amount: {} GRAM\n  Tree ID: {}\n  Estimated messages: ~{}\n  Estimated generations: {}",
        heading("Recursive load"),
        root.address,
        amount,
        tree_id,
        format_count(estimate.messages),
        estimate.generations
    );

    let account = account(&paths, &root.address, state_dir).await?;
    ensure!(
        account.state != "active",
        "root {} is already active; choose a new tree ID",
        root.address
    );

    if account.balance_nano == 0 {
        if funding_is_pending(state_dir, tree_id, &root.address, expected_balance)? {
            println!(
                "\n{}\n  Already submitted, waiting for confirmation",
                heading("Funding")
            );
        } else {
            record_pending_funding(state_dir, tree_id, &root.address, expected_balance)?;
            run_localton(
                &paths,
                [
                    "wallet",
                    "send",
                    "--from",
                    "faucet",
                    "--to",
                    &root.address,
                    "--amount",
                    amount,
                    "--no-bounce",
                    "--state-dir",
                    path_arg(state_dir)?,
                ],
                "fund recursive load root",
            )
            .await
            .context(
                "funding submission failed; its pending marker was kept to prevent a duplicate transfer",
            )?;
            println!(
                "\n{}\n  Submitted, waiting for confirmation",
                heading("Funding")
            );
        }
    } else {
        ensure!(
            account.balance_nano == expected_balance,
            "root {} has {} nanoGRAM, expected {} nanoGRAM",
            root.address,
            account.balance_nano,
            expected_balance
        );
    }

    wait_for_account(
        &paths,
        &root.address,
        state_dir,
        Duration::from_secs(timeout_seconds),
        "funding",
        |account| account.state == "uninit" && account.balance_nano == expected_balance,
    )
    .await
    .with_context(|| {
        format!(
            "funding is still pending; rerun `cargo xtask run-recursive-load {amount} {tree_id}` to resume without another transfer"
        )
    })?;
    clear_pending_funding(state_dir, tree_id)?;
    success("Confirmed");

    run_localton(
        &paths,
        [
            "lite",
            "send",
            path_arg(&root.message)?,
            "--state-dir",
            path_arg(state_dir)?,
        ],
        "submit recursive load root",
    )
    .await?;
    println!(
        "\n{}\n  Root message submitted, waiting for activation",
        heading("Deployment")
    );

    let account = wait_for_account(
        &paths,
        &root.address,
        state_dir,
        Duration::from_secs(timeout_seconds),
        "deployment",
        |account| account.state == "active",
    )
    .await?;

    success("Root active");
    println!(
        "  State: {}\n  Remaining root balance: {} nanoGRAM",
        account.state, account.balance_nano
    );
    Ok(())
}

async fn wait_for_account(
    paths: &WorkloadPaths,
    address: &str,
    state_dir: &Path,
    timeout: Duration,
    phase: &str,
    ready: impl Fn(&Account) -> bool,
) -> Result<Account> {
    let started = Instant::now();
    loop {
        let account = account(paths, address, state_dir).await?;
        if ready(&account) {
            return Ok(account);
        }
        if started.elapsed() >= timeout {
            bail!(
                "timed out during {phase} for {address}: state={}, balance_nano={}",
                account.state,
                account.balance_nano
            );
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn account(paths: &WorkloadPaths, address: &str, state_dir: &Path) -> Result<Account> {
    let stdout = run_localton(
        paths,
        [
            "lite",
            "account",
            address,
            "--state-dir",
            path_arg(state_dir)?,
        ],
        "query recursive load account",
    )
    .await?;
    let json: Value =
        serde_json::from_str(&stdout).context("invalid account JSON from Localton")?;
    let state = json["state"]
        .as_str()
        .context("Localton account JSON has no string state")?
        .to_owned();
    let balance_nano = parse_balance_nano(&json["balance_nano"])?;
    Ok(Account {
        state,
        balance_nano,
    })
}

async fn run_localton<'a>(
    paths: &WorkloadPaths,
    args: impl IntoIterator<Item = &'a str>,
    operation: &str,
) -> Result<String> {
    let output = Command::new(&paths.localton)
        .args(args)
        // The account subcommand's stdout is a machine-readable protocol for
        // this xtask. Human-oriented tracing and ANSI styling would corrupt it.
        .env("RUST_LOG", "error")
        .env("NO_COLOR", "1")
        .output()
        .await
        .with_context(|| format!("failed to execute {}", paths.localton.display()))?;
    checked_output(output, operation)
}

fn checked_output(output: Output, operation: &str) -> Result<String> {
    let stdout = String::from_utf8(output.stdout).context("command stdout is not UTF-8")?;
    if output.status.success() {
        return Ok(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "{operation} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout.trim(),
        stderr.trim()
    )
}

fn path_arg(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))
}

/// Returns whether this exact funding operation was already submitted.
///
/// The marker makes retries conservative across timeouts and process exits. A
/// mismatched address or amount is rejected because reusing a tree ID for a
/// different transfer would otherwise risk silently losing or duplicating funds.
fn funding_is_pending(
    state_dir: &Path,
    tree_id: u64,
    address: &str,
    amount_nano: u128,
) -> Result<bool> {
    let path = pending_funding_path(state_dir, tree_id);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let pending: Value = serde_json::from_str(&contents)
        .with_context(|| format!("invalid pending funding marker {}", path.display()))?;
    let pending_address = pending["address"]
        .as_str()
        .context("pending funding marker has no address")?;
    let pending_amount = parse_balance_nano(&pending["amount_nano"])?;
    ensure!(
        pending_address == address && pending_amount == amount_nano,
        "tree ID {tree_id} has different pending funding: address={pending_address}, amount_nano={pending_amount}"
    );
    Ok(true)
}

/// Persists intent before submitting a wallet message so a crash cannot cause
/// an automatic duplicate on the next invocation.
fn record_pending_funding(
    state_dir: &Path,
    tree_id: u64,
    address: &str,
    amount_nano: u128,
) -> Result<()> {
    let path = pending_funding_path(state_dir, tree_id);
    let parent = path
        .parent()
        .context("pending funding path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let temporary = path.with_extension("json.tmp");
    let marker = serde_json::json!({
        "address": address,
        "amount_nano": amount_nano.to_string(),
    });
    fs::write(&temporary, serde_json::to_vec_pretty(&marker)?)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .with_context(|| format!("failed to commit {}", path.display()))?;
    Ok(())
}

fn clear_pending_funding(state_dir: &Path, tree_id: u64) -> Result<()> {
    let path = pending_funding_path(state_dir, tree_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn pending_funding_path(state_dir: &Path, tree_id: u64) -> PathBuf {
    state_dir
        .join("tools/recursive-load")
        .join(format!("tree-{tree_id}.json"))
}

fn parse_balance_nano(value: &Value) -> Result<u128> {
    if let Some(balance) = value.as_str() {
        return balance
            .parse::<u128>()
            .context("Localton account JSON has an invalid string balance_nano");
    }
    value
        .as_u64()
        .map(u128::from)
        .context("Localton account JSON has no unsigned balance_nano")
}

/// Estimates how many root and child messages the recursive contract will process.
///
/// The calculation mirrors the contract's balance split and node-ID depth cap.
/// Forwarding fees can make the actual tree slightly smaller, so the value is
/// intentionally presented as an estimate rather than an exact transaction count.
fn estimate_workload(mut balance_nano: u128) -> WorkloadEstimate {
    let mut messages = 1_u128;
    let mut nodes_at_depth = 1_u128;
    let mut depth = 0_u32;

    while balance_nano > MINIMUM_SPLIT_BALANCE_NANO && depth < MAX_TREE_DEPTH {
        balance_nano = (balance_nano - SPLIT_OVERHEAD_NANO) / 2;
        nodes_at_depth *= 2;
        messages += nodes_at_depth;
        depth += 1;
    }
    WorkloadEstimate {
        messages,
        generations: depth + 1,
    }
}

fn format_count(count: u128) -> String {
    let digits = count.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(' ');
        }
        formatted.push(digit);
    }
    formatted
}

fn heading(text: &str) -> String {
    styled(text, "1;36")
}

fn success(message: &str) {
    println!("  {} {message}", styled("✓", "1;32"));
}

fn styled(text: &str, ansi_code: &str) -> String {
    if io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{ansi_code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn parse_grams(amount: &str) -> Result<u128> {
    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    ensure!(
        !whole.is_empty(),
        "GRAM amount must contain a whole-number part"
    );
    ensure!(
        fraction.len() <= 9,
        "GRAM amount must have at most nine decimal places"
    );
    ensure!(
        whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit()),
        "GRAM amount must be a non-negative decimal number"
    );

    let whole = whole.parse::<u128>().context("GRAM amount is too large")?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        let value = fraction
            .parse::<u128>()
            .context("GRAM fractional amount is too large")?;
        value * 10_u128.pow(9 - fraction.len() as u32)
    };
    let nano = whole
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(fraction))
        .context("GRAM amount is too large")?;
    ensure!(nano > 0, "GRAM amount must be positive");
    Ok(nano)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        clear_pending_funding, estimate_workload, format_count, funding_is_pending,
        parse_balance_nano, parse_grams, record_pending_funding,
    };

    #[test]
    fn estimates_recursive_messages_from_attached_balance() {
        let leaf = estimate_workload(80_000_000);
        assert_eq!((leaf.messages, leaf.generations), (1, 1));
        assert_eq!(estimate_workload(150_000_000).messages, 3);
        assert_eq!(estimate_workload(500_000_000).messages, 15);
        assert_eq!(estimate_workload(100_000_000_000).messages, 2_047);

        let large = estimate_workload(5_000_000_000_000);
        assert_eq!((large.messages, large.generations), (131_071, 17));
        assert_eq!(format_count(131_071), "131 071");
    }

    #[test]
    fn pending_funding_prevents_duplicate_submission() {
        let state_dir = tempdir().unwrap();
        let address = "kQExample";
        let amount = 5_000_000_000_000;

        assert!(!funding_is_pending(state_dir.path(), 4, address, amount).unwrap());
        record_pending_funding(state_dir.path(), 4, address, amount).unwrap();
        assert!(funding_is_pending(state_dir.path(), 4, address, amount).unwrap());
        assert!(funding_is_pending(state_dir.path(), 4, address, amount + 1).is_err());
        clear_pending_funding(state_dir.path(), 4).unwrap();
        assert!(!funding_is_pending(state_dir.path(), 4, address, amount).unwrap());
    }

    #[test]
    fn parses_localton_balance_from_canonical_string_or_json_number() {
        assert_eq!(
            parse_balance_nano(&json!("100000000000")).unwrap(),
            100_000_000_000
        );
        assert_eq!(parse_balance_nano(&json!(42)).unwrap(), 42);
        assert!(parse_balance_nano(&json!("not-a-balance")).is_err());
    }

    #[test]
    fn parses_exact_gram_amounts_without_floating_point_rounding() {
        assert_eq!(parse_grams("10000").unwrap(), 10_000_000_000_000);
        assert_eq!(parse_grams("1.2").unwrap(), 1_200_000_000);
        assert_eq!(parse_grams("0.000000001").unwrap(), 1);
    }

    #[test]
    fn rejects_ambiguous_or_non_positive_gram_amounts() {
        assert!(parse_grams("0").is_err());
        assert!(parse_grams(".5").is_err());
        assert!(parse_grams("1.0000000001").is_err());
        assert!(parse_grams("-1").is_err());
    }
}
