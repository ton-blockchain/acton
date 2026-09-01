//! Typed interface to the official `generate-random-id` program.
//!
//! Localton uses this executable for two different TON operations: generating a
//! durable Ed25519 identity and signing a DHT node descriptor. Keeping both behind
//! semantic methods prevents workflows from depending on release-specific modes,
//! positional arguments, noisy stdout, or TON's numeric IPv4 JSON convention.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::{Instrument, Span, field::Empty, info, info_span, warn};

use crate::{binaries::TonBinaries, runtime::run_checked, storage::write_json_atomic};

use super::types::{
    AdnlAddressList, AdnlEndpoint, DhtNodeDescriptor, GeneratedKey, KeyId, OperationContext,
    TonPublicKey,
};

/// Semantic role of an identity created by `generate-random-id`.
///
/// Official file stems are kept here so bootstrap workflows cannot accidentally
/// use different names for the same role on genesis and additional nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityRole {
    /// Permanent key embedded in the initial validator set.
    Validator,
    /// Validator-engine's authenticated administrative server identity.
    ControlServer,
    /// Host-local client identity allowed to administer validator-engine.
    ControlClient,
    /// Public liteserver identity advertised in the global configuration.
    Liteserver,
}

impl IdentityRole {
    /// Returns the stable artifact stem expected by Localton's persisted layout.
    const fn file_stem(self) -> &'static str {
        match self {
            Self::Validator => "validator",
            Self::ControlServer => "server",
            Self::ControlClient => "client",
            Self::Liteserver => "liteserver",
        }
    }
}

impl fmt::Display for IdentityRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_stem())
    }
}

/// Typed placement policy for one durable TON identity.
///
/// Callers select a role and storage boundary, not an executable filename. For
/// identities consumed by validator-engine, the adapter also installs the private
/// key under its canonical uppercase key ID inside the supplied keyring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerateKeyRequest {
    directory: PathBuf,
    role: IdentityRole,
    engine_keyring: Option<PathBuf>,
}

impl GenerateKeyRequest {
    /// Places the genesis validator identity in validator-engine's keyring.
    pub fn validator(keyring: impl Into<PathBuf>) -> Self {
        let keyring = keyring.into();
        Self::new(keyring.clone(), IdentityRole::Validator, Some(keyring))
    }

    /// Places the control server certificate in `certs` and installs it for engine use.
    pub fn control_server(certs: impl Into<PathBuf>, keyring: impl Into<PathBuf>) -> Self {
        Self::new(
            certs.into(),
            IdentityRole::ControlServer,
            Some(keyring.into()),
        )
    }

    /// Places the host-local control client certificate in `certs` only.
    pub fn control_client(certs: impl Into<PathBuf>) -> Self {
        Self::new(certs.into(), IdentityRole::ControlClient, None)
    }

    /// Places and installs the identity used by the public liteserver endpoint.
    pub fn liteserver(keyring: impl Into<PathBuf>) -> Self {
        let keyring = keyring.into();
        Self::new(keyring.clone(), IdentityRole::Liteserver, Some(keyring))
    }

    fn new(directory: PathBuf, role: IdentityRole, engine_keyring: Option<PathBuf>) -> Self {
        Self {
            directory,
            role,
            engine_keyring,
        }
    }

    fn private_path(&self) -> PathBuf {
        self.directory.join(self.role.file_stem())
    }
}

/// Inputs for signing one bootstrap DHT node descriptor.
///
/// The address-list file is an explicit artifact because the official program
/// consumes JSON from disk. The adapter rewrites it atomically for the requested
/// endpoint before every call, avoiding stale advertised addresses after a retry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtDescriptorRequest {
    /// Private DHT identity created inside the `dht-server` keyring.
    pub private_key: PathBuf,
    /// Public IPv4/UDP endpoint peers must use for ADNL discovery.
    pub address: AdnlEndpoint,
    /// Scratch JSON file passed through `generate-random-id -f`.
    pub address_list_path: PathBuf,
}

/// Semantic operations Localton requires from `generate-random-id`.
///
/// Implementations return validated TON values and files rather than stdout. This
/// also permits deterministic recording implementations in workflow tests without
/// teaching them the official executable's CLI syntax.
#[async_trait]
pub trait RandomIdGenerator: Send + Sync {
    /// Creates and validates one keypair used by ADNL, liteserver, or validators.
    async fn generate_key(
        &self,
        context: &OperationContext,
        request: GenerateKeyRequest,
    ) -> Result<GeneratedKey>;

    /// Publishes an endpoint as a signed TON `dht.node` descriptor.
    ///
    /// The operation never returns raw descriptor stdout and never logs the full
    /// signed payload, because callers need only the typed value for global config.
    async fn create_dht_descriptor(
        &self,
        context: &OperationContext,
        request: DhtDescriptorRequest,
    ) -> Result<DhtNodeDescriptor>;
}

/// Production adapter backed by the pinned official TON distribution.
///
/// It is deliberately the only place that knows `-m keys`, `-m dht`, output field
/// ordering, and the address-list file contract for the pinned release.
#[derive(Clone, Debug)]
pub struct OfficialRandomIdGenerator {
    binaries: TonBinaries,
}

