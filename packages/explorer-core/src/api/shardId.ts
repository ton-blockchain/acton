export function v2ShardToV3Shard(shard: string): string {
  return BigInt.asUintN(64, BigInt(shard)).toString(16).padStart(16, "0").toUpperCase()
}

export function v3ShardToV2Shard(shard: string): string {
  return BigInt.asIntN(64, BigInt(`0x${shard}`)).toString()
}
