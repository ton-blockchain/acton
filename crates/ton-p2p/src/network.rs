//! ADNL transport and DHT peer discovery for the masterchain overlay.

use std::{net::SocketAddrV4, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use everscale_network::{adnl, crypto, dht, overlay, proto};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep, timeout};
use ton_fullnode_master::tl::{Answer, Query};
use tracing::{info, warn};

use crate::{NetworkConfig, rldp};

/// UDP identity and request deadline shared by discovery and block downloads.
/// The advertised address must be reachable by peers; the listener binds its
/// port on all local IPv4 interfaces.
pub struct NetworkOptions {
    pub address: SocketAddrV4,
    /// Persistent ADNL identity, normally read with `load_identity`.
    pub secret_key: [u8; 32],
    /// Per-request deadline, or the total deadline for `bootstrap`.
    pub timeout: Duration,
}

/// Result of a successful discovery and capabilities request.
/// This confirms connectivity; it does not verify any chain data from the peer.
#[derive(Debug, Serialize)]
pub struct BootstrapReport {
    pub local_id: String,
    pub address: SocketAddrV4,
    pub overlay_id: String,
    pub peer_id: String,
    pub peer_address: SocketAddrV4,
    pub version_major: i32,
    pub version_minor: i32,
    pub flags: u32,
}

/// Discovers a masterchain peer and queries its full-node capabilities over ADNL.
///
/// The deadline covers discovery and the capabilities query together. The UDP
/// listener is stopped on success, error, or cancellation; the caller can retry
/// with the same persisted identity.
pub async fn bootstrap(config: &NetworkConfig, options: NetworkOptions) -> Result<BootstrapReport> {
    let started = Instant::now();
    let overlay_id = config.masterchain_overlay();

    ensure!(
        !options.timeout.is_zero(),
        "bootstrap timeout must be positive"
    );

    let network = Network::new(config, &options)?;
    let dht = &network.dht;

    info!(
        operation = "p2p_bootstrap",
        node = %dht.key().id(),
        target = %overlay_id,
        address = %network.adnl.socket_addr(),
        "discovering masterchain peers",
    );

    let result = timeout(options.timeout, discover_peer(&network)).await;

    match result {
        Ok(Ok(report)) => {
            info!(
                operation = "p2p_bootstrap",
                node = %report.local_id,
                target = %report.peer_id,
                duration_ms = started.elapsed().as_millis(),
                outcome = "connected",
                "masterchain peer connected",
            );

            Ok(report)
        }
        Ok(Err(error)) => {
            warn!(
                operation = "p2p_bootstrap",
                node = %dht.key().id(),
                target = %overlay_id,
                duration_ms = started.elapsed().as_millis(),
                outcome = "failed",
                %error,
                "masterchain peer discovery failed",
            );

            Err(error).context("masterchain peer discovery failed")
        }
        Err(_) => {
            warn!(
                operation = "p2p_bootstrap",
                node = %dht.key().id(),
                target = %overlay_id,
                duration_ms = started.elapsed().as_millis(),
                outcome = "timed_out",
                "masterchain peer discovery timed out",
            );

            bail!(
                "P2P bootstrap timed out after {:?} for overlay {overlay_id}",
                options.timeout
            )
        }
    }
}

/// Tries signed overlay members until one speaks the TON full-node protocol.
/// The caller owns the overall deadline and transport shutdown.
async fn discover_peer(network: &Network) -> Result<BootstrapReport> {
    let query = network.query_bytes(Query::GetCapabilities);

    loop {
        for peer in network.find_peers().await? {
            let peer_id = peer.id;
            let answer = network
                .adnl
                .query_raw(
                    network.dht.key().id(),
                    &peer_id,
                    query.clone().into(),
                    Some(2_000),
                )
                .await;

            let capabilities = match answer {
                Ok(Some(bytes)) => tl_proto::deserialize::<Answer>(&bytes),
                Ok(None) => continue,
                Err(error) => {
                    warn!(
                        operation = "p2p_bootstrap",
                        target = %peer_id,
                        %error,
                        "full-node query failed",
                    );

                    continue;
                }
            };

            match capabilities {
                Ok(Answer::Capabilities {
                    version_major,
                    version_minor,
                    flags,
                }) => {
                    return Ok(BootstrapReport {
                        local_id: network.dht.key().id().to_string(),
                        address: network.adnl.socket_addr(),
                        overlay_id: network.overlay_id.to_string(),
                        peer_id: peer_id.to_string(),
                        peer_address: peer.address,
                        version_major,
                        version_minor,
                        flags,
                    });
                }
                Ok(_) => warn!(
                    operation = "p2p_bootstrap",
                    target = %peer_id,
                    "unexpected full-node response type",
                ),
                Err(error) => warn!(
                    operation = "p2p_bootstrap",
                    target = %peer_id,
                    %error,
                    "invalid full-node response",
                ),
            }
        }

        sleep(Duration::from_millis(250)).await;
    }
}

