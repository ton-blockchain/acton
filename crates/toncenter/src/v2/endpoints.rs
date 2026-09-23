//! Endpoint metadata connects each request with its result type.
//!
//! These zero-sized markers do not perform I/O. Transports may use `Endpoint` to
//! construct typed calls while retaining ownership of authentication and retries.

use serde::{Serialize, de::DeserializeOwned};

/// One REST operation and the corresponding JSON-RPC method.
/// The associated response is the result inside `TonlibResponse`, not the envelope.
pub trait Endpoint {
    /// Parameters serialized as a POST body or individual GET query parameters.
    type Request: Serialize + DeserializeOwned;
    /// Successful result before the common transport envelope is added.
    type Response: Serialize + DeserializeOwned;
    /// Exact case-sensitive method name used by the JSON-RPC proxy.
    const METHOD: &'static str;
    /// Absolute route, including the `/api/v2` prefix.
    const PATH: &'static str;
    /// Whether GET is supported in addition to POST.
    const SUPPORTS_GET: bool;
    /// User-facing explanation of the operation and its relevant limitations.
    const DESCRIPTION: &'static str;
}

macro_rules! endpoints {
    ($($name:ident, $rust_name:ident, $method:tt, $request:ty, $response:ty, $get:literal, $description:literal;)+) => {
        $(
            #[doc = $description]
            #[derive(Debug, Clone, Copy)]
            pub struct $name;

            impl Endpoint for $name {
                type Request = $request;
                type Response = $response;
                const METHOD: &'static str = $method;
                const PATH: &'static str = concat!("/api/v2/", $method);
                const SUPPORTS_GET: bool = $get;
                const DESCRIPTION: &'static str = $description;
            }
        )+

        /// Case-sensitive JSON-RPC method names, including `shards` and `sendBocReturnHashNoError`.
        pub const METHODS: &[&str] = &[$($method),+];

        #[cfg(feature = "openapi")]
        pub(super) fn register(document: &mut super::openapi::Document) {
            $(document.endpoint::<$name>();)+
        }
    };
}

