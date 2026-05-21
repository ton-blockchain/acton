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

export interface AccountAddress {
  readonly "@type": "accountAddress"
  readonly account_address: string
}

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

export interface Message {
  readonly "@type": "raw.message" | "msg.message"
  readonly hash?: string
  readonly opcode?: string
  readonly source?: string
  readonly destination?: string
  readonly value?: string
  readonly fwd_fee?: string
  readonly ihr_fee?: string
  readonly created_lt?: string
  readonly body_hash?: string
  readonly msg_data?: {
    readonly "@type": "msg.dataRaw"
    readonly body: string
    readonly init_state: string
  }
  readonly extra_currencies?: readonly unknown[]
}

export interface Transaction {
  readonly "@type": "ext.transaction"
  readonly hash: string
  readonly address: AccountAddress
  readonly account: string
  readonly utime: number
  readonly data: string
  readonly success: boolean
  readonly exit_code: number
  readonly transaction_id: TransactionId
  readonly fee: string
  readonly storage_fee: string
  readonly other_fee: string
  readonly in_msg: Message
  readonly out_msgs: readonly Message[]
}

export interface FullAccountState {
  readonly "@type": "raw.fullAccountState"
  readonly balance: string
  readonly extra_currencies: readonly unknown[]
  readonly last_transaction_id: TransactionId
  readonly block_id: BlockId
  readonly code: string
  readonly data: string
  readonly frozen_hash: string
  readonly sync_utime: number
  readonly state: "active" | "uninitialized" | "frozen" | "nonexist"
}

export interface AccountStateTokenInfo {
  readonly type: string
  readonly [key: string]: unknown
}

export interface AccountStatesAddressBookRow {
  readonly user_friendly: string
  readonly domain?: string | null
  readonly interfaces: readonly string[]
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
  readonly interfaces: readonly string[]
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
  readonly metadata: Record<string, unknown>
  readonly traces: readonly V3Trace[]
}

export interface V3TransactionsResponse {
  readonly address_book: Record<string, unknown>
  readonly transactions: readonly V3TransactionListItem[]
}

export interface V3TransactionListItem {
  readonly account: string
  readonly hash: string
  readonly lt: string
  readonly now: number
  readonly total_fees: string
  readonly description: {
    readonly type: string
    readonly aborted: boolean
    readonly compute_ph: {
      readonly skipped: boolean
      readonly success: boolean
      readonly exit_code: number
    }
    readonly action: {
      readonly success: boolean
      readonly result_code: number
    }
  }
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
}

export interface V3Trace {
  readonly trace_id: string
  readonly external_hash: string
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
  readonly actions: readonly unknown[]
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
  readonly in_msg_hash: string
  readonly in_msg?: V3Message | null
  readonly transaction: V3Transaction
  readonly children: readonly V3TraceNode[]
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
  readonly description: {
    readonly type: string
    readonly aborted: boolean
    readonly compute_ph: {
      readonly skipped: boolean
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
    readonly action: {
      readonly success: boolean
      readonly valid?: boolean
      readonly no_funds?: boolean
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
    readonly credit_first?: boolean
    readonly destroyed?: boolean
  }
  readonly in_msg?: V3Message | null
  readonly out_msgs: readonly V3Message[]
  readonly block_ref: {
    readonly workchain: number
    readonly shard: string
    readonly seqno: number
  }
  readonly mc_block_seqno: number
  readonly child_transactions: readonly string[]
}

export interface V3Message {
  readonly hash: string
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

export interface JettonMaster {
  readonly address: string
  readonly admin_address: string
  readonly code_hash: string
  readonly data_hash: string
  readonly jetton_content: {
    readonly uri?: string
    readonly name?: string
    readonly description?: string
    readonly image?: string
    readonly symbol?: string
    readonly decimals?: string
    readonly [key: string]: unknown
  }
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
  readonly owner: string
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
