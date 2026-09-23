//! Typed routes for the supported TON Center v3 operations.
//!
//! Transports use these markers to pair requests with responses and select the
//! HTTP method. Authentication, rate limits, and retries belong to the transport.

use serde::{Serialize, de::DeserializeOwned};

/// HTTP method accepted by a v3 route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// Parameters are encoded in the URL query.
    Get,
    /// Parameters are encoded as a JSON request body.
    Post,
}

/// Associates a v3 route with its parameters and successful JSON response.
pub trait Endpoint {
    /// Query parameters for GET, or the JSON body for POST.
    type Request: Serialize + DeserializeOwned;
    /// Successful JSON body, without a v2 response envelope.
    type Response: Serialize + DeserializeOwned;
    /// HTTP method accepted by the route.
    const METHOD: HttpMethod;
    /// Absolute route, including the `/api/v3` prefix.
    const PATH: &'static str;
    /// Stable operation name used in the generated `OpenAPI` document.
    const OPERATION_ID: &'static str;
    /// Purpose of the operation and relevant usage constraints.
    const DESCRIPTION: &'static str;
}

macro_rules! endpoints {
    ($($name:ident, $rust_name:ident, $method:ident, $path:tt, $request:ty, $response:ty, $description:literal;)+) => {
        $(
            #[doc = $description]
            #[derive(Debug, Clone, Copy)]
            pub struct $name;

            impl Endpoint for $name {
                type Request = $request;
                type Response = $response;
                const METHOD: HttpMethod = HttpMethod::$method;
                const PATH: &'static str = concat!("/api/v3/", $path);
                const OPERATION_ID: &'static str = stringify!($name);
                const DESCRIPTION: &'static str = $description;
            }
        )+

        /// Supported routes and their HTTP methods, including the `/api/v3` prefix.
        pub const ROUTES: &[(HttpMethod, &str)] = &[$(($name::METHOD, $name::PATH)),+];

        #[cfg(feature = "openapi")]
        pub(super) fn register(document: &mut super::openapi::Document) {
            $(document.endpoint::<$name>();)+
        }
    };
}

