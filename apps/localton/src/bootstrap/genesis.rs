//! One-time creation of a persistent local TON network.
//!
//! A fresh state directory needs validator keys, zerostates, static-state files,
//! DHT descriptors, validator-engine databases, console credentials, a
//! liteserver identity, and a manifest tying them together. This module performs
//! those operations in dependency order and leaves a reusable network state.

use std::{fs, io::ErrorKind, time::Duration};

use anyhow::{Context, Result};
use tracing::info;

use crate::{
    storage::Settings,
    storage::{Layout, Manifest, SCHEMA_VERSION, TON_RELEASE, write_json_atomic},
    ton::accounts::ImportedAccount,
    ton::{
        global_config::GlobalConfig,
        toolchain::Toolchain,
        tools::{
            random_id::GenerateKeyRequest, types::OperationContext,
            validator_engine::ValidatorInitializeRequest,
        },
    },
};

use super::{dht, files::copy_tree, validator, zerostate};

/// Creates every persistent artifact that defines a new local TON network.
///
/// Zerostate hashes, validator identity, DHT descriptors, liteserver key, and
/// global config are mutually dependent, so they must be produced in this exact
/// order. The manifest is saved last and acts as the commit marker: without it,
/// a later run treats the directory as an interrupted bootstrap and rebuilds it.
pub(super) async fn initialize(
    layout: &Layout,
    tools: &Toolchain,
    settings: &Settings,
    imported_accounts: &[ImportedAccount],
    startup_timeout: Duration,
) -> Result<Manifest> {
    info!("initializing a new local TON genesis");
    let genesis = settings
        .node("genesis")
        .context("settings contain no genesis node")?;
    let context = OperationContext::for_node(startup_timeout, &genesis.name);

    // Step 1: discard only output from an interrupted bootstrap, then copy the
    // official smart-contract and Fift support files used by `create-state`.
    // This branch is entered only when no completed manifest exists.
    clean_partial_bootstrap(layout)?;
    layout.create_dirs()?;
    copy_tree(&tools.binaries.smartcont_dir(), &layout.smartcont)?;

    // Step 2: create the permanent validator key before rendering zerostate.
    // Its public key is embedded into the initial validator set, which makes the
    // corresponding private key the only identity able to produce first blocks.
    let validator_key = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::validator(&layout.validator_keyring),
        )
        .await?;

    // The official create-state Fift resources read this conventional filename
    // while assembling the masterchain zerostate. Its raw 32-byte key becomes the
    // initial validator set, so the validator-engine identity created above can
    // produce the first blocks immediately after genesis.
    fs::write(
        layout.smartcont.join("validator-keys-1.pub"),
        validator_key.public_key.as_bytes(),
    )?;

    let genesis_states = zerostate::create_genesis_states(
        layout,
        tools.create_state.as_ref(),
        &context,
        &settings.network,
        imported_accounts,
    )
    .await?;

    // Step 3: create three separate authentication roles.
    // `server` identifies validator-engine's control endpoint, `client` is
    // allowed to administer that endpoint, and `liteserver` is the public key
    // applications use to authenticate lite-protocol responses.
    let server = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::control_server(&layout.certs, &layout.validator_keyring),
        )
        .await?;
    let client = tools
        .random_id
        .generate_key(&context, GenerateKeyRequest::control_client(&layout.certs))
        .await?;
    let liteserver = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::liteserver(&layout.validator_keyring),
        )
        .await?;

    // Step 4: break the DHT/global-config dependency cycle in two passes.
    // dht-server needs a global config to initialize its database, but the final
    // global config must itself contain the signed descriptor created from that
    // database. The preliminary config therefore has an empty DHT node list.
    let preliminary = GlobalConfig::local(
        genesis_states.masterchain.id(),
        vec![],
        genesis.public_ip,
        genesis.liteserver_port,
        liteserver.public_key,
    );
    write_json_atomic(&layout.global_config, &preliminary)?;

    let initialized_dht = dht::initialize_dht(
        layout,
        tools.dht_server.as_ref(),
        tools.random_id.as_ref(),
        genesis,
        &context,
    )
    .await?;

    let global = GlobalConfig::local(
        genesis_states.masterchain.id(),
        initialized_dht.descriptors,
        genesis.public_ip,
        genesis.liteserver_port,
        liteserver.public_key,
    );
    write_json_atomic(&layout.global_config, &global)?;

    fs::copy(
        &layout.global_config,
        layout.validator_db.join("global.config.json"),
    )?;

    // Step 5: let validator-engine create its database and generated config,
    // patch in local control/liteserver endpoints, then use the live console to
    // register permanent validator and ADNL identities in the engine keyring.
    let validator_database = tools
        .validator_engine
        .initialize(
            &context,
            ValidatorInitializeRequest::for_node(layout, genesis),
        )
        .await?;
    validator_database.install_control_and_liteserver(
        genesis,
        server.id,
        client.id,
        liteserver.id,
    )?;
    validator::configure_genesis_identity(
        layout,
        tools.validator_engine.as_ref(),
        tools.validator_console_tool.as_ref(),
        genesis,
        &validator_key,
        &context,
    )
    .await?;

    // Step 6: commit bootstrap by saving immutable network identity.
    // Future runs use this manifest to reuse exactly the same zerostate, keys,
    // global config, and imported account snapshots.
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        ton_release: TON_RELEASE.to_owned(),
        ton_bin_dir: tools.binaries.root.clone(),
        validator_public_key: validator_key.public_key,
        liteserver_public_key: liteserver.public_key,
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
