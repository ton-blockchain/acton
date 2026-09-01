//! Shared values exchanged by typed adapters for the official TON programs.
//!
//! These types keep protocol identities and initialized artifacts distinct from
//! arbitrary strings and paths. They deliberately stay small: Localton workflows
//! still own layout and ordering, while adapters own release-specific command
//! syntax and output parsing.

use std::{
    fmt, io,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

const ED25519_PUBLIC_KEY_TL_CONSTRUCTOR: [u8; 4] = [0xc6, 0xb4, 0x13, 0x48];

/// Execution policy and diagnostic identity shared by one semantic tool call.
///
/// A workflow chooses the deadline because it knows whether an operation belongs
/// to initial genesis creation or a routine node startup. The optional node name
/// exists for structured tracing only; adapters must not use it to discover state
/// or change their behavior.
#[derive(Clone, Debug)]
pub struct OperationContext {
    /// Maximum wall-clock time allowed for one bounded tool operation.
    ///
    /// Each adapter owns this deadline exactly once. Callers compose semantic
    /// operations and must not wrap the same request in another timeout.
    pub timeout: Duration,
    /// Human-readable Localton node associated with the operation, when any.
    pub node_name: Option<String>,
}

/// A canonical 256-bit hash used in TON block and zerostate identities.
///
/// The bytes stay decoded inside Localton. Base64 is used only by TON JSON,
/// hexadecimal text only by operator output, and uppercase hexadecimal only by
/// validator-engine's content-addressed static-state directory.
#[derive(Clone, Copy, Eq, Hash, PartialEq, ToSchema)]
#[schema(value_type = String)]
pub struct TonBlockHash([u8; 32]);

impl TonBlockHash {
    /// Wraps a hash that was already validated by a typed protocol boundary.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrows the protocol bytes without choosing a transport encoding.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Formats the hash for diagnostics and human-facing identifiers.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Formats a file hash as validator-engine's static-state filename.
    ///
    /// The engine looks up initial BoCs by uppercase file hash before its block
    /// database exists, so this casing is a filesystem protocol requirement.
    pub fn to_static_state_filename(self) -> String {
        hex::encode_upper(self.0)
    }

    fn from_slice(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("TON block hash must contain exactly 32 bytes"))?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for TonBlockHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TonBlockHash")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for TonBlockHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl Serialize for TonBlockHash {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(self.0))
    }
}

impl<'de> Deserialize<'de> for TonBlockHash {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let bytes = BASE64.decode(value).map_err(D::Error::custom)?;
        Self::from_slice(&bytes).map_err(D::Error::custom)
    }
}

/// The two hashes that jointly identify one TON zerostate.
///
/// A representation hash identifies the root cell, while a file hash identifies
/// its exact serialized BoC. Keeping the pair indivisible prevents global config
/// and static storage from accidentally referring to different state files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZeroStateId {
    root_hash: TonBlockHash,
    file_hash: TonBlockHash,
}

impl ZeroStateId {
    /// Creates an identity from hashes validated at the artifact boundary.
    pub const fn new(root_hash: TonBlockHash, file_hash: TonBlockHash) -> Self {
        Self {
            root_hash,
            file_hash,
        }
    }

    /// Returns the root-cell representation hash published in global config.
    pub const fn root_hash(self) -> TonBlockHash {
        self.root_hash
    }

    /// Returns the serialized BoC hash used by global config and static storage.
    pub const fn file_hash(self) -> TonBlockHash {
        self.file_hash
    }
}

impl OperationContext {
    /// Creates an execution context for a network-wide operation.
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            node_name: None,
        }
    }

    /// Attaches a node identity used by tracing and failure diagnostics.
    ///
    /// This method does not make the tool adapter node-aware; filesystem paths and
    /// TON endpoints remain explicit request fields.
    pub fn for_node(timeout: Duration, node_name: impl Into<String>) -> Self {
        Self {
            timeout,
            node_name: Some(node_name.into()),
        }
    }
}

/// Public IPv4/UDP address advertised for a TON ADNL identity.
///
/// TON global configuration serializes IPv4 as one 32-bit integer, but workflows
/// use the native address type so adapters cannot accidentally pass dotted text to
/// a JSON field or a numeric value to a CLI endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdnlEndpoint {
    /// IPv4 address visible to other TON nodes.
    pub ip: Ipv4Addr,
    /// UDP port accepting ADNL packets.
    pub port: u16,
}

