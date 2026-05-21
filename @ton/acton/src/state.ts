import {Buffer} from "node:buffer"

import type {ContractState} from "@ton/core"

import type {AccountInfoResult} from "./types.js"

export function toContractState(info: AccountInfoResult): ContractState {
  const last = toLastTransaction(info.last_transaction_id)

  if (info.state === "active") {
    return {
      balance: BigInt(info.balance),
      extracurrency: null,
      last,
      state: {
        code: decodeMaybeBase64(info.code),
        data: decodeMaybeBase64(info.data),
        type: "active",
      },
    }
  }

  if (info.state === "frozen") {
    return {
      balance: BigInt(info.balance),
      extracurrency: null,
      last,
      state: {
        stateHash: decodeMaybeBase64(info.frozen_hash) ?? Buffer.alloc(32),
        type: "frozen",
      },
    }
  }

  return {
    balance: BigInt(info.balance),
    extracurrency: null,
    last,
    state: {type: "uninit"},
  }
}

function toLastTransaction(id: AccountInfoResult["last_transaction_id"]): ContractState["last"] {
  const lt = BigInt(id.lt)
  const hash = decodeMaybeBase64(id.hash) ?? Buffer.alloc(32)
  return lt === 0n && hash.every((byte: number) => byte === 0) ? null : {hash, lt}
}

function decodeMaybeBase64(value: string): Buffer | null {
  return value.length === 0 ? null : Buffer.from(value, "base64")
}