impl OfficialRandomIdGenerator {
    /// Binds the adapter to an already validated official TON distribution.
    pub fn new(binaries: TonBinaries) -> Self {
        Self { binaries }
    }

    fn generate_key_command(&self, request: &GenerateKeyRequest) -> Command {
        let mut command = Command::new(self.binaries.command("generate-random-id"));
        command
            .args(["-m", "keys", "-n"])
            .arg(request.private_path());
        command
    }

    fn dht_descriptor_command(&self, request: &DhtDescriptorRequest) -> Command {
        let mut command = Command::new(self.binaries.command("generate-random-id"));
        command
            .args(["-m", "dht", "-k"])
            .arg(&request.private_key)
            .arg("-f")
            .arg(&request.address_list_path);
        command
    }
}

#[async_trait]
impl RandomIdGenerator for OfficialRandomIdGenerator {
    async fn generate_key(
        &self,
        context: &OperationContext,
        request: GenerateKeyRequest,
    ) -> Result<GeneratedKey> {
        let started = Instant::now();
        let span = operation_span(context, "generate_key");
        let result = async {
            let private_path = request.private_path();
            fs::create_dir_all(&request.directory).with_context(|| {
                format!(
                    "failed to create {} identity directory {}",
                    request.role,
                    request.directory.display()
                )
            })?;
            info!(
                milestone = "key_generation_started",
                identity.role = %request.role,
                private_key_path = %private_path.display(),
                "generating TON identity"
            );
            let output = run_checked(
                "generate-random-id key generation",
                self.generate_key_command(&request),
                context.timeout,
            )
            .await?;
            let generated = parse_generated_key(&output.stdout, &private_path)?;
            if let Some(keyring) = &request.engine_keyring {
                fs::create_dir_all(keyring).with_context(|| {
                    format!("failed to create validator keyring {}", keyring.display())
                })?;
                let installed_path = keyring.join(generated.id.to_keyring_filename());
                fs::copy(&generated.private_path, &installed_path).with_context(|| {
                    format!(
                        "failed to install {} identity in {}",
                        request.role,
                        installed_path.display()
                    )
                })?;
                info!(
                    milestone = "key_installed",
                    identity.role = %request.role,
                    key_id = %generated.id,
                    "installed TON identity for validator-engine"
                );
            }
            info!(
                milestone = "key_artifacts_validated",
                identity.role = %request.role,
                private_key_path = %generated.private_path.display(),
                public_key_path = %generated.public_path.display(),
                key_id = %generated.id,
                "TON identity is ready"
            );
            Ok(generated)
        }
        .instrument(span.clone())
        .await;
        finish_operation(&span, started, &result);
        result
    }

    async fn create_dht_descriptor(
        &self,
        context: &OperationContext,
        request: DhtDescriptorRequest,
    ) -> Result<DhtNodeDescriptor> {
        let started = Instant::now();
        let span = operation_span(context, "create_dht_descriptor");
        let result = async {
            ensure!(
                request.private_key.is_file(),
                "DHT private key does not exist: {}",
                request.private_key.display()
            );
            let address_list = AdnlAddressList::single(request.address);
            write_json_atomic(&request.address_list_path, &address_list)?;
            info!(
                milestone = "address_list_written",
                endpoint = %request.address,
                address_list_path = %request.address_list_path.display(),
                "prepared TON ADNL address list"
            );
            let output = run_checked(
                "DHT node descriptor generation",
                self.dht_descriptor_command(&request),
                context.timeout,
            )
            .await?;
            let descriptor =
                DhtNodeDescriptor::from_json_str(extract_json_object(&output.stdout)?)?;
            info!(
                milestone = "descriptor_validated",
                endpoint = %request.address,
                private_key_path = %request.private_key.display(),
                "signed TON DHT descriptor is ready"
            );
            Ok(descriptor)
        }
        .instrument(span.clone())
        .await;
        finish_operation(&span, started, &result);
        result
    }
}

/// Converts a generator invocation into a complete, validated artifact set.
///
/// The executable emits one key ID in hexadecimal and base64 while writing the
/// private key and TL-encoded public key as side effects. Validating both printed
/// representations and deriving the same ID from `.pub` prevents a partial or
/// mismatched generation from surfacing later as an opaque ADNL error.
fn parse_generated_key(stdout: &str, private_path: &Path) -> Result<GeneratedKey> {
    let fields: Vec<&str> = stdout.split_whitespace().collect();
    ensure!(
        fields.len() >= 2,
        "generate-random-id returned unexpected output: {}",
        stdout.trim()
    );
    let id = KeyId::from_hex(fields[0]).context("generate-random-id returned invalid key id")?;
    let base64_id = KeyId::from_base64(fields[1])
        .context("generate-random-id returned invalid base64 key id")?;
    ensure!(
        base64_id == id,
        "generate-random-id returned different hexadecimal and base64 key ids"
    );
    let public_path = public_key_path(private_path);
    ensure!(
        private_path.is_file(),
        "private key was not created: {}",
        private_path.display()
    );
    ensure!(
        public_path.is_file(),
        "public key was not created: {}",
        public_path.display()
    );
    let public_file = fs::read(&public_path)
        .with_context(|| format!("failed to read generated key {}", public_path.display()))?;
    let public_key = TonPublicKey::from_tl_bytes(&public_file)
        .context("generated public key file is not a valid TON public key")?;
    ensure!(
        public_key.key_id() == id,
        "generated public key file does not match the reported key id"
    );
    Ok(GeneratedKey {
        id,
        public_key,
        private_path: private_path.to_owned(),
        public_path,
    })
}

