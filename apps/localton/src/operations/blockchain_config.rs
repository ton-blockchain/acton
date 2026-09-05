//! Changes one blockchain configuration parameter through the config contract.
//!
//! TON keeps its configuration in the data of the configuration smart contract,
//! and the collator re-reads it from there on every masterchain block. Two paths
//! can change it. Validators can vote on a proposal, which only takes effect
//! after whole validator rounds have passed. Or a request signed by the
//! configuration master key can set one parameter immediately
//! (`crypto/smartcont/config-code.fc`, action `0x43665021`). A development
//! network keeps that key next to its zerostate, so the direct path costs one
//! external message and no restart.
//!
//! The contract stores whatever value it is given; validity is only checked
//! afterwards by the collator, and a configuration it rejects stops block
//! production permanently. Values are therefore checked against the release's
//! own TL-B schema before the request is signed.

use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, ensure};
use ed25519_dalek::{Signer, SigningKey};
use tempfile::TempDir;
use tracing::info;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::models::message::{ExtInMsgInfo, IntAddr, MsgInfo, OwnedMessage, StdAddr};
use tycho_types::num::Tokens;
use tycho_types::prelude::HashBytes;

use crate::{
    cli::BlockchainConfigCommand,
    storage::Layout,
    ton::{
        lite::require_existing_config,
        toolchain::Toolchain,
        tools::{
            fift::{Fift, FiftScriptRequest, OfficialFift},
            lite_client::{Boc as LiteBoc, LiteTarget, RunMethodRequest},
            types::OperationContext,
        },
    },
};

/// `change one configuration parameter` action of the configuration contract.
const CHANGE_PARAM_ACTION: u32 = 0x4366_5021;

/// How long a signed request stays acceptable to the contract.
///
/// The contract rejects a request whose deadline has passed
/// (`throw_if(35, valid_until < now())`), so the window only has to cover
/// delivery and one round of collation.
const REQUEST_LIFETIME: Duration = Duration::from_secs(120);

