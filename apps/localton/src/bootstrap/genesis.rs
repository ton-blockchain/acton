//! One-time creation of a persistent local TON network.
//!
//! A fresh state directory needs validator keys, zerostates, static-state files,
//! DHT descriptors, validator-engine databases, console credentials, a
//! liteserver identity, and a manifest tying them together. This module performs
//! those operations in dependency order and leaves a reusable network state.

use std::{fs, io::ErrorKind, time::Duration};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use tracing::info;

use crate::{
    binaries::TonBinaries,
    storage::Settings,
    storage::{Layout, Manifest, SCHEMA_VERSION, TON_RELEASE, global_config, write_json_atomic},
    ton::accounts::ImportedAccount,
};

use super::{dht, files::copy_tree, keys::generate_key, validator, zerostate};

/// Creates every persistent artifact that defines a new local TON network.
///
/// Zerostate hashes, validator identity, DHT descriptors, liteserver key, and
/// global config are mutually dependent, so they must be produced in this exact
/// order. The manifest is saved last and acts as the commit marker: without it,
/// a later run treats the directory as an interrupted bootstrap and rebuilds it.
pub(super) async fn initialize(
    layout: &Layout,
    binaries: &TonBinaries,
    settings: &Settings,
    imported_accounts: &[ImportedAccount],
    startup_timeout: Duration,
) -> Result<Manifest> {
    info!("initializing a new local TON genesis");
    let genesis = settings
        .node("genesis")
        .context("settings contain no genesis node")?;

    // Step 1: discard only output from an interrupted bootstrap, then copy the
    // official smart-contract and Fift support files used by `create-state`.
    // This branch is entered only when no completed manifest exists.
    clean_partial_bootstrap(layout)?;
    layout.create_dirs()?;
    copy_tree(&binaries.smartcont_dir(), &layout.smartcont)?;

    // Step 2: create the permanent validator key before rendering zerostate.
    // Its public key is embedded into the initial validator set, which makes the
    // corresponding private key the only identity able to produce first blocks.
    let validator_key = generate_key(binaries, &layout.validator_keyring.join("validator")).await?;
    fs::copy(
        &validator_key.private_path,
        layout.validator_keyring.join(&validator_key.id_hex),
    )?;
    let validator_public = fs::read(&validator_key.public_path)?;
    ensure!(
        validator_public.len() == 36,
        "unexpected validator public key length: {}",
        validator_public.len()
    );
    fs::write(
        layout.smartcont.join("validator-keys-1.pub"),
        &validator_public[4..],
    )?;
    zerostate::create_zero_state(
        layout,
        binaries,
        &settings.network,
        imported_accounts,
        startup_timeout,
    )
    .await?;

    // Step 3: install masterchain and basechain zerostates as static states.
    // TON identifies an initial state by both representation/root hash and file
    // hash; global config publishes these hashes while validator-engine reads the
    // matching BoCs from its hash-named `db/static` files.
    let zero_root_hash = zerostate::read_hash_base64(&layout.zerostate.join("zerostate.rhash"))?;
    let zero_file_bytes = zerostate::read_hash(&layout.zerostate.join("zerostate.fhash"))?;
    let zero_file_hash = BASE64.encode(&zero_file_bytes);
    zerostate::install_static_state(layout, "zerostate.boc", &zero_file_bytes)?;
    let base_file_bytes = zerostate::read_hash(&layout.zerostate.join("basestate0.fhash"))?;
    zerostate::install_static_state(layout, "basestate0.boc", &base_file_bytes)?;

    // Step 4: create three separate authentication roles.
    // `server` identifies validator-engine's control endpoint, `client` is
    // allowed to administer that endpoint, and `liteserver` is the public key
    // applications use to authenticate lite-protocol responses.
    let server = generate_key(binaries, &layout.certs.join("server")).await?;
    fs::copy(
        &server.private_path,
        layout.validator_keyring.join(&server.id_hex),
    )?;
    let client = generate_key(binaries, &layout.certs.join("client")).await?;
    let liteserver = generate_key(binaries, &layout.validator_keyring.join("liteserver")).await?;
    fs::copy(
        &liteserver.private_path,
        layout.validator_keyring.join(&liteserver.id_hex),
    )?;
    let liteserver_public = fs::read(&liteserver.public_path)?;
    ensure!(
        liteserver_public.len() == 36,
        "unexpected liteserver public key length: {}",
        liteserver_public.len()
    );
    let liteserver_public_key = BASE64.encode(&liteserver_public[4..]);

    // Step 5: break the DHT/global-config dependency cycle in two passes.
    // dht-server needs a global config to initialize its database, but the final
    // global config must itself contain the signed descriptor created from that
    // database. The preliminary config therefore has an empty DHT node list.
    let preliminary = global_config(
        &zero_root_hash,
        &zero_file_hash,
        vec![],
        &liteserver_public_key,
    );
    write_json_atomic(&layout.global_config, &preliminary)?;
    let dht_nodes = dht::initialize_dht(layout, binaries, genesis, startup_timeout).await?;
    let global = global_config(
        &zero_root_hash,
        &zero_file_hash,
        dht_nodes,
        &liteserver_public_key,
    );
    write_json_atomic(&layout.global_config, &global)?;
    fs::copy(
        &layout.global_config,
        layout.validator_db.join("global.config.json"),
    )?;

    // Step 6: let validator-engine create its database and generated config,
    // patch in local control/liteserver endpoints, then use the live console to
    // register permanent validator and ADNL identities in the engine keyring.
    validator::initialize(layout, binaries, genesis, startup_timeout).await?;
    validator::configure_local_services(layout, genesis, &server, &client, &liteserver)?;
    validator::configure_genesis_identity(
        layout,
        binaries,
        genesis,
        &validator_key,
        startup_timeout,
    )
    .await?;

    // Step 7: commit bootstrap by saving immutable network identity.
    // Future runs use this manifest to reuse exactly the same zerostate, keys,
    // global config, and imported account snapshots.
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        ton_release: TON_RELEASE.to_owned(),
        ton_bin_dir: Some(binaries.root.clone()),
        validator_id_hex: validator_key.id_hex,
        validator_id_base64: validator_key.id_base64,
        liteserver_public_key,
        global_config: layout.global_config.clone(),
        imported_accounts: imported_accounts
            .iter()
            .map(|account| account.descriptor.clone())
            .collect(),
    };
    manifest.save_atomic(&layout.manifest)?;
    info!("genesis initialization complete");
    Ok(manifest)
}

/// Removes artifacts that may be internally inconsistent after interrupted genesis.
///
/// This is called only when the manifest is absent. User settings and downloaded
/// binaries remain intact, while genesis, DHT, and global-config output are
/// recreated as one coherent set.
fn clean_partial_bootstrap(layout: &Layout) -> Result<()> {
    for path in [&layout.genesis, &layout.dht_db, &layout.global_config] {
        match if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        } {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to clean partial state {}", path.display()));
            }
        }
    }
    Ok(())
}