impl AdnlEndpoint {
    /// Creates an ADNL endpoint without performing network reachability checks.
    pub const fn new(ip: Ipv4Addr, port: u16) -> Self {
        Self { ip, port }
    }

    /// Proves that a new TON process can bind this host-local UDP port.
    ///
    /// Abrupt instance termination can leave `validator-engine` alive after the
    /// state-directory lock is released. Detecting the occupied ADNL socket before
    /// spawn avoids an opaque console timeout and identifies the usual stale-child
    /// failure directly. The advertised address can belong to a NAT gateway or UDP
    /// relay, so only the port is host-local and the probe must bind the wildcard
    /// address. The socket is released immediately; the official process remains the
    /// sole long-term owner and a spawn race is still reported by it.
    pub fn ensure_available(self, service: &str) -> Result<()> {
        match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, self.port)) {
            Ok(socket) => {
                drop(socket);
                Ok(())
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => bail!(
                "{service} cannot bind ADNL endpoint {self}: address is already in use; \
                 a TON process from a previous Localton run may still be running"
            ),
            Err(error) => {
                Err(error).with_context(|| format!("{service} cannot bind ADNL endpoint {self}"))
            }
        }
    }
}

impl fmt::Display for AdnlEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.ip, self.port)
    }
}

/// Returns whether an IPv4 address can be advertised to peers on a public TON network.
///
/// Besides the standard private and special-use ranges, carrier-grade NAT and
/// benchmarking networks are excluded because neither can accept unsolicited ADNL
/// traffic from Internet peers.
pub(crate) fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let shared_address_space = octets[0] == 100 && (64..=127).contains(&octets[1]);
    let benchmarking = octets[0] == 198 && matches!(octets[1], 18 | 19);

    !ip.is_unspecified()
        && !ip.is_loopback()
        && !ip.is_private()
        && !ip.is_link_local()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && !ip.is_documentation()
        && !shared_address_space
        && !benchmarking
}

/// JSON address list accepted by official TON ADNL tools.
///
/// All timing and priority fields are explicit because omitting one changes the
/// TL object that is signed into a DHT descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdnlAddressList {
    #[serde(rename = "@type")]
    constructor: AdnlAddressListConstructor,
    addrs: Vec<AdnlUdpAddress>,
    version: i32,
    reinit_date: i32,
    priority: i32,
    expire_at: i32,
}