/// Bounded deadline for the tool calls one parameter change performs.
const OPERATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs one blockchain configuration command.
pub(crate) async fn execute(command: BlockchainConfigCommand) -> Result<()> {
    match command {
        BlockchainConfigCommand::Set {
            state,
            index,
            value,
            force,
        } => {
            let outcome = set_param(&state.state_dir, index, &value, force).await?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
    }
    Ok(())
}

/// What one accepted parameter change reports back.
#[derive(Debug, serde::Serialize)]
pub(crate) struct SetParamOutcome {
    /// Index of the parameter that was changed.
    pub(crate) index: i32,
    /// Sequence number the request consumed.
    pub(crate) seqno: u32,
    /// Representation hash of the new parameter value.
    pub(crate) value_hash: String,
}

/// Sets one configuration parameter and waits for the network to accept it.
pub(crate) async fn set_param(
    state_dir: &Path,
    index: i32,
    value: &Path,
    force: bool,
) -> Result<SetParamOutcome> {
    // Serialize config-master seqno allocation across independent CLI callers.
    let lock_path = state_dir.join("blockchain-config.lock");
    let _lock = crate::bootstrap::acquire_lock(&lock_path)
        .context("Another blockchain configuration change is in progress")?;
    let toolchain = Toolchain::resolve(state_dir, None).await?;
    let master = ConfigMaster::load(&toolchain.layout)?;

    let stdin_dir = TempDir::new()?;
    let stdin_value = stdin_dir.path().join("config.boc");
    let value = if value == Path::new("-") {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "Parameter BoC is too large"
        );
        fs::write(&stdin_value, bytes)?;
        stdin_value.as_path()
    } else {
        value
    };
    let value_boc = fs::read(value)
        .with_context(|| format!("failed to read parameter value {}", value.display()))?;
    let value_cell = Boc::decode(&value_boc)
        .with_context(|| format!("{} is not a valid BoC", value.display()))?;

    if force {
        info!(
            operation = "set_config_param",
            index, "skipping schema validation of the parameter value"
        );
    } else {
        validate_value(&toolchain, index, value).await?;
    }

    let target = lite_target(&toolchain)?;
    let context = OperationContext::new(OPERATION_TIMEOUT);
    let seqno = toolchain
        .lite_client_tool
        .run_method(
            &context,
            &target,
            RunMethodRequest::new(&master.address.to_string(), "seqno", Vec::new())?,
        )
        .await
        .context("failed to read the configuration contract sequence number")?
        .first_u64()?
        .try_into()
        .context("configuration contract sequence number does not fit into 32 bits")?;

    let valid_until = unix_time()?
        .checked_add(REQUEST_LIFETIME.as_secs() as u32)
        .context("configuration request deadline overflow")?;
    let message = master.change_param_message(seqno, valid_until, index, value_cell.clone())?;

    toolchain
        .lite_client_tool
        .send_boc(&context, &target, LiteBoc::new(Boc::encode(message))?)
        .await
        .context("failed to submit the configuration change")?;

    let mut client = crate::ton::lite::LocalLiteClient::connect(toolchain.lite_config()).await?;
    tokio::time::timeout(OPERATION_TIMEOUT, async {
        loop {
            let current = client.config_params(vec![index]).await?;
            let accepted = toolchain.lite_client_tool.run_method(
                &context, &target,
                RunMethodRequest::new(&master.address.to_string(), "seqno", Vec::new())?,
            ).await?.first_u64()?;
            if accepted > u64::from(seqno) && current.get_raw_cell(index as u32)?.is_some_and(|cell| cell.repr_hash() == value_cell.repr_hash()) {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }).await.context("Configuration change was submitted but confirmation timed out; read the active value before retrying")??;

    info!(
        operation = "set_config_param",
        index,
        seqno,
        outcome = "confirmed",
        "confirmed a configuration parameter change"
    );
    Ok(SetParamOutcome {
        index,
        seqno,
        value_hash: value_cell.repr_hash().to_string(),
    })
}

/// Rejects values the pinned release would not accept for this parameter.
///
/// The check runs the release's own `is-valid-config?`, so it stays in step with
/// the node instead of duplicating its TL-B schema.
async fn validate_value(toolchain: &Toolchain, index: i32, value: &Path) -> Result<()> {
    let workspace = TempDir::new().context("failed to create a validation directory")?;
    let script = workspace.path().join("validate-config-param.fif");
    fs::write(
        &script,
        include_str!("../../assets/validate-config-param.fif"),
    )
    .context("failed to write the validation script")?;

    let value = dunce::canonicalize(value)
        .with_context(|| format!("failed to resolve {}", value.display()))?;
    // Plain `fift` cannot check configuration values; only the interpreter that
    // links the block schema can.
    let validator = OfficialFift::with_block_schema(toolchain.binaries.clone());
    let output = validator
        .run_script(
            &OperationContext::new(OPERATION_TIMEOUT),
            FiftScriptRequest {
                script,
                arguments: vec![index.to_string().into(), value.into_os_string()],
                current_dir: workspace.path().to_path_buf(),
                include_paths: vec![toolchain.layout.smartcont.clone()],
            },
        )
        .await
        .with_context(|| {
            format!("configuration parameter {index} value was rejected by the TON schema")
        })?;
    ensure!(
        output.stdout.contains("valid"),
        "configuration parameter {index} value was rejected by the TON schema"
    );
    Ok(())
}

/// The identity that may change configuration parameters without a vote.
struct ConfigMaster {
    address: StdAddr,
    key: SigningKey,
}

impl ConfigMaster {
    /// Loads the configuration contract address and its master key.
    ///
    /// Both are artifacts of zerostate creation: `gen-zerostate.fif` generates
    /// the key pair and stores its public half in the contract's data.
    fn load(layout: &Layout) -> Result<Self> {
        let address = read_address(&layout.zerostate.join("config-master.addr"))?;
        let key = read_key(&layout.zerostate.join("config-master.pk"))?;
        Ok(Self { address, key })
    }

    /// Builds the signed external message that sets one parameter.
    ///
    /// The layout mirrors `crypto/smartcont/update-config.fif`: the request cell
    /// carries the action, the contract's sequence number, a deadline and the
    /// parameter index, with the new value as its only reference. The signature
    /// covers that cell's representation hash, and the message body is the
    /// signature followed by the request inlined with its reference.
    fn change_param_message(
        &self,
        seqno: u32,
        valid_until: u32,
        index: i32,
        value: Cell,
    ) -> Result<Cell> {
        let mut request = CellBuilder::new();
        request.store_u32(CHANGE_PARAM_ACTION)?;
        request.store_u32(seqno)?;
        request.store_u32(valid_until)?;
        request.store_u32(index as u32)?;
        request.store_reference(value)?;
        let request = request
            .build()
            .context("failed to build the request cell")?;

        let signature = self.key.sign(request.repr_hash().as_slice()).to_bytes();

        let mut body = CellBuilder::new();
        body.store_raw(&signature, 512)?;
        body.store_slice(request.as_slice()?)?;
        let body = body.build().context("failed to build the request body")?;

        let message = OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                src: None,
                dst: IntAddr::Std(self.address.clone()),
                import_fee: Tokens::ZERO,
            }),
            init: None,
            body: body.into(),
            layout: None,
        };
        CellBuilder::build_from(&message).context("failed to build the external message")
    }
}

