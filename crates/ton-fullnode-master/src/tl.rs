//! The `tonNode` schema subset a full-node master has to speak.
//!
//! Constructor ids are the CRC-32 of the canonical TL declaration, exactly as in
//! `tl/generate/scheme/ton_api.tl`; each one is quoted above its variant so the
//! numbers stay checkable against the schema.

use tl_proto::{TlRead, TlWrite};

/// Block identifier as it travels over the full-node protocol.
///
/// `tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, TlRead, TlWrite)]
pub struct BlockIdExt {
    pub workchain: i32,
    pub shard: u64,
    pub seqno: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}

/// Queries a full-node master answers.
#[derive(Debug, Clone, PartialEq, Eq, TlRead, TlWrite)]
#[tl(boxed)]
pub enum Query {
    /// `tonNode.getCapabilities = tonNode.Capabilities`
    #[tl(id = 0xdee618f8)]
    GetCapabilities,

    /// `tonNode.downloadBlockFull block:tonNode.blockIdExt = tonNode.DataFull`
    #[tl(id = 0x6a27c49d)]
    DownloadBlockFull { block: BlockIdExt },

    /// `tonNode.downloadNextBlockFull prev_block:tonNode.blockIdExt = tonNode.DataFull`
    #[tl(id = 0x6ea0374a)]
    DownloadNextBlockFull { prev_block: BlockIdExt },

    /// `tonNode.prepareBlock block:tonNode.blockIdExt = tonNode.Prepared`
    #[tl(id = 0x75a37f4e)]
    PrepareBlock { block: BlockIdExt },

    /// `tonNode.downloadBlock block:tonNode.blockIdExt = tonNode.Data`
    #[tl(id = 0xe27279c3)]
    DownloadBlock { block: BlockIdExt },

    /// `tonNode.prepareBlockProof block:tonNode.blockIdExt allow_partial:Bool = tonNode.PreparedProof`
    #[tl(id = 0x875c3308)]
    PrepareBlockProof {
        block: BlockIdExt,
        allow_partial: bool,
    },

    /// `tonNode.downloadBlockProof block:tonNode.blockIdExt = tonNode.Data`
    #[tl(id = 0x4bd6478a)]
    DownloadBlockProof { block: BlockIdExt },

    /// `tonNode.downloadBlockProofLink block:tonNode.blockIdExt = tonNode.Data`
    #[tl(id = 0x25b300c6)]
    DownloadBlockProofLink { block: BlockIdExt },

    /// `tonNode.getNextKeyBlockIds block:tonNode.blockIdExt max_size:int = tonNode.KeyBlocks`
    #[tl(id = 0xf2e7cfbb)]
    GetNextKeyBlockIds { block: BlockIdExt, max_size: i32 },

    /// `tonNode.getNextBlockDescription prev_block:tonNode.blockIdExt = tonNode.BlockDescription`
    #[tl(id = 0x1455b0f3)]
    GetNextBlockDescription { prev_block: BlockIdExt },

    /// `tonNode.slave.sendExtMessage message:tonNode.externalMessage = tonNode.Success`
    #[tl(id = 0x0376f2a9)]
    SlaveSendExtMessage { message: ExternalMessage },
}

/// `tonNode.externalMessage data:bytes = tonNode.ExternalMessage`
#[derive(Debug, Clone, PartialEq, Eq, TlRead, TlWrite)]
#[tl(boxed, id = 0xdc75a209)]
pub struct ExternalMessage {
    pub data: Vec<u8>,
}

/// Answers a full-node master returns.
#[derive(Debug, Clone, PartialEq, Eq, TlRead, TlWrite)]
#[tl(boxed)]
pub enum Answer {
    /// `tonNode.capabilities#f5bf60c0 version_major:int version_minor:int flags:#`
    #[tl(id = 0xf5bf60c0)]
    Capabilities {
        version_major: i32,
        version_minor: i32,
        flags: u32,
    },

    /// `tonNode.dataFull id:tonNode.blockIdExt proof:bytes block:bytes is_link:Bool = tonNode.DataFull`
    #[tl(id = 0xbe589f93)]
    DataFull {
        id: BlockIdExt,
        proof: Vec<u8>,
        block: Vec<u8>,
        is_link: bool,
    },

    /// `tonNode.dataFullEmpty = tonNode.DataFull`
    #[tl(id = 0x576e85ca)]
    DataFullEmpty,

    /// `tonNode.data data:bytes = tonNode.Data`
    #[tl(id = 0x560a2484)]
    Data { data: Vec<u8> },

    /// `tonNode.prepared = tonNode.Prepared`
    #[tl(id = 0xeac4bbcd)]
    Prepared,

    /// `tonNode.notFound = tonNode.Prepared`
    #[tl(id = 0xe2c33da6)]
    NotFound,

    /// `tonNode.preparedProofLink = tonNode.PreparedProof`
    #[tl(id = 0x3dff328d)]
    PreparedProofLink,

    /// `tonNode.preparedProofEmpty = tonNode.PreparedProof`
    #[tl(id = 0xc769c17a)]
    PreparedProofEmpty,

    /// `tonNode.keyBlocks blocks:(vector tonNode.blockIdExt) incomplete:Bool error:Bool = tonNode.KeyBlocks`
    #[tl(id = 0x17286d4e)]
    KeyBlocks {
        blocks: Vec<BlockIdExt>,
        incomplete: bool,
        error: bool,
    },

    /// `tonNode.blockDescriptionEmpty = tonNode.BlockDescription`
    #[tl(id = 0x8384ae95)]
    BlockDescriptionEmpty,

    /// `tonNode.success = tonNode.Success`
    #[tl(id = 0xc096244f)]
    Success,
}

/// Constructor id of `tonNode.query = Object`, the prefix of every query.
pub const TON_NODE_QUERY_PREFIX: u32 = 0x69f324d3;
