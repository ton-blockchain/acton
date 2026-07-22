use tl_proto::{TlRead, TlResult};

#[cfg(feature = "liteclient")]
use crate::liteclient::types::LiteError;

#[cfg(feature = "liteclient")]
use super::response::*;

pub fn lossy_read<'tl, T: TlRead<'tl>>(packet: &'tl [u8]) -> TlResult<Option<T>> {
    let mut slice = packet;
    let result = T::read_from(&mut slice);
    if let Ok(x) = result {
        Ok(Some(x))
    } else {
        Ok(None)
    }
}

pub fn fmt_string(bytes: &[u8], f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(
        f,
        "{}",
        std::string::String::from_utf8(bytes.to_vec()).unwrap()
    )
}

pub fn fmt_bytes(bytes: &[u8], f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(f, "0x{}", hex::encode(bytes))
}

pub fn fmt_opt_bytes<T: AsRef<[u8]>>(
    bytes: &Option<T>,
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    if let Some(bytes) = bytes {
        write!(f, "Some(0x{})", hex::encode(bytes))
    } else {
        write!(f, "None")
    }
}

pub mod struct_as_bytes {
    use tl_proto::{TlPacket, TlRead, TlResult, TlWrite};

    pub fn size_hint<T: TlWrite>(v: &T) -> usize {
        tl_proto::serialize(v).len()
    }

    pub fn write<P: TlPacket, T: TlWrite>(v: &T, packet: &mut P) {
        tl_proto::serialize(v).write_to(packet)
    }

    pub fn read<'tl, T: TlRead<'tl>>(packet: &'tl [u8]) -> TlResult<T> {
        let mut slice = packet;
        <&'tl [u8]>::read_from(&mut slice).and_then(|x| tl_proto::deserialize(x))
    }
}

#[cfg(feature = "liteclient")]
pub trait FromResponse: Sized {
    fn from_response(response: Response) -> Result<Self, LiteError>;
}

#[cfg(feature = "liteclient")]
impl FromResponse for MasterchainInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::MasterchainInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for MasterchainInfoExt {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::MasterchainInfoExt(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for CurrentTime {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::CurrentTime(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for Version {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::Version(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockData {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockData(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockState {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockState(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockHeader {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockHeader(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for SendMsgStatus {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::SendMsgStatus(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for AccountState {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::AccountState(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for RunMethodResult {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::RunMethodResult(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for ShardInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::ShardInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for AllShardsInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::AllShardsInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for TransactionInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::TransactionInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for TransactionList {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::TransactionList(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for TransactionId {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::TransactionId(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockTransactions {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockTransactions(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockTransactionsExt {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockTransactionsExt(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for PartialBlockProof {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::PartialBlockProof(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for ConfigInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::ConfigInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for ValidatorStats {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::ValidatorStats(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for LibraryResult {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::LibraryResult(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for LibraryResultWithProof {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::LibraryResultWithProof(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for ShardBlockProof {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::ShardBlockProof(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for LookupBlockResult {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::LookupBlockResult(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for OutMsgQueueSizes {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::OutMsgQueueSizes(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for BlockOutMsgQueueSize {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::BlockOutMsgQueueSize(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for DispatchQueueInfo {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::DispatchQueueInfo(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for DispatchQueueMessages {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::DispatchQueueMessages(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for NonfinalValidatorGroups {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::NonfinalValidatorGroups(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for NonfinalCandidate {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::NonfinalCandidate(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for NonfinalPendingShardBlocks {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::NonfinalPendingShardBlocks(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}

#[cfg(feature = "liteclient")]
impl FromResponse for Error {
    fn from_response(response: Response) -> Result<Self, LiteError> {
        match response {
            Response::Error(s) => Ok(s),
            _ => Err(LiteError::UnexpectedMessage),
        }
    }
}
