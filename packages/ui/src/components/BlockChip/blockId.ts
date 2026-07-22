export interface ToncenterBlockId {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number | string
}

export function formatToncenterBlockId({workchain, shard, seqno}: ToncenterBlockId): string {
  return `(${workchain},${shard},${seqno})`
}