impl AdnlAddressList {
    /// Creates the non-expiring single-endpoint list used by Localton nodes.
    pub fn single(endpoint: AdnlEndpoint) -> Self {
        Self {
            constructor: AdnlAddressListConstructor::AddressList,
            addrs: vec![AdnlUdpAddress::new(endpoint)],
            version: 0,
            reinit_date: 0,
            priority: 0,
            expire_at: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum AdnlAddressListConstructor {
    #[serde(rename = "adnl.addressList")]
    AddressList,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct AdnlUdpAddress {
    #[serde(rename = "@type")]
    constructor: AdnlUdpAddressConstructor,
    ip: i32,
    port: u16,
}

impl AdnlUdpAddress {
    fn new(endpoint: AdnlEndpoint) -> Self {
        Self {
            constructor: AdnlUdpAddressConstructor::Udp,
            ip: i32::from_be_bytes(endpoint.ip.octets()),
            port: endpoint.port,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum AdnlUdpAddressConstructor {
    #[serde(rename = "adnl.address.udp")]
    Udp,
}

/// Canonical 256-bit TON key identifier used in keyring filenames and console calls.
///
/// The identifier is public metadata rather than private key material. Keeping the
/// decoded bytes prevents subtly different lowercase/uppercase representations from
/// referring to the same key in different parts of the bootstrap workflow.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, ToSchema)]
#[schema(value_type = String)]
pub struct KeyId([u8; 32]);

impl KeyId {
    /// Wraps an already decoded 32-byte identifier from a typed TON parser.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Decodes the 32-byte base64 form used by validator-engine JSON files.
    ///
    /// Keeping this conversion on the type prevents callers from round-tripping
    /// binary identifiers through an unrelated hexadecimal representation.
    pub fn from_base64(value: &str) -> Result<Self> {
        let bytes = BASE64
            .decode(value)
            .context("TON key identifier is not valid base64")?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("TON key identifier must contain exactly 32 bytes"))?;
        Ok(Self::from_bytes(bytes))
    }

    /// Parses exactly 64 hexadecimal characters into a 256-bit identifier.
    pub fn from_hex(value: &str) -> Result<Self> {
        ensure!(
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "TON key id must contain exactly 64 hexadecimal characters"
        );
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(value, &mut bytes).context("failed to decode TON key id")?;
        Ok(Self(bytes))
    }

    /// Returns the canonical lowercase spelling used by typed adapter contracts.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Returns the uppercase spelling used by official TON keyring filenames.
    ///
    /// Keeping this filesystem-specific convention separate from [`Self::to_hex`]
    /// avoids spreading casing conversions through validator and console workflows.
    pub fn to_keyring_filename(self) -> String {
        hex::encode_upper(self.0)
    }

    /// Encodes the identifier for validator-engine JSON configuration fields.
    pub fn to_base64(self) -> String {
        BASE64.encode(self.0)
    }
}

impl Serialize for KeyId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for KeyId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_base64(&value).map_err(D::Error::custom)
    }
}

impl fmt::Debug for KeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("KeyId")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl FromStr for KeyId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_hex(value)
    }
}

/// Validated raw Ed25519 public key used by TON identities.
///
/// The value is stored as 32 bytes rather than base64 or a TL-encoded file. This
/// prevents workflows from repeatedly decoding transport representations and
/// makes conversions at JSON and filesystem boundaries explicit.
#[derive(Clone, Copy, Eq, Hash, PartialEq, ToSchema)]
#[schema(value_type = String)]
pub struct TonPublicKey([u8; 32]);

impl TonPublicKey {
    /// Wraps an already decoded 32-byte key from a typed TON parser.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Decodes the 32-byte base64 form printed by official TON tools.
    pub fn from_base64(value: &str) -> Result<Self> {
        let bytes = BASE64
            .decode(value)
            .context("TON public key is not valid base64")?;
        Self::from_slice(&bytes)
    }

    /// Decodes a `.pub` artifact containing a TL constructor and 32-byte key.
    pub fn from_tl_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() == 36,
            "TL-encoded TON public key must contain exactly 36 bytes"
        );
        ensure!(
            bytes[..4] == ED25519_PUBLIC_KEY_TL_CONSTRUCTOR,
            "TON public key has an unexpected TL constructor"
        );
        Self::from_slice(&bytes[4..])
    }

    /// Encodes the complete `pub.ed25519` TL artifact written by TON tools.
    pub fn to_tl_bytes(self) -> [u8; 36] {
        let mut encoded = [0_u8; 36];
        encoded[..4].copy_from_slice(&ED25519_PUBLIC_KEY_TL_CONSTRUCTOR);
        encoded[4..].copy_from_slice(&self.0);
        encoded
    }

    /// Encodes the complete TL artifact for official tools that consume a public-key token.
    ///
    /// TON JSON stores the raw 32-byte key, while validator election Fift scripts
    /// expect the constructor-prefixed 36-byte value. Keeping both encodings on
    /// this type prevents workflow state from storing either transport representation.
    pub fn to_tl_base64(self) -> String {
        BASE64.encode(self.to_tl_bytes())
    }

    /// Computes the identifier used by TON keyrings and console commands.
    ///
    /// TON hashes the complete `pub.ed25519` TL value, including its four-byte
    /// constructor. Hashing only the raw Ed25519 bytes produces a different ID.
    pub fn key_id(self) -> KeyId {
        KeyId::from_bytes(Sha256::digest(self.to_tl_bytes()).into())
    }

    /// Borrows the raw key used by zerostate and protocol encoders.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encodes the public key for TON JSON configuration fields.
    pub fn to_base64(self) -> String {
        BASE64.encode(self.0)
    }

    /// Encodes the public key for operator-facing identities and comparisons.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    fn from_slice(bytes: &[u8]) -> Result<Self> {
        let key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("TON public key must contain exactly 32 bytes"))?;
        Ok(Self(key))
    }
}

impl fmt::Debug for TonPublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TonPublicKey")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for TonPublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl Serialize for TonPublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for TonPublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_base64(&value).map_err(D::Error::custom)
    }
}

