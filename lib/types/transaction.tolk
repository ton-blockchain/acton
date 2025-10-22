struct (0b00) Uninitialized
struct (0b01) Frozen
struct (0b10) Active
struct (0b11) NonExisting

type TransactionList = tuple

type AccountStatus = Uninitialized | Frozen | Active | NonExisting

struct Transaction {
    tag: int4 // 0x07
    address: uint256
    lt: uint64
    prevTransactionHash: uint256
    prevTransactionLt: uint64
    now: uint32
    outMessagesCount: uint15
    oldStatus: AccountStatus
    endStatus: AccountStatus
}
