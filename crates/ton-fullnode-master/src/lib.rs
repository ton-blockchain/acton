//! Serves hand-built blocks to unmodified TON nodes over the full-node protocol.
//!
//! A node reads blocks from `db/static` only through
//! `run_hardfork_accept_block_query`, which refuses anything outside the
//! masterchain (`accept-block.cpp:392`). A hand-built *shard* block therefore
//! cannot be grafted through the filesystem at all; it has to arrive the way any
//! other shard block does — over the network.
//!
//! The cheapest network path into a stock node is its full-node *master*. When
//! `fullnodeslaves` is set in the engine config, `FullNodeImpl::get_query_sender`
//! routes every download query to that master over ADNL-over-TCP — the same
//! transport the liteserver uses, which [`ton_liteapi`] already implements. The
//! answers travel the ordinary `DownloadBlockNew` path, so the block is accepted
//! as a normal download instead of a fork, and only its `BlockProof` link is
//! verified.
//!
//! This server implements just enough of that master: it answers
//! `tonNode.getCapabilities`, hands out the blocks it was given, and reports
//! "not found" for everything else so the node falls back to its own data.

pub mod tl;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::bytes::Bytes;
use ton_liteapi::adnl::AdnlPeer;
use ton_liteapi::adnl::crypto::{KeyPair, SecretKey};
use tycho_types::models::block::BlockId;

use self::tl::{Answer, BlockIdExt, Query, TON_NODE_QUERY_PREFIX};

/// Protocol version reported to the node in `tonNode.capabilities`.
///
/// These are the values a stock full node reports; the slave only uses them to
/// decide which query flavours it may send.
const PROTO_VERSION_MAJOR: i32 = 2;
const PROTO_VERSION_MINOR: i32 = 0;
const PROTO_CAPABILITIES: u32 = 1;

/// One block this server is willing to hand out.
#[derive(Debug, Clone)]
pub struct ServedBlock {
    /// Serialized block `BoC`.
    pub data: Vec<u8>,
    /// Serialized `BlockProof` link for that block.
    pub proof_link: Vec<u8>,
}

/// Blocks a node may download from this process.
///
/// The registry is shared with the running server, so blocks can be added
/// between grafts without restarting it.
#[derive(Clone, Default)]
pub struct BlockSource {
    blocks: Arc<RwLock<HashMap<BlockIdExt, ServedBlock>>>,
}

impl BlockSource {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes one block for download.
    pub async fn insert(&self, id: &BlockId, block: ServedBlock) {
        self.blocks.write().await.insert(block_id_to_tl(id), block);
    }

    /// Removes one block from the registry.
    pub async fn remove(&self, id: &BlockId) {
        self.blocks.write().await.remove(&block_id_to_tl(id));
    }

    /// Serves the registry on `addr` until the returned future is dropped.
    ///
    /// `secret` is the ed25519 private key whose public half has to be written
    /// into the `fullnodeslaves` entry of every node that should reach it.
    pub async fn serve(self, addr: SocketAddr, secret: [u8; 32]) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind block source on {addr}"))?;
        self.serve_listener(listener, secret).await
    }

    /// Serves an already-bound listener. Bind errors are reported before a node starts.
    pub async fn serve_listener(
        self,
        listener: TcpListener,
        secret: [u8; 32],
    ) -> anyhow::Result<()> {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            let (socket, peer) = listener
                .accept()
                .await
                .context("Block source accept failed")?;
            let source = self.clone();
            connections.spawn(async move {
                if let Err(error) = source.handle(socket, secret).await {
                    tracing::debug!(%peer, %error, "block source connection closed");
                }
            });
            while connections.try_join_next().is_some() {}
        }
    }

    /// Runs one ADNL-over-TCP connection from a node.
    async fn handle(&self, socket: TcpStream, secret: [u8; 32]) -> anyhow::Result<()> {
        let mut peer = AdnlPeer::handle_handshake(socket, move |_| {
            Some(KeyPair::from(&SecretKey::from_bytes(secret)))
        })
        .await
        .map_err(|error| anyhow!("ADNL handshake failed: {error}"))?;

        while let Some(packet) = peer.next().await {
            let packet = packet.map_err(|error| anyhow!("ADNL read failed: {error}"))?;
            let Some(response) = self.respond(&packet).await? else {
                continue;
            };
            peer.send(response)
                .await
                .map_err(|error| anyhow!("ADNL write failed: {error}"))?;
        }
        Ok(())
    }

    /// Builds the ADNL response for one inbound ADNL packet.
    async fn respond(&self, packet: &[u8]) -> anyhow::Result<Option<Bytes>> {
        use ton_liteapi::tl::adnl::Message;

        let Ok(message) = tl_proto::deserialize::<Message>(packet) else {
            return Ok(None);
        };
        match message {
            Message::Query { query_id, query } => {
                let Some(answer) = self.answer(&query).await else {
                    // A stock master simply drops queries it cannot serve; the
                    // node then falls back to its other sources.
                    return Ok(None);
                };
                Ok(Some(
                    tl_proto::serialize(Message::Answer { query_id, answer }).into(),
                ))
            }
            Message::Ping { random_id } => Ok(Some(
                tl_proto::serialize(Message::Pong { random_id }).into(),
            )),
            // A slave authenticates itself before it considers the connection
            // usable: it signs its own nonce concatenated with ours. The master
            // does not restrict who may connect, so the exchange only has to
            // complete, not to authorize anything, and the signed completion
            // needs no answer.
            Message::Authenticate { .. } => {
                let mut nonce = vec![0u8; 256];
                getrandom(&mut nonce).context("Failed to draw an authentication nonce")?;
                Ok(Some(
                    tl_proto::serialize(Message::AuthenticationNonce { nonce }).into(),
                ))
            }
            _ => Ok(None),
        }
    }

    /// Answers one `tonNode` query, or `None` when it cannot be served.
    async fn answer(&self, query: &[u8]) -> Option<Vec<u8>> {
        // Every full-node query is prefixed with the bare `tonNode.query` id.
        let body = query.strip_prefix(&TON_NODE_QUERY_PREFIX.to_le_bytes())?;
        let query = tl_proto::deserialize::<Query>(body).ok()?;

        let answer = match query {
            Query::GetCapabilities => Answer::Capabilities {
                version_major: PROTO_VERSION_MAJOR,
                version_minor: PROTO_VERSION_MINOR,
                flags: PROTO_CAPABILITIES,
            },
            Query::DownloadBlockFull { block } => match self.blocks.read().await.get(&block) {
                Some(served) => Answer::DataFull {
                    id: block,
                    proof: served.proof_link.clone(),
                    block: served.data.clone(),
                    // The proof is a link, not a signed masterchain proof; the
                    // node accepts that for shard blocks only.
                    is_link: true,
                },
                None => Answer::DataFullEmpty,
            },
            Query::DownloadBlock { block } => Answer::Data {
                data: self.blocks.read().await.get(&block)?.data.clone(),
            },
            Query::DownloadBlockProofLink { block } => Answer::Data {
                data: self.blocks.read().await.get(&block)?.proof_link.clone(),
            },
            Query::PrepareBlock { block } => {
                if self.blocks.read().await.contains_key(&block) {
                    Answer::Prepared
                } else {
                    Answer::NotFound
                }
            }
            Query::PrepareBlockProof { block, .. } => {
                if self.blocks.read().await.contains_key(&block) {
                    Answer::PreparedProofLink
                } else {
                    Answer::PreparedProofEmpty
                }
            }
            Query::GetNextKeyBlockIds { .. } => Answer::KeyBlocks {
                blocks: Vec::new(),
                incomplete: true,
                error: false,
            },
            Query::GetNextBlockDescription { .. } => Answer::BlockDescriptionEmpty,
            Query::DownloadNextBlockFull { .. } => Answer::DataFullEmpty,
            Query::DownloadBlockProof { .. } => return None,
            // A slave forwards its outbound external messages here. There is
            // nowhere to forward them to, and the node only needs the query to
            // succeed.
            Query::SlaveSendExtMessage { .. } => Answer::Success,
        };
        Some(tl_proto::serialize(answer))
    }
}

