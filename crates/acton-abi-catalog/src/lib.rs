use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use tolk_compiler::abi::{ABIDeclaration, ContractABI};

const DATA_ABIS_ZST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data-abis.json.zst"));

static CATALOG: OnceLock<AbiCatalog> = OnceLock::new();

#[derive(Debug)]
pub struct AbiCatalog {
    contracts: Vec<CatalogContract>,
    by_code_hash: HashMap<String, usize>,
    by_opcode: HashMap<u32, Vec<usize>>,
}

#[derive(Debug, Clone)]
pub struct CatalogContract {
    pub display_name: String,
    pub code_hashes: Vec<String>,
    pub links: Vec<ContractAbiLink>,
    abi: Arc<ContractABI>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractAbiLink {
    pub kind: String,
    pub title: String,
    pub url: String,
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtendedContractAbi<T = ContractABI> {
    pub compiler_abi: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub code_hashes: Vec<String>,
    #[serde(default)]
    pub links: Vec<ContractAbiLink>,
}

#[derive(Debug, Deserialize)]
struct RawBundle {
    contracts: Vec<RawContract>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContract {
    display_name: String,
    hashes: Vec<String>,
    compiler_abi: ContractABI,
    #[serde(default)]
    links: Vec<ContractAbiLink>,
}

#[must_use]
pub fn catalog() -> &'static AbiCatalog {
    CATALOG.get_or_init(|| AbiCatalog::load().expect("bundled ABI catalog JSON must be valid"))
}

#[must_use]
pub fn find_contract_by_code_hash(code_hash: &str) -> Option<&'static CatalogContract> {
    catalog().find_contract_by_code_hash(code_hash)
}

#[must_use]
pub fn find_contract_by_name(name: &str) -> Option<&'static CatalogContract> {
    catalog().find_contract_by_name(name)
}

#[must_use]
pub fn find_abi_by_code_hash(code_hash: &str) -> Option<Arc<ContractABI>> {
    find_contract_by_code_hash(code_hash).map(CatalogContract::abi)
}

#[must_use]
pub fn find_abis_by_opcode(opcode: u32) -> Vec<Arc<ContractABI>> {
    catalog().find_abis_by_opcode(opcode)
}

impl AbiCatalog {
    fn load() -> Result<Self, CatalogLoadError> {
        let json = zstd::stream::decode_all(DATA_ABIS_ZST)?;
        let json = String::from_utf8(json)?;
        Ok(Self::from_json(&json)?)
    }

    fn from_json(json: &str) -> serde_json::Result<Self> {
        let raw: RawBundle = serde_json::from_str(json)?;
        let mut contracts = Vec::with_capacity(raw.contracts.len());
        let mut by_code_hash = HashMap::new();
        let mut by_opcode: HashMap<u32, Vec<usize>> = HashMap::new();

        for raw_contract in raw.contracts {
            let contract_index = contracts.len();
            let code_hashes = raw_contract
                .hashes
                .into_iter()
                .filter_map(|hash| normalize_code_hash(&hash))
                .collect::<Vec<_>>();

            for code_hash in &code_hashes {
                by_code_hash
                    .entry(code_hash.clone())
                    .or_insert(contract_index);
            }

            for opcode in opcodes_from_abi(&raw_contract.compiler_abi) {
                by_opcode.entry(opcode).or_default().push(contract_index);
            }

            contracts.push(CatalogContract {
                display_name: raw_contract.display_name,
                code_hashes,
                links: raw_contract.links,
                abi: Arc::new(raw_contract.compiler_abi),
            });
        }

        Ok(Self {
            contracts,
            by_code_hash,
            by_opcode,
        })
    }

    #[must_use]
    pub fn contracts(&self) -> &[CatalogContract] {
        &self.contracts
    }

    #[must_use]
    pub fn find_contract_by_code_hash(&self, code_hash: &str) -> Option<&CatalogContract> {
        let normalized = normalize_code_hash(code_hash)?;
        self.by_code_hash
            .get(&normalized)
            .and_then(|index| self.contracts.get(*index))
    }

    #[must_use]
    pub fn find_contract_by_name(&self, name: &str) -> Option<&CatalogContract> {
        let normalized = normalize_contract_name(name);
        if normalized.is_empty() {
            return None;
        }

        self.contracts.iter().find(|contract| {
            normalize_contract_name(&contract.display_name) == normalized
                || normalize_contract_name(&contract.abi.contract_name) == normalized
        })
    }

    #[must_use]
    pub fn find_abis_by_opcode(&self, opcode: u32) -> Vec<Arc<ContractABI>> {
        self.by_opcode
            .get(&opcode)
            .into_iter()
            .flatten()
            .filter_map(|index| self.contracts.get(*index))
            .map(CatalogContract::abi)
            .collect()
    }
}

#[derive(Debug)]
enum CatalogLoadError {
    Zstd(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for CatalogLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zstd(error) => write!(formatter, "failed to decompress catalog: {error}"),
            Self::Utf8(error) => write!(formatter, "catalog is not UTF-8: {error}"),
            Self::Json(error) => write!(formatter, "failed to parse catalog JSON: {error}"),
        }
    }
}

impl std::error::Error for CatalogLoadError {}

impl From<std::io::Error> for CatalogLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Zstd(error)
    }
}

impl From<std::string::FromUtf8Error> for CatalogLoadError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::Utf8(error)
    }
}

