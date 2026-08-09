import type {TransactionBlockRef, TransactionInfo} from "@acton/transaction-ui"

export interface TraceShardFlowSegment {
  readonly workchain: number
  readonly shard: string
  readonly transactionCount: number
  readonly transactionIds: readonly string[]
  readonly blocks: readonly TransactionBlockRef[]
}

export interface TraceShardFlow {
  readonly shardCount: number
  readonly transactionCount: number
  readonly segments: readonly TraceShardFlowSegment[]
}

type TraceShardTransaction = Pick<TransactionInfo, "blockRef" | "id" | "lt">

const shardKey = ({workchain, shard}: Pick<TransactionBlockRef, "workchain" | "shard">): string =>
  `${workchain}:${shard}`

const blockKey = ({workchain, shard, seqno}: TransactionBlockRef): string =>
  `${workchain}:${shard}:${seqno}`

export function buildTraceShardFlow(
  transactions: readonly TraceShardTransaction[],
): TraceShardFlow {
  const orderedTransactions = transactions
    .filter(
      (
        transaction,
      ): transaction is TraceShardTransaction & {readonly blockRef: TransactionBlockRef} =>
        transaction.blockRef !== undefined,
    )
    .map((transaction, inputIndex) => ({transaction, inputIndex}))
    .sort((left, right) => {
      const leftLt = BigInt(left.transaction.lt)
      const rightLt = BigInt(right.transaction.lt)
      if (leftLt === rightLt) {
        return left.inputIndex - right.inputIndex
      }
      return leftLt < rightLt ? -1 : 1
    })

  const segments: Array<{
    workchain: number
    shard: string
    transactionCount: number
    transactionIds: string[]
    blocks: TransactionBlockRef[]
    blockKeys: Set<string>
  }> = []
  const shardKeys = new Set<string>()

  for (const {transaction} of orderedTransactions) {
    const {blockRef} = transaction
    const currentShardKey = shardKey(blockRef)
    shardKeys.add(currentShardKey)

    let segment = segments.at(-1)
    if (!segment || shardKey(segment) !== currentShardKey) {
      segment = {
        workchain: blockRef.workchain,
        shard: blockRef.shard,
        transactionCount: 0,
        transactionIds: [],
        blocks: [],
        blockKeys: new Set<string>(),
      }
      segments.push(segment)
    }

    segment.transactionCount += 1
    segment.transactionIds.push(transaction.id)
    const currentBlockKey = blockKey(blockRef)
    if (!segment.blockKeys.has(currentBlockKey)) {
      segment.blockKeys.add(currentBlockKey)
      segment.blocks.push(blockRef)
    }
  }

  return {
    shardCount: shardKeys.size,
    transactionCount: orderedTransactions.length,
    segments: segments.map(({blockKeys: _, ...segment}) => segment),
  }
}
