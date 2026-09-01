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
    node,
    storage::Settings,
    storage::{Layout, Manifest, NodeManifest, SCHEMA_VERSION, TON_RELEASE},
    ton::accounts::ImportedAccount,
    ton::{
        global_config::GlobalConfig,
        toolchain::Toolchain,
        tools::{random_id::GenerateKeyRequest, types::OperationContext},
    },
};

use super::{dht, files::copy_tree, zerostate};

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
    let genesis = &settings.node;
    let node_layout = &layout.node;
    let context = OperationContext::for_node(startup_timeout, &genesis.name);

    // Step 1: discard only output from an interrupted bootstrap, then copy the
    // official smart-contract and Fift support files used by `create-state`.
    // This branch is entered only when no completed manifest exists.
    clean_partial_bootstrap(layout)?;
    layout.create_bootstrap_dirs()?;
    copy_tree(&tools.binaries.smartcont_dir(), &layout.smartcont)?;

    // Step 2: create the permanent validator key before rendering zerostate.
    // Its public key is embedded into the initial validator set, which makes the
    // corresponding private key the only identity able to produce first blocks.
    let validator_key = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::validator(&node_layout.keyring),
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

    // Step 3: create independent control and liteserver authentication roles
    // through the same node lifecycle used by joined nodes.
    let service_keys = node::generate_service_keys(node_layout, tools, genesis, &context).await?;
    let liteserver = service_keys
        .liteserver
        .as_ref()
        .context("genesis node must expose a liteserver")?;

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
    preliminary.save_atomic(&layout.global_config)?;

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
    global.save_atomic(&layout.global_config)?;

    // Step 5: let validator-engine create its database and generated config,
    // patch in local control/liteserver endpoints, then use the live console to
    // register permanent validator and ADNL identities in the engine keyring.
    node::initialize_database(layout, node_layout, tools, genesis, &service_keys, &context).await?;
    let identity = node::configure_genesis_identity(
        node_layout,
        tools.validator_engine.as_ref(),
        tools.validator_console_tool.as_ref(),
        genesis,
        &validator_key,
        &context,
    )
    .await?;

    // The node marker is committed only after every engine-owned identity is
    // configured. The outer network manifest below remains bootstrap's final
    // all-or-nothing boundary.
    NodeManifest::new(
        &genesis.name,
        service_keys.server.public_key,
        Some(liteserver.public_key),
        identity.full_node_adnl,
        Some(validator_key.public_key),
        Some(identity.validator_adnl),
    )
    .save_atomic(&node_layout.manifest)?;

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
    for path in [
        &layout.node.root,
        &layout.genesis,
        &layout.dht_db,
        &layout.global_config,
    ] {
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
