use aes::Aes256;
use ctr::Ctr128BE;

pub mod codec;
pub mod handshake;

pub type AdnlAes = Ctr128BE<Aes256>;