/// Complete artifact set produced by `generate-random-id -m keys`.
///
/// The private key bytes never enter this value: only their path is retained. The
/// public key is decoded at the adapter boundary, while [`KeyId`] supplies
/// canonical keyring form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedKey {
    /// Canonical identifier printed by the official generator.
    pub id: KeyId,
    /// Validated public identity printed by the official generator.
    pub public_key: TonPublicKey,
    /// Private-key file written by the generator.
    pub private_path: PathBuf,
    /// TL-encoded public-key file written beside the private key.
    pub public_path: PathBuf,
}

/// Signed TON DHT node descriptor suitable for `global.config.json`.
///
/// The complete schema is modeled so a malformed generator response fails at the
/// adapter boundary instead of leaking arbitrary JSON into persistent network state.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DhtNodeDescriptor {
    #[serde(rename = "@type")]
    constructor: DhtNodeConstructor,
    id: Ed25519PublicKey,
    addr_list: AdnlAddressList,
    version: i32,
    signature: TonSignature,
}

impl fmt::Debug for DhtNodeDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Descriptors contain public keys and signatures. They are not secret, but
        // dumping the complete payload makes operational logs noisy and encourages
        // callers to treat release-owned JSON as a stable Rust data model.
        formatter.write_str("DhtNodeDescriptor(dht.node)")
    }
}

impl DhtNodeDescriptor {
    /// Parses a descriptor printed by `generate-random-id -m dht`.
    pub fn from_json_str(value: &str) -> Result<Self> {
        serde_json::from_str(value).context("invalid TON dht.node descriptor")
    }

    /// Reports whether this bootstrap node is reachable through a public IPv4 address.
    ///
    /// A config whose discovery nodes are public describes a network outside the
    /// operator's private LAN. Joining it with a private advertised address leaves
    /// overlay peers unable to return block proofs, although DHT probes can still
    /// appear healthy.
    pub(crate) fn advertises_public_ipv4(&self) -> bool {
        self.addr_list
            .addrs
            .iter()
            .map(|address| Ipv4Addr::from(address.ip.to_be_bytes()))
            .any(is_public_ipv4)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum DhtNodeConstructor {
    #[serde(rename = "dht.node")]
    Node,
}

/// Ed25519 public-key constructor used by TON JSON configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ed25519PublicKey {
    #[serde(rename = "@type")]
    constructor: PublicKeyConstructor,
    #[schema(value_type = String)]
    key: TonPublicKey,
}

impl Ed25519PublicKey {
    /// Wraps a validated key with TON's `pub.ed25519` JSON constructor.
    pub(crate) const fn new(key: TonPublicKey) -> Self {
        Self {
            constructor: PublicKeyConstructor::Ed25519,
            key,
        }
    }

    /// Returns the validated raw key for typed protocol adapters.
    pub(crate) const fn public_key(&self) -> TonPublicKey {
        self.key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum PublicKeyConstructor {
    #[serde(rename = "pub.ed25519")]
    Ed25519,
}

/// A validated 64-byte Ed25519 signature serialized in TON's base64 form.
#[derive(Clone, Eq, PartialEq, ToSchema)]
#[schema(value_type = String)]
struct TonSignature([u8; 64]);

impl Serialize for TonSignature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(self.0))
    }
}

impl<'de> Deserialize<'de> for TonSignature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let bytes = BASE64.decode(value).map_err(D::Error::custom)?;
        let bytes = bytes
            .try_into()
            .map_err(|_| D::Error::custom("TON signature must contain exactly 64 bytes"))?;
        Ok(Self(bytes))
    }
}

/// Persistent files created by `dht-server` initialization.
///
/// The keyring paths identify the stable DHT identities that must later be signed
/// into bootstrap descriptors. Reopening this database on normal startup preserves
/// those identities; generating it again would make the node undiscoverable under
/// its previously published descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtDatabase {
    /// Root passed to `dht-server --db` and later `dht-server -D`.
    pub path: PathBuf,
    /// Binary-owned configuration file validated after initialization.
    pub config: PathBuf,
    /// Canonically ordered private-key files created in the database keyring.
    pub keyring: Vec<PathBuf>,
}

