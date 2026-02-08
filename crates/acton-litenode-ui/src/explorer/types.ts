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
  readonly source?: AccountAddress
  readonly destination?: AccountAddress
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
