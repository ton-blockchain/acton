#![cfg(all(feature = "wallet", feature = "serde"))]

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use ed25519_dalek::{Signature, VerifyingKey};
use expect_test::expect_file;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, CellSlice, HashBytes, Load, Store};
use rston::dict::{Dict, RawDict};
use rston::models::{OwnedRelaxedMessage, RelaxedMsgInfo, StateInit, StdAddr};
use rston::wallet::*;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Fixtures {
    codes: Vec<Fixture>,
    states: Vec<Fixture>,
    messages: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    id: String,
    version: WalletVersion,
    file: String,
    hash: HashBytes,
    #[serde(default)]
    address: Option<StdAddr>,
    #[serde(default)]
    code_hash: Option<HashBytes>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    account: Option<String>,
}

fn fixtures() -> Result<Fixtures> {
    Ok(serde_json::from_str(include_str!(
        "fixtures/wallet/manifest.json"
    ))?)
}

fn read_cell(fixture: &Fixture) -> Result<Cell> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wallet")
        .join(&fixture.file);
    let cell = Boc::decode(std::fs::read(&path)?)
        .with_context(|| format!("decode {} from {}", fixture.id, path.display()))?;
    ensure!(
        cell.repr_hash() == &fixture.hash,
        "{}: source hash",
        fixture.id
    );
    Ok(cell)
}

fn load_exact<T: for<'a> Load<'a> + Store>(cell: &Cell) -> Result<T> {
    let mut slice = cell.as_slice()?;
    let value = T::load_from(&mut slice)?;
    ensure_empty(slice)?;
    ensure!(
        CellBuilder::build_from(&value)?.repr_hash() == cell.repr_hash(),
        "serialization must preserve the original cell tree"
    );
    Ok(value)
}

fn ensure_empty(slice: CellSlice<'_>) -> Result<()> {
    ensure!(
        slice.is_data_empty() && slice.is_refs_empty(),
        "unconsumed input: {} bits, {} references",
        slice.size_bits(),
        slice.size_refs()
    );
    Ok(())
}

