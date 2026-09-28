//! Outbound external messages through TON's public workchain overlays.

#[cfg(test)]
mod tests;

mod broadcast;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use everscale_network::{adnl, overlay};
use rand::seq::SliceRandom;
use rston::{
    boc::Boc,
    cell::{Cell, HashBytes},
    models::{IntAddr, MsgInfo, OwnedMessage, StdAddr},
};
use tokio::{sync::Mutex, time::Instant};
use tracing::info;

use crate::network::Network;

/// A structurally checked inbound external message, retaining its original `BoC`.
///
/// Parsing does not execute the destination contract or validate its signature,
/// balance, sequence number, or expiry. Receiving peers enforce those checks.
pub struct ExternalMessage {
    boc: Vec<u8>,
    destination: StdAddr,
    hash: HashBytes,
    root: Cell,
}

impl ExternalMessage {
    /// Maximum accepted serialized `BoC` size. This is a local admission limit;
    /// the destination network may enforce stricter message limits.
    pub const MAX_BYTES: usize = 65_535;

    /// Requires one ordinary, level-zero message root and a standard destination
    /// in masterchain or basechain. Internal messages cannot be submitted here.
    pub fn new(boc: Vec<u8>) -> Result<Self> {
        ensure!(
            boc.len() <= Self::MAX_BYTES,
            "external message exceeds 65535 bytes"
        );
        let root = Boc::decode(&boc).context("invalid external message BoC")?;
        ensure!(
            !root.is_exotic() && root.level() == 0 && root.repr_depth() < 512,
            "external message must have an ordinary level-zero root with depth below 512"
        );
        let message = root
            .parse::<OwnedMessage>()
            .context("invalid message layout")?;
        let MsgInfo::ExtIn(info) = message.info else {
            bail!("expected an inbound external message");
        };
        let IntAddr::Std(destination) = info.dst else {
            bail!("external message requires a standard destination address");
        };
        ensure!(
            destination.anycast.is_none(),
            "anycast destinations are not supported"
        );
        ensure!(
            matches!(destination.workchain, -1 | 0),
            "external message destination must be in masterchain or basechain"
        );

        Ok(Self {
            hash: *root.repr_hash(),
            destination,
            boc,
            root,
        })
    }

    /// Destination used to select the public workchain overlay.
    #[must_use]
    pub const fn destination(&self) -> &StdAddr {
        &self.destination
    }

    /// Representation hash of the original message root, not a normalized hash.
    #[must_use]
    pub const fn hash(&self) -> HashBytes {
        self.hash
    }

    /// Original validated root for inspecting the message or computing a lookup
    /// key. Broadcasting still uses the original bytes, preserving the signature.
    #[must_use]
    pub const fn root(&self) -> &Cell {
        &self.root
    }
}

/// Cloneable outbound handle sharing one UDP transport and discovery cache.
///
/// Sending queues a broadcast to discovered peers; there is no delivery receipt
/// or inclusion guarantee. Callers must observe the resulting transaction.
#[derive(Clone)]
pub struct MessageSender {
    network: Arc<Network>,
    peers: Arc<Mutex<BTreeMap<i8, CachedPeers>>>,
    timeout: Duration,
}

struct CachedPeers {
    ids: Vec<adnl::NodeIdShort>,
    discovered: Instant,
}

impl MessageSender {
    pub(crate) fn new(network: Arc<Network>, timeout: Duration) -> Self {
        Self {
            network,
            peers: Arc::new(Mutex::new(BTreeMap::new())),
            timeout,
        }
    }

    /// Discovers signed overlay members, then sends ordinary or FEC broadcast
    /// to at most five peers. Receiving peers deduplicate repeated broadcasts.
    /// The deadline covers peer lookup.
    pub async fn send(&self, message: ExternalMessage) -> Result<()> {
        let started = Instant::now();
        let workchain = message.destination.workchain;
        let overlay_id = overlay::IdFull::for_workchain_overlay(
            i32::from(workchain),
            &self.network.zero_state_file_hash,
        )
        .compute_short_id();
        let mut peers = tokio::time::timeout(self.timeout, async {
            let mut cache = self.peers.lock().await;
            let stale = cache
                .get(&workchain)
                .is_none_or(|peers| peers.discovered.elapsed() >= Duration::from_secs(60));
            if stale {
                let found = self.network.find_overlay_peers(&overlay_id).await?;
                ensure!(
                    !found.is_empty(),
                    "no peers found for external message overlay {overlay_id}"
                );
                cache.insert(
                    workchain,
                    CachedPeers {
                        ids: found.into_iter().map(|peer| peer.id).collect(),
                        discovered: Instant::now(),
                    },
                );
            }

            Ok::<_, anyhow::Error>(cache[&workchain].ids.clone())
        })
        .await
        .context("external message peer discovery timed out")??;

        peers.shuffle(&mut rand::thread_rng());
        peers.truncate(5);

        // Full nodes join parent overlays too, so the public workchain root
        // continues to route external messages when the shard frontier splits.
        let data = tl_proto::serialize(ExternalMessageBroadcast { data: message.boc });
        let key = self.network.dht.key().clone();
        let packets =
            tokio::task::spawn_blocking(move || broadcast::encode(&key, &overlay_id, &data))
                .await
                .context("external message broadcast encoding failed")??;

        for (index, packet) in packets.iter().enumerate() {
            for peer in &peers {
                self.network
                    .adnl
                    .send_custom_message(self.network.dht.key().id(), peer, packet)
                    .with_context(|| format!("cannot send external message to {peer}"))?;
            }

            if (index + 1) % 20 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        info!(
            operation = "external_message_broadcast",
            target = %message.destination,
            hash = %message.hash,
            overlay = %overlay_id,
            recipients = peers.len(),
            packets = packets.len(),
            duration_ms = started.elapsed().as_millis(),
            outcome = "queued",
            "submitted external message to the P2P transport",
        );
        Ok(())
    }
}

#[derive(tl_proto::TlWrite)]
#[tl(
    boxed,
    id = "tonNode.externalMessageBroadcast",
    scheme_inline = "tonNode.externalMessageBroadcast message:tonNode.externalMessage = tonNode.Broadcast;"
)]
struct ExternalMessageBroadcast {
    // The nested tonNode.externalMessage is bare and contains only a bytes field.
    data: Vec<u8>,
}