impl DhtDatabase {
    /// Verifies that the database still contains the artifacts promised at init.
    ///
    /// This check is intentionally shallow: the official binary owns its config
    /// schema and key serialization, while Localton only requires the files to be
    /// present before it starts a persistent service.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.path.is_dir(),
            "DHT database directory does not exist: {}",
            self.path.display()
        );
        ensure!(
            self.config.is_file(),
            "DHT config does not exist: {}",
            self.config.display()
        );
        ensure!(!self.keyring.is_empty(), "DHT database has no keyring keys");
        for path in &self.keyring {
            ensure!(path.is_file(), "DHT key does not exist: {}", path.display());
        }
        Ok(())
    }

    /// Reconstructs the typed artifact set from an existing database directory.
    ///
    /// Only filenames that are canonical 256-bit hexadecimal identifiers are
    /// accepted. Sorting removes platform-specific directory iteration order so
    /// descriptor publication is deterministic.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let config = path.join("config.json");
        let keyring_dir = path.join("keyring");
        let mut keyring = Vec::new();
        for entry in std::fs::read_dir(&keyring_dir)
            .with_context(|| format!("failed to read DHT keyring {}", keyring_dir.display()))?
        {
            let key_path = entry?.path();
            if !key_path.is_file() {
                continue;
            }
            let Some(name) = key_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if KeyId::from_hex(name).is_ok() {
                keyring.push(key_path);
            }
        }
        keyring.sort();
        let database = Self {
            path,
            config,
            keyring,
        };
        database.validate()?;
        Ok(database)
    }

    /// Returns the keyring directory owned by the official DHT database.
    pub fn keyring_dir(&self) -> PathBuf {
        self.path.join("keyring")
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    #[test]
    fn key_ids_have_one_canonical_spelling() {
        let lower = "abcdef0123456789".repeat(4);
        let key = KeyId::from_hex(&lower).unwrap();

        assert_eq!(key.to_string(), lower);
        assert_eq!(key.to_keyring_filename(), "ABCDEF0123456789".repeat(4));
        assert!(KeyId::from_hex("not-a-key").is_err());
    }

    #[test]
    fn public_key_id_matches_the_official_tl_hash() {
        // Captured from TON v2026.06 `generate-random-id -m keys`.
        let bytes: [u8; 32] =
            hex::decode("15945a6bce5c6ba2e2d1205a40ca827ef2131f1986c574344cafbfdf1baa5e17")
                .unwrap()
                .try_into()
                .unwrap();

        expect!["c0028373a759ddf15c4f7f6c1abac7fa85ac34d8d82c335ce173fae5f6ee1f9c"]
            .assert_eq(&TonPublicKey::from_bytes(bytes).key_id().to_hex());
    }

    #[test]
    fn occupied_adnl_endpoint_reports_a_stale_ton_process() {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let error = AdnlEndpoint::new(Ipv4Addr::LOCALHOST, port)
            .ensure_available("temporary validator-engine")
            .unwrap_err()
            .to_string()
            .replace(&port.to_string(), "<port>");

        expect![[r#"temporary validator-engine cannot bind ADNL endpoint 127.0.0.1:<port>: address is already in use; a TON process from a previous Localton run may still be running"#]]
            .assert_eq(&error);
    }

    #[test]
    fn adnl_port_probe_does_not_bind_the_advertised_address() {
        AdnlEndpoint::new(Ipv4Addr::new(203, 0, 113, 7), 0)
            .ensure_available("validator-engine")
            .unwrap();
    }

    #[test]
    fn dht_descriptor_rejects_incomplete_or_unknown_shapes() {
        let value = serde_json::json!({
            "@type": "dht.node",
            "id": {
                "@type": "pub.ed25519",
                "key": BASE64.encode([1_u8; 32]),
            },
            "addr_list": {
                "@type": "adnl.addressList",
                "addrs": [{
                    "@type": "adnl.address.udp",
                    "ip": 2_130_706_433_i32,
                    "port": 6302,
                }],
                "version": 0,
                "reinit_date": 0,
                "priority": 0,
                "expire_at": 0,
            },
            "version": 7,
            "signature": BASE64.encode([2_u8; 64]),
        });
        let descriptor: DhtNodeDescriptor = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(serde_json::to_value(descriptor).unwrap(), value);
        assert!(serde_json::from_value::<DhtNodeDescriptor>(serde_json::json!({})).is_err());
        assert!(
            serde_json::from_value::<DhtNodeDescriptor>(serde_json::json!({
                "@type": "dht.node",
                "unexpected": true,
            }))
            .is_err()
        );
    }
}