/// Shared transport for a block source or bootstrap attempt. The last owner
/// shuts down ADNL, including its UDP listener.
pub(crate) struct Network {
    pub(crate) adnl: Arc<adnl::Node>,
    pub(crate) dht: Arc<dht::Node>,
    pub(crate) rldp: Arc<rldp::Client>,
    pub(crate) overlay_id: overlay::IdShort,
    _shutdown: Shutdown,
}

#[derive(Clone, Debug)]
pub(crate) struct Peer {
    pub(crate) id: adnl::NodeIdShort,
    pub(crate) address: SocketAddrV4,
    pub(crate) descriptor: PeerDescriptor,
}

/// The signed overlay identity and resolved address needed to reconnect over ADNL.
/// A saved descriptor is revalidated for this overlay before registering its key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PeerDescriptor {
    pub(crate) address: SocketAddrV4,
    public_key: String,
    version: u32,
    signature: String,
}

impl Network {
    pub(crate) fn new(config: &NetworkConfig, options: &NetworkOptions) -> Result<Self> {
        let keystore = adnl::Keystore::builder()
            .with_tagged_key(options.secret_key, 0)?
            .build();
        // TON peers use the ordinary ADNL channel. Everscale's priority-channel
        // extension otherwise drops initial queries until its fallback activates.
        let adnl_options = adnl::NodeOptions {
            force_use_priority_channels: false,
            ..Default::default()
        };
        let adnl = adnl::Node::new(options.address, keystore, adnl_options, None)?;
        let shutdown = Shutdown(Arc::clone(&adnl));
        let dht = dht::Node::new(Arc::clone(&adnl), 0, Default::default())?;
        let rldp = rldp::Client::new(&adnl)?;
        let mut accepted = 0;

        for node in config.dht_nodes() {
            match node.and_then(|node| dht.add_dht_peer(node)) {
                Ok(Some(_)) => accepted += 1,
                Ok(None) => warn!(
                    operation = "p2p_connect",
                    "rejected DHT bootstrap descriptor",
                ),
                Err(error) => {
                    warn!(
                        operation = "p2p_connect",
                        %error,
                        "invalid DHT bootstrap descriptor",
                    )
                }
            }
        }

        ensure!(accepted > 0, "no valid DHT bootstrap descriptors");
        adnl.start()?;

        Ok(Self {
            adnl,
            dht,
            rldp,
            overlay_id: config.masterchain_overlay(),
            _shutdown: shutdown,
        })
    }

    /// Prefixes each full-node query with the overlay ID expected by TON peers.
    pub(crate) fn query_bytes(&self, query: impl tl_proto::TlWrite) -> Vec<u8> {
        let mut bytes = tl_proto::serialize(OverlayQuery {
            overlay: *self.overlay_id.as_slice(),
        });
        bytes.extend(tl_proto::serialize(query));
        bytes
    }

    pub(crate) async fn find_peers(&self) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();

        for (address, node) in self.dht.find_overlay_nodes(&self.overlay_id).await? {
            match self.add_peer(address, &node) {
                Ok(Some(peer)) => peers.push(peer),
                Ok(None) => {}
                Err(error) => warn!(
                    operation = "p2p_discovery",
                    target = %address,
                    %error,
                    "invalid overlay descriptor",
                ),
            }
        }

        Ok(peers)
    }

    pub(crate) fn restore_peer(&self, descriptor: &PeerDescriptor) -> Result<Option<Peer>> {
        let key: [u8; 32] = STANDARD
            .decode(&descriptor.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("saved peer public key must contain 32 bytes"))?;
        let node = proto::overlay::NodeOwned {
            id: crypto::tl::PublicKeyOwned::Ed25519 { key },
            overlay: *self.overlay_id.as_slice(),
            version: descriptor.version,
            signature: STANDARD.decode(&descriptor.signature)?.into(),
        };
        self.add_peer(descriptor.address, &node)
    }

    fn add_peer(
        &self,
        address: SocketAddrV4,
        node: &proto::overlay::NodeOwned,
    ) -> Result<Option<Peer>> {
        self.overlay_id
            .verify_overlay_node(&node.as_equivalent_ref())?;
        let full_id = adnl::NodeIdFull::try_from(node.id.as_equivalent_ref())?;
        let id = full_id.compute_short_id();
        let crypto::tl::PublicKeyOwned::Ed25519 { key } = &node.id else {
            bail!("overlay peer must use an Ed25519 key");
        };

        if id == *self.dht.key().id() {
            return Ok(None);
        }

        self.adnl.add_peer(
            adnl::NewPeerContext::PublicOverlay,
            self.dht.key().id(),
            &id,
            address,
            full_id,
        )?;

        Ok(Some(Peer {
            id,
            address,
            descriptor: PeerDescriptor {
                address,
                public_key: STANDARD.encode(key),
                version: node.version,
                signature: STANDARD.encode(&node.signature),
            },
        }))
    }
}

#[derive(tl_proto::TlWrite)]
#[tl(
    boxed,
    id = "overlay.query",
    scheme_inline = "overlay.query overlay:int256 = True;"
)]
struct OverlayQuery {
    overlay: [u8; 32],
}

struct Shutdown(Arc<adnl::Node>);

impl Drop for Shutdown {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
