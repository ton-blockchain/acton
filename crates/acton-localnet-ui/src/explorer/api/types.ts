export interface ApiOk<T> {
  readonly ok: true
  readonly result: T
  readonly "@extra"?: string
}

export interface ApiError {
  readonly ok: false
  readonly error: string
  readonly code?: number
  readonly "@extra"?: string
}

export type ApiResponse<T> = ApiOk<T> | ApiError

export interface TransactionId {
  readonly "@type": "internal.transactionId"
  readonly lt: string
  readonly hash: string
}

export interface BlockId {
  readonly "@type": "ton.blockIdExt"
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
  readonly root_hash: string
  readonly file_hash: string
}

export interface AddressInformation {
  readonly balance: string
  readonly code: string | null
  readonly data: string | null
  readonly frozen_hash: string | null
  readonly last_transaction_hash: string
  readonly last_transaction_lt: string
  readonly status: "active" | "uninitialized" | "uninit" | "frozen" | "nonexist"
}

export interface AccountStateTokenInfo {
  readonly type: string
  readonly [key: string]: unknown
}

export interface AccountStatesAddressBookRow {
  readonly user_friendly: string
  readonly domain?: string | null
  readonly interfaces: readonly string[] | null
}

export interface V3AccountState {
  readonly account_state_hash: string
  readonly address: string
  readonly balance: string
  readonly code_boc?: string
  readonly code_hash?: string
  readonly contract_methods: readonly number[]
  readonly data_boc?: string
  readonly data_hash?: string
  readonly extra_currencies: Record<string, string>
  readonly frozen_hash?: string
  readonly interfaces: readonly string[] | null
  readonly last_transaction_hash: string
  readonly last_transaction_lt: string
  readonly status: string
}

export interface AccountStatesResponse {
  readonly accounts: readonly V3AccountState[]
  readonly address_book: Record<string, AccountStatesAddressBookRow>
  readonly metadata: Record<
    string,
    {
      readonly is_indexed: boolean
      readonly token_info: readonly AccountStateTokenInfo[]
    }
  >
}

export interface V3TracesResponse {
  readonly address_book: Record<string, unknown>
  readonly metadata: V3Metadata
  readonly traces: readonly V3Trace[]
}

export interface V3ActionsResponse {
  readonly actions: readonly V3Action[]
  readonly address_book: Record<string, unknown>
  readonly metadata: V3Metadata
}

export interface V3AddressMetadata {
  readonly token_info?: readonly AccountStateTokenInfo[]
}

export type V3Metadata = Record<string, V3AddressMetadata>

type V3ActionAddress = string | null
type V3ActionString = string | null
type V3ActionBoolean = boolean | null
type V3ActionNumber = number | null
type V3ActionOpcode = string | number | null

export interface V3ActionBase<TType extends string, TDetails> {
  readonly accounts?: readonly string[]
  readonly action_id: string
  readonly details: TDetails
  readonly end_lt: string
  readonly end_utime: number
  readonly finality: string
  readonly start_lt: string
  readonly start_utime: number
  readonly success: boolean | null
  readonly trace_end_lt: string
  readonly trace_end_utime: number
  readonly trace_external_hash?: string | null
  readonly trace_external_hash_norm?: string | null
  readonly trace_id: string | null
  readonly trace_mc_seqno_end: number
  readonly transactions: readonly string[]
  readonly transactions_full?: readonly V3Transaction[]
  readonly type: TType
}

export interface V3ActionDetailsCallContract {
  readonly opcode?: V3ActionOpcode
  readonly source?: V3ActionAddress
  readonly destination?: V3ActionAddress
  readonly value?: V3ActionString
  readonly extra_currencies?: Record<string, string> | null
}

export interface V3ActionDetailsContractDeploy {
  readonly opcode?: V3ActionOpcode
  readonly source?: V3ActionAddress
  readonly destination?: V3ActionAddress
  readonly value?: V3ActionString
}

export interface V3ActionDetailsTonTransfer {
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly value: V3ActionString
  readonly value_extra_currencies?: Record<string, string> | null
  readonly comment: V3ActionString
  readonly encrypted: V3ActionBoolean
}

export interface V3ActionDetailsAuctionBid {
  readonly amount: V3ActionString
  readonly bidder: V3ActionAddress
  readonly auction: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly nft_item_index: V3ActionString
}

export interface V3ActionDetailsChangeDnsValue {
  readonly sum_type: V3ActionString
  readonly dns_smc_address: V3ActionString
  readonly dns_adnl_address: V3ActionString
  readonly dns_text: V3ActionString
  readonly dns_next_resolver_address: V3ActionString
  readonly dns_storage_address: V3ActionString
  readonly flags: V3ActionNumber
}

export interface V3ActionDetailsChangeDns {
  readonly key: V3ActionString
  readonly value: V3ActionDetailsChangeDnsValue
  readonly source: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly nft_collection: V3ActionAddress
}

export interface V3ActionDetailsDeleteDns {
  readonly hash: V3ActionString
  readonly source: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly nft_collection: V3ActionAddress
}

export interface V3ActionDetailsRenewDns {
  readonly source: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly nft_collection: V3ActionAddress
}

export interface V3ActionDetailsElection {
  readonly stake_holder: V3ActionAddress
  readonly amount?: V3ActionString
}