/// Extracts one JSON object when the official binary surrounds it with log lines.
///
/// TON builds have printed diagnostics before or after successful JSON on stdout.
/// Taking the outermost braces preserves the current compatibility behavior while
/// still requiring the enclosed payload to be one valid JSON value.
fn extract_json_object(output: &str) -> Result<&str> {
    let trimmed = output.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed);
    }
    let start = output
        .find('{')
        .context("command output contains no JSON object")?;
    let end = output
        .rfind('}')
        .context("command output contains no complete JSON object")?;
    Ok(&output[start..=end])
}

fn public_key_path(private_path: &Path) -> PathBuf {
    private_path.with_extension(
        private_path
            .extension()
            .map(|extension| format!("{}.pub", extension.to_string_lossy()))
            .unwrap_or_else(|| "pub".to_owned()),
    )
}

/// Starts the structured envelope shared by every semantic generator operation.
///
/// Paths and public identifiers are emitted as child events, while the span keeps
/// stable low-cardinality fields suitable for aggregating tool latency and errors.
fn operation_span(context: &OperationContext, operation: &'static str) -> Span {
    info_span!(
        "ton_tool_operation",
        ton.tool = "generate-random-id",
        operation,
        node = context.node_name.as_deref().unwrap_or("network"),
        outcome = Empty,
        duration_ms = Empty,
    )
}

/// Records the final outcome even when parsing or artifact validation fails after
/// the subprocess itself has exited successfully.
fn finish_operation<T>(span: &Span, started: Instant, result: &Result<T>) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    span.in_scope(|| match result {
        Ok(_) => info!(duration_ms, outcome, "TON tool operation completed"),
        Err(error) => warn!(duration_ms, outcome, %error, "TON tool operation failed"),
    });
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, net::Ipv4Addr};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn parses_generated_key_and_validates_side_effects() {
        let directory = tempdir().unwrap();
        let private_path = directory.path().join("validator.key");
        let public_path = directory.path().join("validator.key.pub");
        fs::write(&private_path, b"private").unwrap();
        let public_key = TonPublicKey::from_bytes([7_u8; 32]);
        fs::write(&public_path, public_key.to_tl_bytes()).unwrap();
        let id = public_key.key_id();

        let generated = parse_generated_key(
            &format!("{} {}\n", id.to_hex(), id.to_base64()),
            &private_path,
        )
        .unwrap();

        assert_eq!(generated.id, id);
        assert_eq!(generated.public_key, public_key);
        assert_eq!(generated.public_path, public_path);
    }

    #[test]
    fn extracts_descriptor_from_noisy_output() {
        let value = extract_json_object("log line\n{\"@type\":\"dht.node\"}\n").unwrap();

        assert_eq!(value, "{\"@type\":\"dht.node\"}");
    }

    #[test]
    fn renders_ton_numeric_ipv4_address_list() {
        let value =
            AdnlAddressList::single(AdnlEndpoint::new(Ipv4Addr::new(192, 168, 27, 4), 18_003));

        assert_eq!(
            serde_json::to_string_pretty(&value).unwrap(),
            r#"{
  "@type": "adnl.addressList",
  "addrs": [
    {
      "@type": "adnl.address.udp",
      "ip": -1062724860,
      "port": 18003
    }
  ],
  "version": 0,
  "reinit_date": 0,
  "priority": 0,
  "expire_at": 0
}"#
        );
    }

    #[test]
    fn renders_official_key_generation_arguments() {
        let adapter = OfficialRandomIdGenerator::new(TonBinaries {
            root: PathBuf::from("/ton"),
        });
        let command = adapter.generate_key_command(&GenerateKeyRequest::validator("/state"));
        let args: Vec<_> = command.as_std().get_args().collect();

        assert_eq!(
            args,
            [
                OsStr::new("-m"),
                OsStr::new("keys"),
                OsStr::new("-n"),
                OsStr::new("/state/validator"),
            ]
        );
    }

    #[test]
    fn derives_identity_paths_and_keyring_installation_from_roles() {
        let validator = GenerateKeyRequest::validator("/state/keyring");
        assert_eq!(
            validator.private_path(),
            PathBuf::from("/state/keyring/validator")
        );
        assert_eq!(
            validator.engine_keyring,
            Some(PathBuf::from("/state/keyring"))
        );

        let client = GenerateKeyRequest::control_client("/state/certs");
        assert_eq!(client.private_path(), PathBuf::from("/state/certs/client"));
        assert_eq!(client.engine_keyring, None);
    }
}
