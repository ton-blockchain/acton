import {toNano} from "@ton/core"

export function ton(value: bigint | number | string): bigint {
  return toNano(value)
}