export interface V3ActionDetailsJettonBurn {
  readonly owner: V3ActionAddress
  readonly owner_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsJettonSwapTransfer {
  readonly asset: V3ActionAddress
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly destination_jetton_wallet: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsJettonSwapPeerSwap {
  readonly asset_in: V3ActionAddress
  readonly amount_in: V3ActionString
  readonly asset_out: V3ActionAddress
  readonly amount_out: V3ActionString
}

export interface V3ActionDetailsJettonSwap {
  readonly dex: V3ActionString
  readonly sender: V3ActionAddress
  readonly asset_in: V3ActionAddress
  readonly asset_out: V3ActionAddress
  readonly dex_incoming_transfer: V3ActionDetailsJettonSwapTransfer | null
  readonly dex_outgoing_transfer: V3ActionDetailsJettonSwapTransfer | null
  readonly peer_swaps: readonly V3ActionDetailsJettonSwapPeerSwap[]
}

export interface V3ActionDetailsToncoJettonSwap extends V3ActionDetailsJettonSwap {
  readonly min_out_amount: V3ActionString
}

export interface V3ActionDetailsLayerZeroSendData {
  readonly send_request_id: number | null
  readonly msglib_manager: V3ActionString
  readonly msglib: V3ActionString
  readonly uln: V3ActionAddress
  readonly native_fee: number | null
  readonly zro_fee: number | null
  readonly endpoint: V3ActionAddress
  readonly channel: V3ActionAddress
}

export interface V3ActionDetailsLayerZeroPacket {
  readonly src_oapp: V3ActionString
  readonly dst_oapp: V3ActionString
  readonly src_eid: V3ActionNumber
  readonly dst_eid: V3ActionNumber
  readonly nonce: V3ActionNumber
  readonly guid: V3ActionString
  readonly message: V3ActionString
}

export interface V3ActionDetailsLayerZeroSend {
  readonly initiator: V3ActionAddress
  readonly layerzero_send_data: V3ActionDetailsLayerZeroSendData
  readonly layerzero_packet_data: V3ActionDetailsLayerZeroPacket
}

export interface V3ActionDetailsLayerZeroReceive {
  readonly sender: V3ActionAddress
  readonly oapp: V3ActionAddress
  readonly channel: V3ActionAddress
  readonly layerzero_packet_data: V3ActionDetailsLayerZeroPacket
}

export interface V3ActionDetailsLayerZeroCommitPacket {
  readonly sender: V3ActionAddress
  readonly endpoint: V3ActionAddress
  readonly uln: V3ActionAddress
  readonly uln_connection: V3ActionAddress
  readonly channel: V3ActionAddress
  readonly msglib_connection: V3ActionAddress
  readonly layerzero_packet_data: V3ActionDetailsLayerZeroPacket
}

export interface V3ActionDetailsLayerZeroDvnVerify {
  readonly initiator: V3ActionAddress
  readonly nonce: V3ActionNumber
  readonly status: V3ActionString
  readonly dvn: V3ActionAddress
  readonly proxy: V3ActionAddress
  readonly uln: V3ActionAddress
  readonly uln_connection: V3ActionAddress
}

export interface V3ActionDetailsLayerZeroSendTokens {
  readonly sender: V3ActionAddress
  readonly sender_wallet: V3ActionAddress
  readonly oapp: V3ActionAddress
  readonly oapp_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
  readonly layerzero_send_data: V3ActionDetailsLayerZeroSendData
  readonly layerzero_packet_data: V3ActionDetailsLayerZeroPacket
}

export interface V3ActionDetailsJettonTransfer {
  readonly asset: V3ActionAddress
  readonly sender: V3ActionAddress
  readonly receiver: V3ActionAddress
  readonly sender_jetton_wallet: V3ActionAddress
  readonly receiver_jetton_wallet: V3ActionAddress
  readonly amount: V3ActionString
  readonly comment: V3ActionString
  readonly is_encrypted_comment: V3ActionBoolean
  readonly query_id: V3ActionString
  readonly response_destination: V3ActionAddress
  readonly custom_payload: V3ActionString
  readonly forward_payload: V3ActionString
  readonly forward_amount: V3ActionString
}

export interface V3ActionDetailsJettonMint {
  readonly asset: V3ActionAddress
  readonly receiver: V3ActionAddress
  readonly receiver_jetton_wallet: V3ActionAddress
  readonly amount: V3ActionString
  readonly ton_amount: V3ActionString
}

export interface V3ActionDetailsNftMint {
  readonly owner?: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly nft_item_index: V3ActionString
}

export interface V3ActionDetailsNftTransfer {
  readonly nft_collection: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_item_index: V3ActionString
  readonly old_owner?: V3ActionAddress
  readonly new_owner: V3ActionAddress
  readonly is_purchase: V3ActionBoolean
  readonly price?: V3ActionString
  readonly query_id: V3ActionString
  readonly response_destination: V3ActionAddress
  readonly custom_payload: V3ActionString
  readonly forward_payload: V3ActionString
  readonly forward_amount: V3ActionString
  readonly comment: V3ActionString
  readonly is_encrypted_comment: V3ActionBoolean
  readonly marketplace: V3ActionString
  readonly real_old_owner: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly payout_amount: V3ActionString
  readonly payout_address: V3ActionAddress
  readonly payout_comment: V3ActionString
  readonly payout_comment_encrypted: V3ActionBoolean
  readonly payout_comment_encoded: V3ActionBoolean
  readonly royalty_amount: V3ActionString
  readonly royalty_address: V3ActionAddress
}

export interface V3ActionDetailsDnsPurchase {
  readonly nft_collection: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_item_index: V3ActionString
  readonly new_owner: V3ActionAddress
  readonly price: V3ActionString
  readonly query_id: V3ActionString
  readonly payout_amount: V3ActionString
}

export interface V3ActionDetailsNftPutOnSale {
  readonly nft_collection: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_item_index: V3ActionString
  readonly owner: V3ActionAddress
  readonly listing_address: V3ActionAddress
  readonly sale_address: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly full_price: V3ActionString
  readonly marketplace_fee: V3ActionString
  readonly royalty_amount: V3ActionString
  readonly marketplace_fee_address: V3ActionAddress
  readonly royalty_address: V3ActionAddress
  readonly marketplace: V3ActionString
}

export interface V3ActionDetailsNftPutOnAuction {
  readonly nft_collection: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_item_index: V3ActionString
  readonly owner: V3ActionAddress
  readonly listing_address: V3ActionAddress
  readonly auction_address: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly marketplace_fee_factor: V3ActionString
  readonly marketplace_fee_base: V3ActionString
  readonly royalty_fee_base: V3ActionString
  readonly max_bid: V3ActionString
  readonly min_bid: V3ActionString
  readonly marketplace_fee_address: V3ActionAddress
  readonly royalty_address: V3ActionAddress
  readonly marketplace: V3ActionString
}

export interface V3ActionDetailsTickTock {
  readonly account?: V3ActionAddress
}

export interface V3ActionDetailsSubscribe {
  readonly subscriber: V3ActionAddress
  readonly beneficiary?: V3ActionAddress
  readonly subscription: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsUnsubscribe {
  readonly subscriber: V3ActionAddress
  readonly beneficiary?: V3ActionAddress
  readonly subscription: V3ActionAddress
  readonly amount?: V3ActionString
}

export interface V3ActionDetailsWtonMint {
  readonly amount: V3ActionString
  readonly receiver: V3ActionAddress
}

export interface V3ActionDetailsLiquidityVaultExcess {
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsDexDepositLiquidity {
  readonly dex: V3ActionString
  readonly amount_1: V3ActionString
  readonly amount_2: V3ActionString
  readonly asset_1: V3ActionAddress
  readonly asset_2: V3ActionAddress
  readonly user_jetton_wallet_1: V3ActionAddress
  readonly user_jetton_wallet_2: V3ActionAddress
  readonly source: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly destination_liquidity: V3ActionAddress
  readonly lp_tokens_minted: V3ActionString
  readonly target_asset_1: V3ActionAddress
  readonly target_asset_2: V3ActionAddress
  readonly target_amount_1: V3ActionString
  readonly target_amount_2: V3ActionString
  readonly vault_excesses: readonly V3ActionDetailsLiquidityVaultExcess[]
  readonly tick_lower: V3ActionString
  readonly tick_upper: V3ActionString
  readonly nft_index: V3ActionString
  readonly nft_address: V3ActionAddress
}

export interface V3ActionDetailsDexWithdrawLiquidity {
  readonly dex: V3ActionString
  readonly amount_1: V3ActionString
  readonly amount_2: V3ActionString
  readonly asset_1: V3ActionAddress
  readonly asset_2: V3ActionAddress
  readonly user_jetton_wallet_1: V3ActionAddress
  readonly user_jetton_wallet_2: V3ActionAddress
  readonly lp_tokens_burnt: V3ActionString
  readonly is_refund: V3ActionBoolean
  readonly source: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly destination_liquidity: V3ActionAddress
  readonly burnt_nft_index: V3ActionString
  readonly burnt_nft_address: V3ActionAddress
  readonly tick_lower: V3ActionString
  readonly tick_upper: V3ActionString
}

export interface V3ActionDetailsToncoDeployPool {
  readonly source: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly router: V3ActionAddress
  readonly router_jetton_wallet_1: V3ActionAddress
  readonly router_jetton_wallet_2: V3ActionAddress
  readonly jetton_minter_1: V3ActionAddress
  readonly jetton_minter_2: V3ActionAddress
  readonly tick_spacing: V3ActionString
  readonly initial_price_x96: V3ActionString
  readonly protocol_fee: V3ActionString
  readonly lp_fee_base: V3ActionString
  readonly lp_fee_current: V3ActionString
  readonly pool_active: V3ActionBoolean
}

export interface V3ActionDetailsStakeDeposit {
  readonly provider: V3ActionString
  readonly stake_holder: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly amount: V3ActionString
  readonly tokens_minted: V3ActionString
  readonly asset: V3ActionAddress
  readonly source_asset?: V3ActionAddress
}

export interface V3ActionDetailsWithdrawStake {
  readonly provider: V3ActionString
  readonly stake_holder: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly amount: V3ActionString
  readonly payout_nft: V3ActionAddress
  readonly tokens_burnt: V3ActionString
  readonly asset: V3ActionAddress
}

export interface V3ActionDetailsWithdrawStakeRequest {
  readonly provider: V3ActionString
  readonly stake_holder: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly payout_nft: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly tokens_burnt: V3ActionString
  readonly tokens_minted?: V3ActionString
}

export interface V3ActionDetailsMultisigCreateOrder {
  readonly query_id: V3ActionString
  readonly order_seqno: V3ActionString
  readonly is_created_by_signer: V3ActionBoolean
  readonly is_signed_by_creator: V3ActionBoolean
  readonly creator_index: V3ActionNumber
  readonly expiration_date: V3ActionNumber
  readonly order_boc: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly destination_order: V3ActionAddress
}

export interface V3ActionDetailsMultisigApprove {
  readonly signer_index: V3ActionNumber
  readonly exit_code: V3ActionNumber
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsMultisigExecute {
  readonly query_id: V3ActionString
  readonly order_seqno: V3ActionString
  readonly expiration_date: V3ActionNumber
  readonly approvals_num: V3ActionNumber
  readonly signers_hash: V3ActionString
  readonly order_boc: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsVestingSendMessage {
  readonly query_id: V3ActionString
  readonly message_boc: V3ActionString
  readonly source: V3ActionAddress
  readonly vesting: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsVestingAddWhitelist {
  readonly query_id: V3ActionString
  readonly accounts_added: readonly string[]
  readonly source: V3ActionAddress
  readonly vesting: V3ActionAddress
}

export interface V3ActionDetailsEvaaSupply {
  readonly sender_jetton_wallet: V3ActionAddress
  readonly recipient_jetton_wallet: V3ActionAddress
  readonly master_jetton_wallet: V3ActionAddress
  readonly master: V3ActionAddress
  readonly asset_id: V3ActionString
  readonly is_ton: V3ActionBoolean
  readonly source: V3ActionAddress
  readonly source_wallet: V3ActionAddress
  readonly recipient: V3ActionAddress
  readonly recipient_contract: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsEvaaWithdraw {
  readonly recipient_jetton_wallet: V3ActionAddress
  readonly master_jetton_wallet: V3ActionAddress
  readonly master: V3ActionAddress
  readonly fail_reason: V3ActionString
  readonly asset_id: V3ActionString
  readonly source: V3ActionAddress
  readonly recipient: V3ActionAddress
  readonly owner_contract: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsEvaaLiquidate {
  readonly fail_reason: V3ActionString
  readonly debt_amount: V3ActionString
  readonly source: V3ActionAddress
  readonly borrower: V3ActionAddress
  readonly borrower_contract: V3ActionAddress
  readonly collateral: V3ActionAddress
  readonly asset_id: V3ActionString
  readonly asset: V3ActionAddress
  readonly is_known_asset: boolean
  readonly amount: V3ActionString
}

export interface V3JettonAmountPair {
  readonly jetton: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsJvaultClaim {
  readonly claimed_rewards: readonly V3JettonAmountPair[]
  readonly source: V3ActionAddress
  readonly stake_wallet: V3ActionAddress
  readonly pool: V3ActionAddress
}

export interface V3ActionDetailsJvaultStake {
  readonly period: V3ActionNumber
  readonly minted_stake_jettons: V3ActionString
  readonly stake_wallet: V3ActionAddress
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsJvaultUnstake {
  readonly source: V3ActionAddress
  readonly stake_wallet: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly amount: V3ActionString
  readonly exit_code: V3ActionNumber
  readonly asset: V3ActionAddress
  readonly staking_asset: V3ActionAddress
}

export interface V3ActionDetailsNftDiscovery {
  readonly source: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly nft_item_index: V3ActionString
}

export interface V3ActionDetailsTgbtcMint {
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
  readonly asset: V3ActionAddress
  readonly bitcoin_tx_id: V3ActionString
  readonly destination_wallet: V3ActionAddress
}

export interface V3ActionDetailsTgbtcBurn {
  readonly source: V3ActionAddress
  readonly source_wallet: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
  readonly asset: V3ActionAddress
}

export interface V3ActionDetailsTgbtcNewKey {
  readonly source: V3ActionAddress
  readonly pubkey: V3ActionString
  readonly coordinator: V3ActionAddress
  readonly pegout: V3ActionAddress
  readonly amount: V3ActionString
  readonly asset: V3ActionAddress
}

export interface V3ActionDetailsDkgLogFallback {
  readonly coordinator: V3ActionAddress
  readonly pubkey: V3ActionString
  readonly timestamp: V3ActionString
}

export interface V3ActionDetailsCoffeeCreatePool {
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly initiator_1: V3ActionAddress
  readonly initiator_2: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly pool_creator_contract: V3ActionAddress
  readonly provided_asset: V3ActionAddress
  readonly amount: V3ActionString
  readonly asset_1: V3ActionAddress
  readonly asset_2: V3ActionAddress
  readonly amount_1: V3ActionString
  readonly amount_2: V3ActionString
  readonly lp_tokens_minted: V3ActionString
}

export interface V3ActionDetailsCoffeeCreatePoolCreator {
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly deposit_recipient: V3ActionAddress
  readonly pool_creator_contract: V3ActionAddress
  readonly provided_asset: V3ActionAddress
  readonly asset_1: V3ActionAddress
  readonly asset_2: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCoffeeStakingDeposit {
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly pool_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
  readonly minted_item_address: V3ActionAddress
  readonly minted_item_index: V3ActionString
}

export interface V3ActionDetailsCoffeeStakingWithdraw {
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly pool: V3ActionAddress
  readonly pool_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
  readonly nft_address: V3ActionAddress
  readonly nft_index: V3ActionString
  readonly points: V3ActionString
}

export interface V3ActionDetailsCoffeeStakingClaimRewards {
  readonly pool: V3ActionAddress
  readonly pool_jetton_wallet: V3ActionAddress
  readonly recipient: V3ActionAddress
  readonly recipient_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCoffeeMevProtectHoldFunds {
  readonly source: V3ActionAddress
  readonly source_jetton_wallet: V3ActionAddress
  readonly mev_contract: V3ActionAddress
  readonly mev_contract_jetton_wallet: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCoffeeCreateVault {
  readonly source: V3ActionAddress
  readonly vault: V3ActionAddress
  readonly asset: V3ActionAddress
  readonly value: V3ActionString
}

export interface V3ActionDetailsAuctionOutbid {
  readonly auction_address: V3ActionAddress
  readonly bidder: V3ActionAddress
  readonly new_bidder: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly amount: V3ActionString
  readonly comment?: V3ActionString
  readonly marketplace?: V3ActionString
}

export interface V3ActionDetailsNftCancelSale {
  readonly owner: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly sale_address: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly marketplace: V3ActionString
}

export interface V3ActionDetailsNftCancelAuction {
  readonly owner: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly auction_address: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly marketplace: V3ActionString
}

export interface V3ActionDetailsNftFinishAuction extends V3ActionDetailsNftCancelAuction {}

export interface V3ActionDetailsDnsRelease {
  readonly query_id: V3ActionString
  readonly source: V3ActionAddress
  readonly nft_item: V3ActionAddress
  readonly nft_collection: V3ActionAddress
  readonly nft_item_index: V3ActionString
  readonly value: V3ActionString
}

export interface V3ActionDetailsNftUpdateSale {
  readonly source: V3ActionAddress
  readonly sale_contract: V3ActionAddress
  readonly nft_address: V3ActionAddress
  readonly marketplace_address: V3ActionAddress
  readonly marketplace: V3ActionString
  readonly full_price: V3ActionString
  readonly marketplace_fee: V3ActionString
  readonly royalty_amount: V3ActionString
}

export interface V3ActionDetailsCocoonWorkerPayout {
  readonly payout_type: V3ActionString
  readonly query_id: V3ActionString
  readonly new_tokens: V3ActionString
  readonly worker_state: V3ActionNumber
  readonly worker_tokens: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCocoonProxyPayout {
  readonly query_id: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonProxyCharge {
  readonly query_id: V3ActionString
  readonly new_tokens_used: V3ActionString
  readonly expected_address: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonClientTopUp {
  readonly query_id: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCocoonRegisterProxy {
  readonly query_id: V3ActionString
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonUnregisterProxy {
  readonly query_id: V3ActionString
  readonly seqno: V3ActionNumber
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonClientRegister {
  readonly query_id: V3ActionString
  readonly nonce: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonClientChangeSecretHash {
  readonly query_id: V3ActionString
  readonly new_secret_hash: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonClientRequestRefund {
  readonly query_id: V3ActionString
  readonly via_wallet: V3ActionBoolean
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
}

export interface V3ActionDetailsCocoonGrantRefund {
  readonly query_id: V3ActionString
  readonly new_tokens_used: V3ActionString
  readonly expected_address: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCocoonClientIncreaseStake {
  readonly query_id: V3ActionString
  readonly new_stake: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export interface V3ActionDetailsCocoonClientWithdraw {
  readonly query_id: V3ActionString
  readonly withdraw_amount: V3ActionString
  readonly source: V3ActionAddress
  readonly destination: V3ActionAddress
  readonly amount: V3ActionString
}

export type V3Action =
  | V3ActionBase<"call_contract", V3ActionDetailsCallContract>
  | V3ActionBase<"contract_deploy", V3ActionDetailsContractDeploy>
  | V3ActionBase<"ton_transfer", V3ActionDetailsTonTransfer>
  | V3ActionBase<
      "extra_currency_transfer",
      V3ActionDetailsCallContract | V3ActionDetailsTonTransfer
    >
  | V3ActionBase<"auction_bid", V3ActionDetailsAuctionBid>
  | V3ActionBase<"change_dns", V3ActionDetailsChangeDns>
  | V3ActionBase<"delete_dns", V3ActionDetailsDeleteDns>
  | V3ActionBase<"renew_dns", V3ActionDetailsRenewDns>
  | V3ActionBase<"election_deposit", V3ActionDetailsElection>
  | V3ActionBase<"election_recover", V3ActionDetailsElection>
  | V3ActionBase<"jetton_burn", V3ActionDetailsJettonBurn>
  | V3ActionBase<"jetton_swap", V3ActionDetailsJettonSwap>
  | V3ActionBase<"tonco_jetton_swap", V3ActionDetailsToncoJettonSwap>
  | V3ActionBase<"jetton_transfer", V3ActionDetailsJettonTransfer>
  | V3ActionBase<"jetton_mint", V3ActionDetailsJettonMint>
  | V3ActionBase<"nft_mint", V3ActionDetailsNftMint>
  | V3ActionBase<"nft_transfer", V3ActionDetailsNftTransfer>
  | V3ActionBase<"nft_purchase", V3ActionDetailsNftTransfer>
  | V3ActionBase<"dns_purchase", V3ActionDetailsDnsPurchase>
  | V3ActionBase<"nft_put_on_sale", V3ActionDetailsNftPutOnSale>
  | V3ActionBase<"nft_put_on_auction", V3ActionDetailsNftPutOnAuction>
  | V3ActionBase<"teleitem_start_auction", V3ActionDetailsNftPutOnAuction>
  | V3ActionBase<"tick_tock", V3ActionDetailsTickTock>
  | V3ActionBase<"stake_deposit", V3ActionDetailsStakeDeposit>
  | V3ActionBase<"stake_withdrawal", V3ActionDetailsWithdrawStake>
  | V3ActionBase<"stake_withdrawal_request", V3ActionDetailsWithdrawStakeRequest>
  | V3ActionBase<"subscribe", V3ActionDetailsSubscribe>
  | V3ActionBase<"unsubscribe", V3ActionDetailsUnsubscribe>
  | V3ActionBase<"wton_mint", V3ActionDetailsWtonMint>
  | V3ActionBase<"dex_deposit_liquidity", V3ActionDetailsDexDepositLiquidity>
  | V3ActionBase<"dex_withdraw_liquidity", V3ActionDetailsDexWithdrawLiquidity>
  | V3ActionBase<"tonco_deploy_pool", V3ActionDetailsToncoDeployPool>
  | V3ActionBase<"multisig_create_order", V3ActionDetailsMultisigCreateOrder>
  | V3ActionBase<"multisig_approve", V3ActionDetailsMultisigApprove>
  | V3ActionBase<"multisig_execute", V3ActionDetailsMultisigExecute>
  | V3ActionBase<"vesting_send_message", V3ActionDetailsVestingSendMessage>
  | V3ActionBase<"vesting_add_whitelist", V3ActionDetailsVestingAddWhitelist>
  | V3ActionBase<"evaa_supply", V3ActionDetailsEvaaSupply>
  | V3ActionBase<"evaa_withdraw", V3ActionDetailsEvaaWithdraw>
  | V3ActionBase<"evaa_liquidate", V3ActionDetailsEvaaLiquidate>
  | V3ActionBase<"jvault_claim", V3ActionDetailsJvaultClaim>
  | V3ActionBase<"jvault_stake", V3ActionDetailsJvaultStake>
  | V3ActionBase<"jvault_unstake", V3ActionDetailsJvaultUnstake>
  | V3ActionBase<"jvault_unstake_request", V3ActionDetailsJvaultUnstake>
  | V3ActionBase<"nft_discovery", V3ActionDetailsNftDiscovery>
  | V3ActionBase<"tgbtc_mint", V3ActionDetailsTgbtcMint>
  | V3ActionBase<"tgbtc_mint_fallback", V3ActionDetailsTgbtcMint>
  | V3ActionBase<"tgbtc_burn", V3ActionDetailsTgbtcBurn>
  | V3ActionBase<"tgbtc_burn_fallback", V3ActionDetailsTgbtcBurn>
  | V3ActionBase<"tgbtc_new_key", V3ActionDetailsTgbtcNewKey>
  | V3ActionBase<"tgbtc_new_key_fallback", V3ActionDetailsTgbtcNewKey>
  | V3ActionBase<"tgbtc_dkg_log_fallback", V3ActionDetailsDkgLogFallback>
  | V3ActionBase<"coffee_create_pool", V3ActionDetailsCoffeeCreatePool>
  | V3ActionBase<"coffee_create_pool_creator", V3ActionDetailsCoffeeCreatePoolCreator>
  | V3ActionBase<"coffee_staking_deposit", V3ActionDetailsCoffeeStakingDeposit>
  | V3ActionBase<"coffee_staking_withdraw", V3ActionDetailsCoffeeStakingWithdraw>
  | V3ActionBase<"coffee_staking_claim_rewards", V3ActionDetailsCoffeeStakingClaimRewards>
  | V3ActionBase<"coffee_mev_protect_hold_funds", V3ActionDetailsCoffeeMevProtectHoldFunds>
  | V3ActionBase<"coffee_create_vault", V3ActionDetailsCoffeeCreateVault>
  | V3ActionBase<"auction_outbid", V3ActionDetailsAuctionOutbid>
  | V3ActionBase<"nft_cancel_sale", V3ActionDetailsNftCancelSale>
  | V3ActionBase<"nft_cancel_auction", V3ActionDetailsNftCancelAuction>
  | V3ActionBase<"teleitem_cancel_auction", V3ActionDetailsNftCancelAuction>
  | V3ActionBase<"nft_finish_auction", V3ActionDetailsNftFinishAuction>
  | V3ActionBase<"dns_release", V3ActionDetailsDnsRelease>
  | V3ActionBase<"nft_update_sale", V3ActionDetailsNftUpdateSale>
  | V3ActionBase<"layerzero_send", V3ActionDetailsLayerZeroSend>
  | V3ActionBase<"layerzero_send_tokens", V3ActionDetailsLayerZeroSendTokens>
  | V3ActionBase<"layerzero_receive", V3ActionDetailsLayerZeroReceive>
  | V3ActionBase<"layerzero_commit_packet", V3ActionDetailsLayerZeroCommitPacket>
  | V3ActionBase<"layerzero_dvn_verify", V3ActionDetailsLayerZeroDvnVerify>
  | V3ActionBase<"cocoon_worker_payout", V3ActionDetailsCocoonWorkerPayout>
  | V3ActionBase<"cocoon_proxy_payout", V3ActionDetailsCocoonProxyPayout>
  | V3ActionBase<"cocoon_proxy_charge", V3ActionDetailsCocoonProxyCharge>
  | V3ActionBase<"cocoon_client_top_up", V3ActionDetailsCocoonClientTopUp>
  | V3ActionBase<"cocoon_register_proxy", V3ActionDetailsCocoonRegisterProxy>
  | V3ActionBase<"cocoon_unregister_proxy", V3ActionDetailsCocoonUnregisterProxy>
  | V3ActionBase<"cocoon_client_register", V3ActionDetailsCocoonClientRegister>
  | V3ActionBase<"cocoon_client_change_secret_hash", V3ActionDetailsCocoonClientChangeSecretHash>
  | V3ActionBase<"cocoon_client_request_refund", V3ActionDetailsCocoonClientRequestRefund>
  | V3ActionBase<"cocoon_grant_refund", V3ActionDetailsCocoonGrantRefund>
  | V3ActionBase<"cocoon_client_increase_stake", V3ActionDetailsCocoonClientIncreaseStake>
  | V3ActionBase<"cocoon_client_withdraw", V3ActionDetailsCocoonClientWithdraw>

export interface V3TransactionsResponse {
  readonly address_book: Record<string, unknown>
  readonly transactions: readonly V3TransactionListItem[]
}

export interface V3BlockId {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number
}

export interface V3Block extends V3BlockId {
  readonly root_hash: string
  readonly file_hash: string
  readonly start_lt: string
  readonly end_lt: string
  readonly gen_utime: string | number
  readonly tx_count: number
  readonly prev_blocks?: readonly V3BlockId[]
  readonly masterchain_block_ref?: V3BlockId | null
  readonly master_ref_seqno?: number | null
  readonly after_merge?: boolean
  readonly after_split?: boolean
  readonly before_split?: boolean
  readonly flags?: number
  readonly gen_catchain_seqno?: number
  readonly global_id?: number
  readonly key_block?: boolean
  readonly min_ref_mc_seqno?: number
  readonly prev_key_block_seqno?: number
  readonly version?: number
  readonly vert_seqno?: number
  readonly vert_seqno_incr?: boolean
  readonly want_merge?: boolean
  readonly want_split?: boolean
}

export interface V3BlocksResponse {
  readonly blocks: readonly V3Block[]
}

export type StreamingFinality = "pending" | "confirmed" | "finalized"

export interface StreamingTransactionsEvent {
  readonly type: "transactions"
  readonly finality: StreamingFinality
  readonly trace_external_hash_norm?: string
  readonly transactions: readonly V3Transaction[]
}

export interface V3TransactionStoragePhase {
  readonly storage_fees_collected?: string
  readonly storage_fees_due?: string
  readonly status_change?: string
}

export interface V3TransactionComputePhase {
  readonly skipped: boolean
  readonly reason?: string
  readonly success: boolean
  readonly msg_state_used?: boolean
  readonly account_activated?: boolean
  readonly gas_fees?: string
  readonly gas_used?: string
  readonly gas_limit?: string
  readonly gas_credit?: string
  readonly mode?: number
  readonly exit_code: number
  readonly exit_arg?: number
  readonly vm_steps?: number
  readonly vm_init_state_hash?: string
  readonly vm_final_state_hash?: string
}

export interface V3TransactionActionPhase {
  readonly success: boolean
  readonly valid?: boolean
  readonly no_funds?: boolean
  readonly status_change?: string
  readonly result_code: number
  readonly result_arg?: number
  readonly tot_actions?: number
  readonly spec_actions?: number
  readonly skipped_actions?: number
  readonly msgs_created?: number
  readonly total_fwd_fees?: string
  readonly total_action_fees?: string
  readonly action_list_hash?: string
  readonly tot_msg_size?: {
    readonly cells?: string
    readonly bits?: string
  }
}

export interface V3TransactionDescription {
  readonly type: string
  readonly aborted: boolean
  readonly destroyed?: boolean
  readonly credit_first?: boolean
  readonly is_tock?: boolean
  readonly storage_ph?: V3TransactionStoragePhase
  readonly compute_ph: V3TransactionComputePhase
  readonly action: V3TransactionActionPhase
}

export interface V3TransactionListItem {
  readonly account: string
  readonly hash: string
  readonly lt: string
  readonly now: number
  readonly total_fees: string
  readonly description: V3TransactionDescription
  readonly in_msg?: V3Message | null
  readonly out_msgs: readonly V3Message[]
  readonly block_ref: {
    readonly workchain: number
    readonly shard: string
    readonly seqno: number
  }
  readonly mc_block_seqno: number
}

export interface LocalnetNodeInfo {
  readonly uptime_seconds: number
  readonly last_block_seqno: number
  readonly state_source: string
  readonly fork_network?: string | null
  readonly fork_block_number?: number | null
  readonly network_conditions?: {
    readonly response_delay_ms: number
  }
}

export type ApiCallStatus = "success" | "failed"
export type ApiCallType = "read" | "write"
export type ApiCallFamily = "control" | "emulate" | "json_rpc" | "streaming" | "v2" | "v3"

export interface ApiCallRecord {
  readonly sequence: number
  readonly status: ApiCallStatus
  readonly status_code: number
  readonly call_type: ApiCallType
  readonly api_family: ApiCallFamily
  readonly http_method: string
  readonly path: string
  readonly method: string
  readonly request_id: unknown
  readonly timestamp_ms: number
  readonly duration_ns: number
}

export interface ApiCallLogResponse {
  readonly calls: readonly ApiCallRecord[]
  readonly total_retained: number
  readonly max_retained: number
}

export interface StartupWallet {
  readonly name: string
  readonly mnemonic: readonly string[]
  readonly version: string
  readonly network: string
  readonly address: string
  readonly public_key: string
  readonly wallet_id: number
}

export interface V3RunGetMethodStackEntry {
  readonly type: string
  readonly value: unknown
}

export interface V3RunGetMethodResponse {
  readonly gas_used: number
  readonly exit_code: number
  readonly stack: readonly V3RunGetMethodStackEntry[]
  readonly vm_log: string
}

export interface V3Trace {
  readonly trace_id: string
  readonly external_hash?: string | null
  readonly mc_seqno_start: string
  readonly mc_seqno_end: string
  readonly start_lt: string
  readonly start_utime: number
  readonly end_lt: string
  readonly end_utime: number
  readonly is_incomplete: boolean
  readonly trace: V3TraceNode
  readonly transactions: Record<string, V3Transaction>
  readonly transactions_order: readonly string[]
  readonly actions: readonly V3Action[]
  readonly trace_info: {
    readonly transactions: number
    readonly messages: number
    readonly pending_messages: number
    readonly trace_state: string
    readonly classification_state: string
  }
}

export interface V3TraceNode {
  readonly tx_hash: string
  readonly in_msg_hash?: string
  readonly in_msg?: V3Message | null
  readonly transaction?: V3Transaction
  readonly children?: readonly V3TraceNode[]
}

export interface V3Transaction {
  readonly account: string
  readonly hash: string
  readonly lt: string
  readonly now: number
  readonly orig_status: string
  readonly end_status: string
  readonly total_fees: string
  readonly prev_trans_hash: string
  readonly prev_trans_lt: string
  readonly description: V3TransactionDescription
  readonly in_msg?: V3Message | null
  readonly out_msgs: readonly V3Message[]
  readonly block_ref: {
    readonly workchain: number
    readonly shard: string
    readonly seqno: number
  }
  readonly mc_block_seqno: number
  readonly child_transactions?: readonly string[] | null
  readonly account_state_before?: V3TransactionAccountState | null
  readonly account_state_after?: V3TransactionAccountState | null
}

export interface V3TransactionAccountState {
  readonly hash: string
  readonly balance: string
  readonly code_boc?: string | null
  readonly extra_currencies: Record<string, string>
  readonly account_status: string
  readonly data_boc?: string | null
  readonly frozen_hash?: string | null
  readonly data_hash?: string | null
  readonly code_hash?: string | null
}

export interface V3Message {
  readonly hash: string
  readonly opcode?: number | string | null
  readonly source?: string
  readonly destination?: string
  readonly value: string
  readonly fwd_fee: string
  readonly ihr_fee: string
  readonly import_fee: string
  readonly created_lt: string
  readonly created_at: string
  readonly ihr_disabled?: boolean
  readonly bounce: boolean
  readonly bounced: boolean
  readonly message_content: {
    readonly hash: string
    readonly body: string
  }
  readonly init_state?: {
    readonly hash: string
    readonly body: string
  }
}

export interface JettonContent {
  readonly uri?: string
  readonly name?: string
  readonly description?: string
  readonly image?: string
  readonly _image_small?: string
  readonly _image_medium?: string
  readonly _image_big?: string
  readonly symbol?: string
  readonly decimals?: string
  readonly [key: string]: unknown
}

export interface JettonMasterMetadata {
  readonly address: string
  readonly jetton_content: JettonContent
  readonly mintable?: boolean
  readonly total_supply?: string
}

export interface JettonMaster extends JettonMasterMetadata {
  readonly address: string
  readonly admin_address: string | null
  readonly code_hash: string
  readonly data_hash: string
  readonly jetton_content: JettonContent
  readonly jetton_wallet_code_hash: string
  readonly last_transaction_lt: string
  readonly mintable: boolean
  readonly total_supply: string
}

export interface JettonWallet {
  readonly address: string
  readonly balance: string
  readonly code_hash: string
  readonly data_hash: string
  readonly jetton: string
  readonly last_transaction_lt: string
  readonly master?: JettonMasterMetadata
  readonly owner: string
}

export interface JettonWalletData {
  readonly balance: string
  readonly jetton: string
  readonly owner: string
}

export interface VerificationSourceResponse {
  readonly address: string | null
  readonly code_hash: string
  readonly verified: boolean
  readonly onchain: OnchainVerification
  readonly bundles: readonly SourceBundle[]
}

export interface OnchainVerification {
  readonly master_address: string
  readonly verification_record_address: string
}

export interface SourceBundle {
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly commit: string | null
  readonly bundle_path: string
  readonly language: string
  readonly compiler_version: string
  readonly entrypoint: string
  readonly compile_params: unknown
  readonly sources: readonly SourceFileSummary[]
  readonly files: readonly SourceFile[]
}

export interface SourceFileSummary {
  readonly path: string
  readonly is_entrypoint: boolean
}

export interface SourceFile {
  readonly path: string
  readonly sha256: string
  readonly content_base64: string
  readonly content_text: string | null
}

export interface BuildSourceTraceRequest {
  readonly vm_logs: string
  readonly code_hash: string
  readonly source_bundle: SourceBundle
}

export interface SourceTraceResponse {
  readonly source_bundle_hash: string
  readonly code_hash: string
  readonly entrypoint: string
  readonly files: readonly SourceTraceFileInfo[]
  readonly steps: readonly SourceTraceStep[]
  readonly truncated: boolean
}

export interface SourceTraceFileInfo {
  readonly path: string
  readonly is_entrypoint: boolean
}

export interface SourceTraceStep {
  readonly index: number
  readonly location: SourceTraceLocation
  readonly instruction: string | null
  readonly vm_position: SourceTraceVmPosition | null
  readonly locals: readonly SourceTraceVariable[]
  readonly stack: readonly string[]
  readonly call_stack: readonly SourceTraceFrame[]
  readonly exception: SourceTraceException | null
}

export interface SourceTraceLocation {
  readonly file: string
  readonly line: number
  readonly column: number
  readonly end_line: number
  readonly end_column: number
}

export interface SourceTraceVmPosition {
  readonly cell_hash: string
  readonly offset: number
}

export interface SourceTraceFrame {
  readonly function_name: string
  readonly location: SourceTraceLocation | null
  readonly is_inlined: boolean
  readonly is_builtin: boolean
}

export interface SourceTraceVariable {
  readonly name: string
  readonly value: string
  readonly type: string | null
  readonly children: readonly SourceTraceVariable[]
}

export interface SourceTraceException {
  readonly errno: string
  readonly symbolic_name: string | null
  readonly is_uncaught: boolean
}

export interface NftCollection {
  readonly address: string
  readonly code_hash?: string
  readonly collection_content?: Record<string, unknown>
  readonly data_hash?: string
  readonly last_transaction_lt?: string
  readonly next_item_index?: string
  readonly owner_address?: string
}

export interface NftItem {
  readonly address: string
  readonly auction_contract_address?: string
  readonly code_hash: string
  readonly collection?: NftCollection | null
  readonly collection_address?: string
  readonly content: Record<string, unknown>
  readonly data_hash: string
  readonly index: string
  readonly init: boolean
  readonly last_transaction_lt: string
  readonly on_sale: boolean
  readonly owner_address?: string
  readonly real_owner?: string
  readonly sale_contract_address?: string
}