#[test]
fn identifies_all_wallet_code_revisions() -> Result<()> {
    let mut snapshot = BTreeMap::new();
    for fixture in fixtures()?.codes {
        let code = read_cell(&fixture)?;
        ensure!(get_version_by_code(*code.repr_hash())? == fixture.version);
        ensure!(get_code(fixture.version)?.repr_hash() == code.repr_hash());
        snapshot.insert(
            fixture.id,
            json!({
                "version": fixture.version,
                "code_hash": code.repr_hash(),
            }),
        );
    }
    expect_file![concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/wallet_codes.json"
    )]
    .assert_eq(&serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

#[test]
fn parses_mainnet_wallet_storage() -> Result<()> {
    let mut snapshot = BTreeMap::new();
    for fixture in fixtures()?.states {
        let cell = read_cell(&fixture)?;
        let data = if fixture.kind.as_deref() == Some("state_init") {
            let state: StateInit = load_exact(&cell)?;
            let code = state.code.context("deployment code")?;
            ensure!(get_version_by_code(*code.repr_hash())? == fixture.version);
            ensure!(
                fixture
                    .address
                    .as_ref()
                    .context("deployment address")?
                    .address
                    == *cell.repr_hash(),
                "{}: StateInit must derive the deployed address",
                fixture.id
            );
            state.data.context("deployment data")?
        } else {
            let hash = fixture.code_hash.context("account code hash")?;
            ensure!(get_version_by_code(hash)? == fixture.version);
            cell
        };
        let (_, mut fields) = read_storage(fixture.version, &data)
            .with_context(|| format!("storage {}", fixture.id))?;
        fields["data_hash"] = json!(data.repr_hash());
        snapshot.insert(fixture.id, fields);
    }
    expect_file![concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/wallet_storage.json"
    )]
    .assert_eq(&serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn read_storage(version: WalletVersion, cell: &Cell) -> Result<(HashBytes, Value)> {
    use WalletVersion::*;

    let (public_key, fields) = match version {
        V1R1 | V1R2 | V1R3 | V2R1 | V2R2 => {
            let data: WalletV1V2Data = load_exact(cell)?;
            (data.public_key, json!({ "seqno": data.seqno }))
        }
        V3R1 | V3R2 => {
            let data: WalletV3Data = load_exact(cell)?;
            (
                data.public_key,
                json!({ "seqno": data.seqno, "wallet_id": data.wallet_id }),
            )
        }
        V4R1 | V4R2 => {
            let data: WalletV4Data = load_exact(cell)?;
            let mut plugins = Vec::new();
            for entry in RawDict::<264>::from(data.plugins).iter() {
                let (key, value) = entry?;
                ensure_empty(value)?;
                let mut key = key.as_data_slice();
                let workchain = key.load_u8()? as i8;
                let address = HashBytes::load_from(&mut key)?;
                plugins.push(StdAddr::new(workchain, address).to_string());
                ensure_empty(key)?;
            }
            (
                data.public_key,
                json!({
                    "seqno": data.seqno, "wallet_id": data.wallet_id, "plugins": plugins,
                }),
            )
        }
        V5R1 => {
            let data: WalletV5Data = load_exact(cell)?;
            let extensions = Dict::<HashBytes, bool>::from_raw(data.extensions)
                .iter()
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            (
                data.public_key,
                json!({
                    "seqno": data.seqno, "wallet_id": data.wallet_id,
                    "sign_allowed": data.sign_allowed, "extensions": extensions,
                }),
            )
        }
        HLV2R2 => {
            let data: WalletHLV2R2Data = load_exact(cell)?;
            let queries = Dict::<u64, ()>::from_raw(data.queries)
                .keys()
                .collect::<Result<Vec<_>, _>>()?;
            (
                data.public_key,
                json!({
                    "wallet_id": data.wallet_id,
                    "last_cleaned_time": data.last_cleaned_time,
                    "queries": queries,
                }),
            )
        }
        _ => bail!("no storage fixture decoder for {version:?}"),
    };
    let mut fields = fields;
    fields["public_key"] = json!(public_key);
    Ok((public_key, fields))
}

#[test]
fn parses_and_verifies_mainnet_signed_transfers() -> Result<()> {
    let fixtures = fixtures()?;
    let mut snapshot = BTreeMap::new();
    for fixture in &fixtures.messages {
        let account = fixtures
            .states
            .iter()
            .find(|state| Some(&state.id) == fixture.account.as_ref())
            .with_context(|| format!("{}: public-key account", fixture.id))?;
        ensure!(account.address == fixture.address && account.version == fixture.version);
        let (public_key, _) = read_storage(account.version, &read_cell(account)?)?;
        let body = read_cell(fixture)?;
        let fields = read_signed_transfer(fixture.version, &body, &public_key)
            .with_context(|| format!("signed transfer {}", fixture.id))?;
        snapshot.insert(&fixture.id, fields);
    }
    expect_file![concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/wallet_transfers.json"
    )]
    .assert_eq(&serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn read_signed_transfer(
    version: WalletVersion,
    cell: &Cell,
    public_key: &HashBytes,
) -> Result<Value> {
    let mut slice = cell.as_slice()?;
    let mut fields = json!({});

    macro_rules! read_body {
        ($ty:ty $(, $field:ident)?) => {{
            let (body, signature) = <$ty>::read_signed(&mut slice)?;
            fields["seqno"] = json!(body.msg_seqno);
            fields["valid_until"] = json!(body.valid_until);
            $(fields["wallet_id"] = json!(body.$field);)?
            (body.to_cell()?, signature, body.msgs)
        }};
    }

    let (unsigned, signature, messages) = match version {
        WalletVersion::V2R1 | WalletVersion::V2R2 => read_body!(WalletV2ExtMsgBody),
        WalletVersion::V3R1 | WalletVersion::V3R2 => read_body!(WalletV3ExtMsgBody, subwallet_id),
        WalletVersion::V4R1 | WalletVersion::V4R2 => read_body!(WalletV4ExtMsgBody, subwallet_id),
        WalletVersion::V5R1 => read_body!(WalletV5ExtMsgBody, wallet_id),
        _ => bail!("no transfer fixture decoder for {version:?}"),
    };
    ensure_empty(slice)?;
    VerifyingKey::from_bytes(public_key.as_array())?.verify_strict(
        unsigned.repr_hash().as_slice(),
        &Signature::from_slice(&signature)?,
    )?;

    let mut signed = CellBuilder::new();
    if version == WalletVersion::V5R1 {
        signed.store_slice(unsigned.as_slice()?)?;
        signed.store_raw(&signature, 512)?;
    } else {
        signed.store_raw(&signature, 512)?;
        signed.store_slice(unsigned.as_slice()?)?;
    }
    ensure!(
        signed.build()?.repr_hash() == cell.repr_hash(),
        "signed body roundtrip"
    );

    let mut outgoing = Vec::new();
    for message in messages {
        let parsed: OwnedRelaxedMessage = load_exact(&message.msg)?;
        let body = parsed.body.0.apply(&parsed.body.1)?;
        let opcode = if body.size_bits() >= 32 {
            Some(body.get_u32(0)?)
        } else {
            None
        };
        let mut summary = json!({
            "mode": message.mode.bits(),
            "message_hash": message.msg.repr_hash(),
            "state_init": parsed.init.is_some(),
            "body_bits": body.size_bits(),
            "body_refs": body.size_refs(),
            "opcode": opcode.map(|value| format!("{value:08x}")),
        });
        match parsed.info {
            RelaxedMsgInfo::Int(info) => {
                summary["destination"] = json!(info.dst.to_string());
                summary["nanograms"] = json!(info.value.tokens.to_string());
                summary["bounce"] = json!(info.bounce);
            }
            RelaxedMsgInfo::ExtOut(info) => {
                summary["external_destination"] = json!(info.dst);
            }
        }
        outgoing.push(summary);
    }
    fields["messages"] = json!(outgoing);
    Ok(fields)
}
