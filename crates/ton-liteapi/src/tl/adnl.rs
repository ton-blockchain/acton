use derivative::Derivative;
use tl_proto::{TlRead, TlWrite};

use crate::adnl::crypto::tl::PublicKeyOwned;

use super::common::*;

#[derive(TlRead, TlWrite, Derivative)]
#[derivative(Debug, Clone, PartialEq)]
#[tl(boxed)]
pub enum Message {
    /// adnl.message.query query_id:int256 query:bytes = adnl.Message;
    #[tl(id = 0xb48bf97a)]
    Query { query_id: Int256, query: Vec<u8> },

    /// adnl.message.answer query_id:int256 answer:bytes = adnl.Message;
    #[tl(id = 0x0fac8416)]
    Answer { query_id: Int256, answer: Vec<u8> },

    /// tcp.ping random_id:long = tcp.Pong;
    #[tl(id = 0x4d082b9a)]
    Ping { random_id: u64 },

    /// tcp.pong random_id:long = tcp.Pong;
    #[tl(id = 0xdc69fb03)]
    Pong { random_id: u64 },

    /// tcp.authentificate nonce:bytes = tcp.Message;
    #[tl(id = 0x445bab12)]
    Authenticate { nonce: Vec<u8> },

    /// tcp.authentificationNonce nonce:bytes = tcp.Message;
    #[tl(id = 0xe35d4ab6)]
    AuthenticationNonce { nonce: Vec<u8> },

    /// tcp.authentificationComplete key:PublicKey signature:bytes = tcp.Message;
    #[tl(id = 0xf7ad9ea6)]
    AuthenticationComplete {
        key: PublicKeyOwned,
        signature: Vec<u8>,
    },
}