/// Reads a TON `.addr` artifact: an account id followed by its workchain.
fn read_address(path: &Path) -> Result<StdAddr> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read address {}", path.display()))?;
    ensure!(
        bytes.len() == 36,
        "{} must contain exactly 36 bytes",
        path.display()
    );
    let account: [u8; 32] = bytes[..32].try_into().expect("checked length");
    let workchain = i32::from_le_bytes(bytes[32..].try_into().expect("checked length"));
    let workchain = i8::try_from(workchain)
        .with_context(|| format!("{} names an unsupported workchain", path.display()))?;
    Ok(StdAddr::new(workchain, HashBytes(account)))
}

/// Reads a TON `.pk` artifact: one raw ed25519 private key.
fn read_key(path: &Path) -> Result<SigningKey> {
    let bytes = fs::read(path).with_context(|| {
        format!(
            "failed to read the configuration master key {}",
            path.display()
        )
    })?;
    let bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
        anyhow::anyhow!(
            "{} must contain exactly 32 bytes of private key",
            path.display()
        )
    })?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn lite_target(toolchain: &Toolchain) -> Result<LiteTarget> {
    require_existing_config(toolchain.lite_config())?;
    Ok(LiteTarget::new(toolchain.lite_config()).with_label("localton"))
}

fn unix_time() -> Result<u32> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the unix epoch")?
        .as_secs()
        .try_into()
        .context("system clock does not fit into a TON timestamp")
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Verifier, VerifyingKey};

    use super::*;

    const INDEX: i32 = 14;
    const SEQNO: u32 = 7;
    const VALID_UNTIL: u32 = 1_800_000_000;

    fn master() -> ConfigMaster {
        ConfigMaster {
            address: StdAddr::new(-1, HashBytes([0x55; 32])),
            key: SigningKey::from_bytes(&[9; 32]),
        }
    }

    fn value() -> Cell {
        let mut builder = CellBuilder::new();
        builder.store_u8(0x6b).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn request_is_addressed_to_the_configuration_contract() {
        let master = master();
        let message = master
            .change_param_message(SEQNO, VALID_UNTIL, INDEX, value())
            .unwrap();

        let message = message.parse::<OwnedMessage>().unwrap();
        let MsgInfo::ExtIn(info) = message.info else {
            panic!("configuration requests are external inbound messages");
        };
        assert_eq!(info.dst, IntAddr::Std(master.address));
        assert!(info.src.is_none());
        assert!(message.init.is_none());
    }

    #[test]
    fn request_body_is_signed_by_the_configuration_master_key() {
        let master = master();
        let verifying = VerifyingKey::from(&master.key);
        let message = master
            .change_param_message(SEQNO, VALID_UNTIL, INDEX, value())
            .unwrap();

        let message = message.parse::<OwnedMessage>().unwrap();
        let (range, cell) = message.body;
        let mut body = range.apply(&cell).unwrap();

        let mut signature = [0u8; 64];
        body.load_raw(&mut signature, 512).unwrap();

        // What is signed is the request cell, which the body carries inlined
        // together with its reference to the new value.
        let mut request = CellBuilder::new();
        request.store_slice(body).unwrap();
        let request = request.build().unwrap();

        verifying
            .verify(request.repr_hash().as_slice(), &signature.into())
            .expect("the configuration contract checks this signature");

        let mut fields = request.as_slice().unwrap();
        assert_eq!(fields.load_u32().unwrap(), CHANGE_PARAM_ACTION);
        assert_eq!(fields.load_u32().unwrap(), SEQNO);
        assert_eq!(fields.load_u32().unwrap(), VALID_UNTIL);
        assert_eq!(fields.load_u32().unwrap() as i32, INDEX);
        assert_eq!(
            fields.load_reference().unwrap().repr_hash(),
            value().repr_hash()
        );
    }

    #[test]
    fn negative_parameter_indexes_survive_the_round_trip() {
        let message = master()
            .change_param_message(SEQNO, VALID_UNTIL, -1000, value())
            .unwrap();

        let message = message.parse::<OwnedMessage>().unwrap();
        let (range, cell) = message.body;
        let mut body = range.apply(&cell).unwrap();
        body.skip_first(512, 0).unwrap();
        body.skip_first(96, 0).unwrap();
        assert_eq!(body.load_u32().unwrap() as i32, -1000);
    }

    #[test]
    fn address_artifact_carries_the_account_before_its_workchain() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config-master.addr");
        let mut bytes = vec![0x55; 32];
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        fs::write(&path, bytes).unwrap();

        let address = read_address(&path).unwrap();
        assert_eq!(address.workchain, -1);
        assert_eq!(address.address, HashBytes([0x55; 32]));
    }

    #[test]
    fn a_truncated_address_artifact_is_rejected() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config-master.addr");
        fs::write(&path, [0x55; 32]).unwrap();

        assert!(read_address(&path).is_err());
    }
}