/// Supplies each v3 operation's marker, Rust method name, wire metadata, and types
/// to a callback macro. SDKs can derive a typed interface from this catalog.
#[macro_export]
macro_rules! for_each_v3_endpoint {
    ($callback:ident) => {
        $callback! {
    GetMasterchainInfo, get_masterchain_info, Get, "masterchainInfo",
        $crate::v3::requests::MasterchainInfoQuery, $crate::v3::responses::MasterchainInfo,
        "Returns the oldest and newest masterchain blocks available in the index.";
    GetMasterchainBlockShardState, get_masterchain_block_shard_state, Get, "masterchainBlockShardState",
        $crate::v3::requests::MasterchainBlockShardStateQuery, $crate::v3::responses::BlocksResponse,
        "Returns the shard blocks referenced by the state of a masterchain block.";
    GetMasterchainBlockShards, get_masterchain_block_shards, Get, "masterchainBlockShards",
        $crate::v3::requests::MasterchainBlockShardsQuery, $crate::v3::responses::BlocksResponse,
        "Returns shard blocks committed by a masterchain block, with pagination.";
    GetAddressInformation, get_address_information, Get, "addressInformation",
        $crate::v3::requests::AddressInformationQuery, $crate::v3::responses::V2AddressInformation,
        "Returns an account's balance, lifecycle state, code, data, and last transaction reference.";
    GetAddressBook, get_address_book, Get, "addressBook",
        $crate::v3::requests::AddressesQuery, $crate::v3::responses::AddressBook,
        "Returns address presentation and detected contract interfaces, keyed by address.";
    GetMetadata, get_metadata, Get, "metadata",
        $crate::v3::requests::AddressesQuery, $crate::v3::responses::Metadata,
        "Returns indexing status and token or NFT metadata, keyed by address.";
    GetWalletInformation, get_wallet_information, Get, "walletInformation",
        $crate::v3::requests::WalletInformationQuery, $crate::v3::responses::V2WalletInformation,
        "Returns wallet detection and version-specific state, including the sequence counter and wallet identifier.";
    GetAccountStates, get_account_states, Get, "accountStates",
        $crate::v3::requests::AccountStatesQuery, $crate::v3::responses::AccountStatesResponse,
        "Returns indexed account states, with optional serialized contract code and data.";
    GetTraces, get_traces, Get, "traces",
        $crate::v3::requests::TracesQuery, $crate::v3::responses::TracesResponse,
        "Returns transaction traces selected by account, hash, block, or execution range. Trace status indicates whether indexing is complete.";
    GetPendingActions, get_pending_actions, Get, "pendingActions",
        $crate::v3::requests::PendingActionsQuery, $crate::v3::responses::ActionsResponse,
        "Returns parsed actions in pending traces. Pending results may change before transactions are included on chain.";
    GetPendingTraces, get_pending_traces, Get, "pendingTraces",
        $crate::v3::requests::PendingTracesQuery, $crate::v3::responses::TracesResponse,
        "Returns pending traces selected by account or external-message hash. Pending results do not prove on-chain inclusion.";
    GetDnsRecords, get_dns_records, Get, "dns/records",
        $crate::v3::requests::DnsRecordsQuery, $crate::v3::responses::DnsRecordsResponse,
        "Returns indexed DNS records selected by domain or wallet address.";
    GetJettonBurns, get_jetton_burns, Get, "jetton/burns",
        $crate::v3::requests::JettonBurnsQuery, $crate::v3::responses::JettonBurnsResponse,
        "Returns indexed jetton burns with their amounts, owners, and originating transactions.";
    GetJettonTransfers, get_jetton_transfers, Get, "jetton/transfers",
        $crate::v3::requests::JettonTransfersQuery, $crate::v3::responses::JettonTransfersResponse,
        "Returns indexed jetton transfers with participants, amounts, and notification payloads.";
    GetNftCollections, get_nft_collections, Get, "nft/collections",
        $crate::v3::requests::NftCollectionsQuery, $crate::v3::responses::NftCollectionsResponse,
        "Returns indexed NFT collection states and content metadata.";
    GetNftSales, get_nft_sales, Get, "nft/sales",
        $crate::v3::requests::NftSalesQuery, $crate::v3::responses::NftSalesResponse,
        "Returns NFT sale and auction contracts with their type-specific details.";
    GetNftTransfers, get_nft_transfers, Get, "nft/transfers",
        $crate::v3::requests::NftTransfersQuery, $crate::v3::responses::NftTransfersResponse,
        "Returns indexed NFT ownership transfers and their notification payloads.";
    GetMultisigOrders, get_multisig_orders, Get, "multisig/orders",
        $crate::v3::requests::MultisigOrdersQuery, $crate::v3::responses::MultisigOrdersResponse,
        "Returns multisig orders, approval state, and optionally decoded outgoing actions.";
    GetMultisigWallets, get_multisig_wallets, Get, "multisig/wallets",
        $crate::v3::requests::MultisigWalletsQuery, $crate::v3::responses::MultisigsResponse,
        "Returns multisig contracts and their authorized participants, with optional orders.";
    GetVesting, get_vesting, Get, "vesting",
        $crate::v3::requests::VestingQuery, $crate::v3::responses::VestingContractsResponse,
        "Returns vesting schedules and participants. The whitelist filter can include contracts that permit transfers to a wallet.";
    GetTransactions, get_transactions, Get, "transactions",
        $crate::v3::requests::TransactionsQuery, $crate::v3::responses::TransactionsResponse,
        "Returns indexed transactions selected by account, block, hash, or execution range.";
    GetBlocks, get_blocks, Get, "blocks",
        $crate::v3::requests::BlocksQuery, $crate::v3::responses::BlocksResponse,
        "Returns indexed block headers, predecessor references, and transaction counts.";
    GetTransactionsByMessage, get_transactions_by_message, Get, "transactionsByMessage",
        $crate::v3::requests::TransactionsByMessageQuery, $crate::v3::responses::TransactionsResponse,
        "Returns transactions linked to messages matching the supplied hashes, body hash, or opcode.";
    GetTransactionsByMasterchainBlock, get_transactions_by_masterchain_block, Get, "transactionsByMasterchainBlock",
        $crate::v3::requests::TransactionsByMasterchainBlockQuery, $crate::v3::responses::TransactionsResponse,
        "Returns transactions associated with a masterchain block, with pagination and ordering.";
    GetMessages, get_messages, Get, "messages",
        $crate::v3::requests::MessagesQuery, $crate::v3::responses::MessagesResponse,
        "Returns indexed messages, transferred values, and serialized or decoded content.";
    GetAdjacentTransactions, get_adjacent_transactions, Get, "adjacentTransactions",
        $crate::v3::requests::AdjacentTransactionsQuery, $crate::v3::responses::TransactionsResponse,
        "Returns transactions connected to a transaction through incoming or outgoing messages.";
    GetWalletStates, get_wallet_states, Get, "walletStates",
        $crate::v3::requests::WalletStatesQuery, $crate::v3::responses::WalletStatesResponse,
        "Returns indexed wallet detection results and version-specific wallet state for the requested accounts.";
    GetTopAccountsByBalance, get_top_accounts_by_balance, Get, "topAccountsByBalance",
        $crate::v3::requests::TopAccountsByBalanceQuery, Vec<$crate::v3::responses::AccountBalance>,
        "Returns a page of accounts ordered by descending GRAM balance, with amounts in nanograms.";
    EstimateFee, estimate_fee, Post, "estimateFee",
        $crate::v3::requests::EstimateFeeRequest, $crate::v3::responses::EstimateFeeResult,
        "Estimates message-processing fees for the sender and destination contracts without broadcasting the message.";
    GetPendingTransactions, get_pending_transactions, Get, "pendingTransactions",
        $crate::v3::requests::PendingTransactionsQuery, $crate::v3::responses::TransactionsResponse,
        "Returns pending transactions for the selected accounts and traces. Pending results do not prove on-chain inclusion.";
    GetJettonMasters, get_jetton_masters, Get, "jetton/masters",
        $crate::v3::requests::JettonMastersQuery, $crate::v3::responses::JettonMastersResponse,
        "Returns indexed jetton master states, supply, administration, and token metadata.";
    GetJettonWallets, get_jetton_wallets, Get, "jetton/wallets",
        $crate::v3::requests::JettonWalletsQuery, $crate::v3::responses::JettonWalletsResponse,
        "Returns indexed jetton wallets, their owners, and balances in the token's smallest units.";
    GetNftItems, get_nft_items, Get, "nft/items",
        $crate::v3::requests::NftItemsQuery, $crate::v3::responses::NftItemsResponse,
        "Returns indexed NFT ownership, collection membership, content, and sale status.";
    SendMessage, send_message, Post, "message",
        $crate::v3::requests::SendMessageRequest, $crate::v3::responses::SendMessageResult,
        "Submits an external message for broadcast and returns its hash. Acceptance does not prove on-chain inclusion.";
    RunGetMethod, run_get_method, Post, "runGetMethod",
        $crate::v3::requests::RunGetMethodRequest, $crate::v3::responses::RunGetMethodResult,
        "Executes a contract get method with the supplied stack and returns its exit code, gas usage, and output stack.";

        }
    };
}

crate::for_each_v3_endpoint!(endpoints);