impl From<serde_json::Error> for CatalogLoadError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl CatalogContract {
    #[must_use]
    pub fn abi(&self) -> Arc<ContractABI> {
        self.abi.clone()
    }

    #[must_use]
    pub fn extended_abi(&self) -> ExtendedContractAbi {
        ExtendedContractAbi {
            compiler_abi: self.abi.as_ref().clone(),
            display_name: Some(self.display_name.clone()),
            code_hashes: self.code_hashes.clone(),
            links: self.links.clone(),
        }
    }
}

fn normalize_code_hash(code_hash: &str) -> Option<String> {
    let code_hash = code_hash.strip_prefix("0x").unwrap_or(code_hash).trim();
    if code_hash.len() != 64 || !code_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(code_hash.to_ascii_lowercase())
}

fn normalize_contract_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn opcodes_from_abi(abi: &ContractABI) -> Vec<u32> {
    let mut opcodes = Vec::new();
    let mut seen = HashSet::new();

    for declaration in &abi.declarations {
        let ABIDeclaration::Struct {
            prefix: Some(prefix),
            fields,
            ..
        } = declaration
        else {
            continue;
        };

        // A bare `0x00000001` body is too ambiguous for code-hash-free fallback.
        // Keep entries like Getgems deploy code-hash matched only.
        if prefix.prefix_len == 32 && prefix.prefix_num == 1 && fields.is_empty() {
            continue;
        }

        if prefix.prefix_len == 32
            && prefix.prefix_num != 0
            && let Ok(opcode) = u32::try_from(prefix.prefix_num)
            && seen.insert(opcode)
        {
            opcodes.push(opcode);
        }
    }

    opcodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_wallet_v1r1_by_code_hash() {
        let contract = find_contract_by_code_hash(
            "a0cfc2c48aee16a271f2cfc0b7382d81756cecb1017d077faaab3bb602f6868c",
        )
        .expect("wallet v1r1 must be present in bundled catalog");

        assert_eq!(contract.display_name, "WalletV1r1");
        assert_eq!(contract.abi().contract_name, "WalletV1r1");
    }

    #[test]
    fn finds_contract_by_display_name() {
        let contract = find_contract_by_name("WalletV1r1")
            .expect("wallet v1r1 must be present in bundled catalog");

        assert_eq!(contract.display_name, "WalletV1r1");
        assert_eq!(contract.abi().contract_name, "WalletV1r1");
    }

    #[test]
    fn finds_contract_by_compiler_abi_name() {
        let contract = find_contract_by_name("AffluentAccount")
            .expect("affluent account must be searchable by compiler ABI name");

        assert_eq!(contract.display_name, "Account");
        assert_eq!(contract.abi().contract_name, "AffluentAccount");
    }

    #[test]
    fn loads_contract_links() {
        let contract = find_contract_by_name("Jetton2Master")
            .expect("jetton master must be present in bundled catalog");

        assert!(
            contract
                .links
                .iter()
                .any(|link| link.kind == "spec" && link.title.contains("TEP-74")),
            "catalog contract links must be loaded from bundled metadata"
        );
    }

    #[test]
    fn finds_template_jetton_minter_by_code_hash() {
        let contract = find_contract_by_code_hash(
            "6bf8f48ca97d3fd9c8e553344efe7af030c322459e2ee2197a052162f1961bfb",
        )
        .expect("template jetton minter must be present in bundled catalog");

        assert_eq!(contract.display_name, "JettonMinter");
        assert_eq!(contract.abi().contract_name, "JettonMinter");
        assert_eq!(contract.abi().storage.storage_ty_idx, Some(36));
    }

    #[test]
    fn normalizes_uppercase_prefixed_hashes() {
        let contract = find_contract_by_code_hash(
            "0xA0CFC2C48AEE16A271F2CFC0B7382D81756CECB1017D077FAAAB3BB602F6868C",
        )
        .expect("uppercase hash must resolve");

        assert_eq!(contract.display_name, "WalletV1r1");
    }

    #[test]
    fn finds_abis_by_message_opcode() {
        let abis = find_abis_by_opcode(0x0f8a7ea5);

        assert!(
            abis.iter()
                .any(|abi| abi.contract_name.to_lowercase().contains("jetton")),
            "jetton transfer opcode must resolve to at least one jetton ABI"
        );
    }

    #[test]
    fn finds_wallet_v4r2_plugin_destruct_by_opcode() {
        let abis = find_abis_by_opcode(0x64737472);

        assert!(
            abis.iter().any(|abi| abi.contract_name == "WalletV4r2"),
            "wallet v4r2 plugin destruct opcode must resolve to WalletV4r2"
        );
    }

    #[test]
    fn does_not_index_zero_opcode_for_global_fallback() {
        let abis = find_abis_by_opcode(0);

        assert!(
            abis.is_empty(),
            "zero opcode is too ambiguous for global catalog fallback"
        );
    }

    #[test]
    fn does_not_index_empty_opcode_one_for_global_fallback() {
        let abis = find_abis_by_opcode(1);

        assert!(
            !abis
                .iter()
                .any(|abi| abi.contract_name == "GetgemsDeployer"),
            "empty opcode 1 must stay code-hash matched instead of a global fallback"
        );
    }

    #[test]
    fn embeds_compressed_catalog() {
        assert!(DATA_ABIS_ZST.len() < include_bytes!("../data/data-abis.json").len());
    }
}