/// Fills a buffer with operating-system randomness.
fn getrandom(buffer: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buffer)
}

/// Converts a typed block id into its wire representation.
const fn block_id_to_tl(id: &BlockId) -> BlockIdExt {
    BlockIdExt {
        workchain: id.shard.workchain(),
        shard: id.shard.prefix(),
        seqno: id.seqno,
        root_hash: id.root_hash.0,
        file_hash: id.file_hash.0,
    }
}

#[cfg(test)]
mod tests {
    use tycho_types::models::block::ShardIdent;
    use tycho_types::prelude::HashBytes;

    use super::*;

    fn block_id(seqno: u32) -> BlockId {
        BlockId {
            shard: ShardIdent::BASECHAIN,
            seqno,
            root_hash: HashBytes([1; 32]),
            file_hash: HashBytes([2; 32]),
        }
    }

    fn query(payload: &[u8]) -> Vec<u8> {
        let mut query = TON_NODE_QUERY_PREFIX.to_le_bytes().to_vec();
        query.extend_from_slice(payload);
        query
    }

    #[tokio::test]
    async fn capabilities_are_reported_without_any_published_block() {
        let source = BlockSource::new();
        let answer = source
            .answer(&query(&tl_proto::serialize(Query::GetCapabilities)))
            .await
            .expect("capabilities are always answered");

        assert_eq!(
            tl_proto::deserialize::<Answer>(&answer).unwrap(),
            Answer::Capabilities {
                version_major: PROTO_VERSION_MAJOR,
                version_minor: PROTO_VERSION_MINOR,
                flags: PROTO_CAPABILITIES,
            }
        );
    }

    #[tokio::test]
    async fn published_block_is_returned_with_its_proof_link() {
        let source = BlockSource::new();
        let id = block_id(7);
        source
            .insert(
                &id,
                ServedBlock {
                    data: b"block".to_vec(),
                    proof_link: b"proof".to_vec(),
                },
            )
            .await;

        let request = Query::DownloadBlockFull {
            block: block_id_to_tl(&id),
        };
        let answer = source
            .answer(&query(&tl_proto::serialize(request)))
            .await
            .expect("a published block is served");

        assert_eq!(
            tl_proto::deserialize::<Answer>(&answer).unwrap(),
            Answer::DataFull {
                id: block_id_to_tl(&id),
                proof: b"proof".to_vec(),
                block: b"block".to_vec(),
                // A shard block only ever has a proof link, never a signed proof.
                is_link: true,
            }
        );
    }

    #[tokio::test]
    async fn unknown_block_is_reported_as_missing_instead_of_failing() {
        let source = BlockSource::new();
        let request = Query::DownloadBlockFull {
            block: block_id_to_tl(&block_id(9)),
        };
        let answer = source
            .answer(&query(&tl_proto::serialize(request)))
            .await
            .expect("a missing block still gets an answer");

        assert_eq!(
            tl_proto::deserialize::<Answer>(&answer).unwrap(),
            Answer::DataFullEmpty
        );
    }

    #[tokio::test]
    async fn queries_without_the_full_node_prefix_are_ignored() {
        let source = BlockSource::new();
        assert!(
            source
                .answer(&tl_proto::serialize(Query::GetCapabilities))
                .await
                .is_none()
        );
    }
}