/// Supplies each v2 operation's marker, Rust method name, wire metadata, and types
/// to a callback macro. SDKs can derive a typed interface from this catalog.
#[macro_export]
macro_rules! for_each_v2_endpoint {
    ($callback:ident) => {
        $callback! {
    DetectAddress, detect_address, "detectAddress",
        $crate::v2::requests::DetectAddressRequest, $crate::v2::responses::DetectAddress, true,
        "Validates an address and returns it in all standard formats. Use this to convert between address formats or to validate user input. Returns raw format (0:abc), base64 bounceable (EQ), base64 non-bounceable (UQ), and URL-safe variants.";
    DetectHash, detect_hash, "detectHash",
        $crate::v2::requests::DetectHashRequest, $crate::v2::responses::DetectHash, true,
        "Validates a hash and returns it in all standard formats. Use this to convert between hex (64 chars) and base64 (44 chars) representations. Works with any 256-bit hash including transaction hashes, block hashes, and message hashes.";
    PackAddress, pack_address, "packAddress",
        $crate::v2::requests::PackAddressRequest, String, true,
        "Converts a raw address to user-friendly base64 format. Raw addresses use the format `workchain:hex` (e.g., `0:abc...`). The packed format is shorter and includes a checksum for error detection.";
    UnpackAddress, unpack_address, "unpackAddress",
        $crate::v2::requests::UnpackAddressRequest, String, true,
        "Converts a user-friendly base64 address to a raw address string in `workchain:hex` format.";
    GetAddressInformation, get_address_information, "getAddressInformation",
        $crate::v2::requests::AddressInformationRequest, $crate::v2::responses::AddressInformation, true,
        "Returns the current state of any account on the TON blockchain. Includes the balance (in nanograms), smart contract code and data (if deployed), account status, and a reference to the last transaction. This is the primary endpoint for checking if an address exists and what's deployed there.";
    GetExtendedAddressInformation, get_extended_address_information, "getExtendedAddressInformation",
        $crate::v2::requests::ExtendedAddressInformationRequest, $crate::v2::responses::ExtendedAddressInformation, true,
        "Returns detailed account information with parsed contract state. For recognized contract types, returns type-specific state fields. For other contracts, returns the raw state.";
    GetShardAccountCell, get_shard_account_cell, "getShardAccountCell",
        $crate::v2::requests::ShardAccountCellRequest, $crate::v2::stack::TvmCell, true,
        "Get raw TVM cell with shard account";
    GetWalletInformation, get_wallet_information, "getWalletInformation",
        $crate::v2::requests::WalletInformationRequest, $crate::v2::responses::WalletInformation, true,
        "Returns wallet-specific information for an address. If the address is a known wallet contract, returns the wallet type, current `seqno` (needed for sending transactions), and `wallet_id`. Always check `wallet: true` before using wallet-specific fields. Call this before sending any transaction to get the current `seqno`.";
    GetAddressBalance, get_address_balance, "getAddressBalance",
        $crate::v2::requests::AddressBalanceRequest, String, true,
        "Returns the GRAM balance of an account in nanograms. 1 GRAM = 1,000,000,000 nanograms. A lightweight endpoint that returns only the balance without contract code, data, or other account details. Returns \"0\" for addresses that have never received any funds.";
    GetAddressState, get_address_state, "getAddressState",
        $crate::v2::requests::AddressStateRequest, $crate::v2::responses::AccountStateEnum, true,
        "Returns the account lifecycle state: `uninitialized` (no contract deployed), `active` (contract deployed), or `frozen` (contract state frozen).";
    GetTokenData, get_token_data, "getTokenData",
        $crate::v2::requests::TokenDataRequest, $crate::v2::responses::TokenData, true,
        "Returns metadata for Jetton or NFT contracts. Automatically detects the contract type and returns appropriate fields. For Jetton masters: total supply, admin, metadata. For Jetton wallets: balance, owner. For NFT items: collection, owner, content. For NFT collections: item count, metadata.";
    DnsResolve, dns_resolve, "dnsResolve",
        $crate::v2::requests::DnsResolveRequest, $crate::v2::responses::DnsResolved, true,
        "Resolve TON DNS contract";
    GetMasterchainInfo, get_masterchain_info, "getMasterchainInfo",
        $crate::v2::requests::MasterchainInfoRequest, $crate::v2::responses::MasterchainInfo, true,
        "Returns the current state of the TON masterchain. The `last` field contains the latest block used for querying current state. The `seqno` in `last` is the current block height. Use this endpoint to obtain the latest block reference for other queries.";
    GetMasterchainBlockSignatures, get_masterchain_block_signatures, "getMasterchainBlockSignatures",
        $crate::v2::requests::MasterchainBlockSignaturesRequest, $crate::v2::responses::MasterchainBlockSignatures, true,
        "Returns validator signatures for a specific masterchain block. Each signature proves that a validator approved this block. Use this for building cryptographic proofs or verifying block authenticity in trustless applications.";
    GetShardBlockProof, get_shard_block_proof, "getShardBlockProof",
        $crate::v2::requests::ShardBlockProofRequest, $crate::v2::responses::ShardBlockProof, true,
        "Returns a Merkle proof that links a shardchain block to a masterchain block. This proof cryptographically verifies that the shard block is part of the canonical chain. Used by light clients and cross-chain bridges to verify shard data without trusting the API.";
    GetConsensusBlock, get_consensus_block, "getConsensusBlock",
        $crate::v2::requests::ConsensusBlockRequest, $crate::v2::responses::ConsensusBlock, true,
        "Get block that was confirmed by consensus";
    LookupBlock, lookup_block, "lookupBlock",
        $crate::v2::requests::LookupBlockRequest, $crate::v2::responses::TonBlockIdExt, true,
        "Finds a block by position or time. Specify workchain and shard, then provide exactly one of `seqno` (block sequence number), `lt` (logical time), or `unixtime` (Unix timestamp). Returns the full block identifier including hashes.";
    GetShards, get_shards, "getShards",
        $crate::v2::requests::ShardsRequest, $crate::v2::responses::Shards, true,
        "Returns the active shardchain block identifiers at a given masterchain block height. Each shard processes a subset of accounts in parallel. The response shows how the basechain is currently partitioned and which block each shard is at.";
    GetBlock, get_block, "getBlock",
        $crate::v2::requests::BlockDataRequest, $crate::v2::responses::BlockData, true,
        "Get raw block data as a base64-encoded BOC";
    GetBlockHeader, get_block_header, "getBlockHeader",
        $crate::v2::requests::BlockHeaderRequest, $crate::v2::responses::BlockHeader, true,
        "Returns block metadata without the full transaction list. Includes timestamps, validator info, and references to previous blocks. Intended for block explorers and other use cases that require block information without transactions.";
    GetOutMsgQueueSize, get_out_msg_queue_size, "getOutMsgQueueSize",
        $crate::v2::requests::OutMsgQueueSizeRequest, $crate::v2::responses::OutMsgQueueSizes, true,
        "Returns the current size of the outbound message queue for each shard. A growing queue indicates network congestion. If the queue is large, transactions may take longer to process. Monitor this to detect network issues.";
    GetBlockTransactions, get_block_transactions, "getBlockTransactions",
        $crate::v2::requests::BlockTransactionsRequest, $crate::v2::responses::BlockTransactions, true,
        "Returns a summary of transactions in a specific block. Each item contains the account address and transaction ID, but not full transaction details. Use `count` to limit results and `after_lt`/`after_hash` for pagination. Call getTransactions with each transaction ID to get full details.";
    GetBlockTransactionsExt, get_block_transactions_ext, "getBlockTransactionsExt",
        $crate::v2::requests::BlockTransactionsExtRequest, $crate::v2::responses::BlockTransactionsExt, true,
        "Returns full transaction objects for transactions in a specific block. Each transaction includes complete data: inbound and outbound messages, fees, and BoC-encoded raw data. Use `count` to limit results and `after_lt`/`after_hash` for pagination when `incomplete` is true.";
    GetTransactions, get_transactions, "getTransactions",
        $crate::v2::requests::TransactionsRequest, $crate::v2::responses::Transactions, true,
        "Returns transaction history for an account. Transactions are returned newest-first. Each transaction shows the incoming message that triggered it, all outgoing messages, and fees paid. For pagination: use the `lt` and `hash` from the oldest transaction as the starting point for the next request.";
    GetTransactionsStd, get_transactions_std, "getTransactionsStd",
        $crate::v2::requests::TransactionsRequest, $crate::v2::responses::TransactionsStd, true,
        "Returns transaction history for an account in a standardized format. Transactions are returned newest-first. Each transaction includes the triggering inbound message, all outbound messages, and fees paid. The response includes a `previous_transaction_id` cursor for paginating through older transactions.";
    TryLocateTx, try_locate_tx, "tryLocateTx",
        $crate::v2::requests::TryLocateTxRequest, $crate::v2::responses::Transaction, true,
        "Finds a transaction by message parameters. Given a source address, destination address, and message creation time (`created_lt`), returns the transaction that processed this message. Useful for locating when a previously sent message was executed.";
    TryLocateResultTx, try_locate_result_tx, "tryLocateResultTx",
        $crate::v2::requests::TryLocateResultTxRequest, $crate::v2::responses::Transaction, true,
        "Finds the transaction that received a specific message. Given message parameters, returns the transaction on the destination account that processed the incoming message. Use this to trace message delivery across accounts.";
    TryLocateSourceTx, try_locate_source_tx, "tryLocateSourceTx",
        $crate::v2::requests::TryLocateSourceTxRequest, $crate::v2::responses::Transaction, true,
        "Finds the transaction that sent a specific message. Given message parameters, returns the transaction on the source account that created this outgoing message. Useful for tracing where a message originated from.";
    GetConfigParam, get_config_param, "getConfigParam",
        $crate::v2::requests::ConfigParamRequest, $crate::v2::responses::ConfigInfo, true,
        "Returns a specific blockchain configuration parameter. TON stores all network settings on-chain as numbered parameters. Common ones: 0 (config contract), 1 (elector), 15 (election timing), 17 (stake limits), 20-21 (gas prices), 34 (current validators). Check TON documentation for the full list.";
    GetConfigAll, get_config_all, "getConfigAll",
        $crate::v2::requests::ConfigAllRequest, $crate::v2::responses::ConfigInfo, true,
        "Returns all blockchain configuration parameters at once. Includes gas prices, validator settings, workchain configs, and governance rules. Use the optional `seqno` to get historical configuration at a specific block height.";
    GetLibraries, get_libraries, "getLibraries",
        $crate::v2::requests::LibrariesRequest, $crate::v2::responses::LibraryResult, true,
        "Returns smart contract library code by hash. Some contracts reference shared libraries instead of including all code directly. When a library reference appears in contract code, this endpoint fetches the actual library implementation.";
    RunGetMethod, run_get_method, "runGetMethod",
        $crate::v2::requests::RunGetMethodRequest, $crate::v2::responses::RunGetMethodResult, false,
        "Executes a read-only method on a smart contract. Get methods query contract state without sending a transaction. Common methods include `seqno` (wallet sequence number), `get_wallet_data` (wallet info), and `get_jetton_data` (token info). Method arguments are provided in the `stack` array.";
    RunGetMethodStd, run_get_method_std, "runGetMethodStd",
        $crate::v2::requests::RunGetMethodStdRequest, $crate::v2::responses::RunGetMethodStdResult, false,
        "Executes a read-only method on a smart contract using typed stack entries. Input and output stack entries use explicit types (`TvmStackEntryNumber`, `TvmStackEntryCell`, etc.) for structured input/output handling. Common methods: `seqno` (wallet sequence number), `get_wallet_data` (wallet info), `get_jetton_data` (token info).";
    SendBoc, send_boc, "sendBoc",
        $crate::v2::requests::SendBocRequest, $crate::v2::responses::ResultOk, false,
        "Broadcasts a signed message to the TON network. The `boc` parameter must contain a complete, signed external message in base64 format. The API validates the message and forwards it to validators. Returns immediately after acceptance; use getTransactions to confirm the transaction was processed.";
    SendBocReturnHash, send_boc_return_hash, "sendBocReturnHash",
        $crate::v2::requests::SendBocRequest, $crate::v2::responses::ExtMessageInfo, false,
        "Broadcasts a signed message to the TON network and returns the message hash. The `boc` parameter must contain a complete, signed external message in base64 format. The API validates the message and forwards it to validators. The returned hash can be used to track the message's processing status.";
    EstimateFee, estimate_fee, "estimateFee",
        $crate::v2::requests::EstimateFeeRequest, $crate::v2::responses::QueryFees, false,
        "Calculates the fees required to send a message. Provide the destination address and message body. For new contract deployments, also include `init_code` and `init_data`. Set `ignore_chksig` to true when estimating before signing. Returns a breakdown of storage, gas, and forwarding fees.";
    ShardsAlias, shards, "shards",
        $crate::v2::requests::ShardsRequest, $crate::v2::responses::Shards, true,
        "C++ compatibility spelling of getShards, present in the server route and JSON-RPC allowlist.";
    SendBocReturnHashNoError, send_boc_return_hash_no_error, "sendBocReturnHashNoError",
        $crate::v2::requests::SendBocRequest, $crate::v2::responses::ExtMessageInfo, false,
        "C++ broadcast route returning message hashes. Despite its name, validation and `TONLib` failures still return errors; a successful response does not prove inclusion.";

        }
    };
}

crate::for_each_v2_endpoint!(endpoints);
