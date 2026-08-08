//! Creation and decoding of ADNL, validator, console, and liteserver keys.
//!
//! TON binaries store private and public key files while referring to keys by
//! a canonical 256-bit identifier. This module validates generated output and
//! converts the public-key files into the base64 representation used by TON
//! JSON configuration.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use tokio::process::Command;

use crate::{binaries::TonBinaries, runtime::run_checked};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(super) struct GeneratedKey {
    pub(super) id_hex: String,
    pub(super) id_base64: String,
    pub(super) private_path: PathBuf,
    pub(super) public_path: PathBuf,
}

/// Generates one TON keypair and validates every artifact used by later steps.
///
/// `generate-random-id` prints the canonical key ID and base64 public identity
/// while writing private/public files. The launcher validates the textual output
/// and both files immediately, so a partial key generation cannot surface later
/// as an opaque DHT, console, or validator-engine error.
pub(super) async fn generate_key(binaries: &TonBinaries, path: &Path) -> Result<GeneratedKey> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut command = Command::new(binaries.command("generate-random-id"));
    command.args(["-m", "keys", "-n"]).arg(path);
    let output = run_checked("generate-random-id", command, COMMAND_TIMEOUT).await?;
    let fields: Vec<&str> = output.stdout.split_whitespace().collect();
    ensure!(
        fields.len() >= 2,
        "generate-random-id returned unexpected output: {}",
        output.stdout.trim()
    );
    let id_hex = canonical_key_id(fields[0])?;
    BASE64
        .decode(fields[1])
        .context("generate-random-id returned invalid base64")?;
    let public_path = path.with_extension(
        path.extension()
            .map(|extension| format!("{}.pub", extension.to_string_lossy()))
            .unwrap_or_else(|| "pub".to_owned()),
    );
    ensure!(
        path.is_file(),
        "private key was not created: {}",
        path.display()
    );
    ensure!(
        public_path.is_file(),
        "public key was not created: {}",
        public_path.display()
    );
    Ok(GeneratedKey {
        id_hex,
        id_base64: fields[1].to_owned(),
        private_path: path.to_owned(),
        public_path,
    })
}

/// Converts a persisted TON public-key file to the ID used in JSON config.
///
/// The file is 36 bytes: a four-byte TL constructor prefix followed by the
/// 32-byte public key. TON configuration stores only that payload in base64.
pub(super) fn read_key_id_base64(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        bytes.len() == 36,
        "{} must contain a 36-byte public key",
        path.display()
    );
    Ok(BASE64.encode(&bytes[4..]))
}

/// Normalizes a validated 256-bit key ID to validator keyring filename form.
fn canonical_key_id(value: &str) -> Result<String> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "generate-random-id returned an invalid key id"
    );
    Ok(value.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_ids_match_linux_keyring_names() {
        let lower = "abcdef0123456789".repeat(4);
        assert_eq!(
            canonical_key_id(&lower).unwrap(),
            "ABCDEF0123456789".repeat(4)
        );
        assert!(canonical_key_id("not-a-key").is_err());
    }
}
