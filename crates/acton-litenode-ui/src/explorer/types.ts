export interface AccountAddress {
  "@type": "accountAddress"
  account_address: string
}

export interface TransactionId {
  "@type": "internal.transactionId"
  lt: string
  hash: string
}

export interface BlockId {
  "@type": "ton.blockIdExt"
  workchain: number
  shard: string
  seqno: number
  root_hash: string
  file_hash: string
}

export interface Message {
  "@type": "raw.message" | "msg.message"
  hash?: string
  opcode?: string
  source?: AccountAddress
  destination?: AccountAddress
  value?: string
  fwd_fee?: string
  ihr_fee?: string
  created_lt?: string
  body_hash?: string
  msg_data?: {
    "@type": "msg.dataRaw"
    body: string
    init_state: string
  }
}

export interface Transaction {
  "@type": "ext.transaction"
  hash: string
  address: AccountAddress
  account: string
  utime: number
  data: string
  success: boolean
  exit_code: number
  transaction_id: TransactionId
  fee: string
  storage_fee: string
  other_fee: string
  in_msg: Message
  out_msgs: Message[]
}

export interface FullAccountState {
  "@type": "raw.fullAccountState"
  balance: string
  last_transaction_id: TransactionId
  block_id: BlockId
  code: string
  data: string
  frozen_hash: string
  sync_utime: number
  state: "active" | "uninitialized" | "frozen" | "nonexist"
}
