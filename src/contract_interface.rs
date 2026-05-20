use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig};
use anyhow::anyhow;
use std::path::Path;
use tolk_compiler::abi::ContractABI;
use tolk_compiler::{CompilerResult, SourceMap};
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes;

pub(crate) struct ContractInterface {
    pub abi: ContractABI,
    pub source_map: SourceMap,
}

pub(crate) struct PrecompiledBoc {
    pub code_boc64: String,
    pub code_hash: HashBytes,
}

pub(crate) fn is_boc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("boc"))
}

pub(crate) fn read_precompiled_boc(
    path: &Path,
    source_display: &str,
) -> anyhow::Result<PrecompiledBoc> {
    let boc_data = std::fs::read(path)
        .map_err(|err| anyhow!("Failed to read BoC file {source_display}: {err}"))?;
    let boc = Boc::decode(&boc_data)
        .map_err(|err| anyhow!("Failed to decode BoC file {source_display}: {err}"))?;

    Ok(PrecompiledBoc {
        code_boc64: Boc::encode_base64(&boc),
        code_hash: *boc.repr_hash(),
    })
}

pub(crate) fn compile_optional_contract_interface(
    config: &ActonConfig,
    project_root: &Path,
    contract_id: &str,
    contract_config: &ContractConfig,
) -> anyhow::Result<Option<ContractInterface>> {
    let Some(types_path) = contract_config.absolute_types_path(project_root) else {
        return Ok(None);
    };

    compile_contract_interface(config, contract_id, contract_config, &types_path).map(Some)
}

pub(crate) fn compile_required_contract_interface(
    config: &ActonConfig,
    project_root: &Path,
    contract_id: &str,
    contract_config: &ContractConfig,
) -> anyhow::Result<ContractInterface> {
    let Some(types_path) = contract_config.absolute_types_path(project_root) else {
        anyhow::bail!(
            "Contract {} uses a precompiled BoC source, so wrapper generation requires `types = \"path/to/types.tolk\"` in Acton.toml",
            contract_id.yellow()
        );
    };

    compile_contract_interface(config, contract_id, contract_config, &types_path)
}

fn compile_contract_interface(
    config: &ActonConfig,
    contract_id: &str,
    contract_config: &ContractConfig,
    types_path: &Path,
) -> anyhow::Result<ContractInterface> {
    if !types_path.exists() {
        anyhow::bail!(
            "Types file for {} not found: {} (specified in Acton.toml as {})",
            contract_id.yellow(),
            types_path.display().to_string().yellow(),
            contract_config
                .types
                .as_deref()
                .unwrap_or_default()
                .yellow()
        );
    }

    let mappings = config.mappings();
    let compiler = tolk_compiler::Compiler::new(2)
        .with_allow_no_entrypoint(true)
        .with_mappings(&mappings);

    match compiler.compile(types_path, false) {
        CompilerResult::Success(result) => {
            let abi = result.abi.ok_or_else(|| {
                anyhow!(
                    "Types file {} did not produce ABI for {}",
                    types_path.display().to_string().yellow(),
                    contract_id.yellow()
                )
            })?;
            let source_map = result.source_map.ok_or_else(|| {
                anyhow!(
                    "Types file {} did not produce symbol types for {}",
                    types_path.display().to_string().yellow(),
                    contract_id.yellow()
                )
            })?;

            Ok(ContractInterface { abi, source_map })
        }
        CompilerResult::Error(error) => {
            anyhow::bail!(
                "Failed to compile types file {} for {}: {}",
                types_path.display().to_string().yellow(),
                contract_id.yellow(),
                error.message.trim_end()
            );
        }
    }
}
