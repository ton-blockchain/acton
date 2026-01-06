import type { Address, Cell, OutAction, Transaction } from "@ton/core"

export interface DeployedContract {
  readonly name: string
  readonly address: string
  readonly abi?: any
}

export interface TransactionInfo {
  readonly address: Address | undefined
  readonly transaction: Transaction
  readonly fields: Record<string, unknown>
  readonly opcode: number | undefined
  readonly computeInfo: ComputeInfo
  readonly money: TransactionMoney
  readonly amount: bigint | undefined
  readonly outActions: OutAction[]
  readonly c5: Cell | undefined
  readonly code: Cell | undefined
  readonly sourceMap: any | undefined
  readonly contractName: string | undefined
  readonly parent: TransactionInfo | undefined
  readonly children: readonly TransactionInfo[]
  readonly oldStorage: Cell | undefined
  readonly newStorage: Cell | undefined
  readonly callStack: string | undefined
}

export type ComputeInfo =
  | "skipped"
  | {
      readonly success: boolean
      readonly exitCode: number
      readonly vmSteps: number
      readonly gasUsed: bigint
      readonly gasFees: bigint
    }

export interface TransactionMoney {
  readonly sentTotal: bigint
  readonly totalFees: bigint
  readonly forwardFee: bigint
}

export const SEND_MODE_CONSTANTS = {
  0: { name: "SendDefaultMode", description: "Ordinary message (default)." },
  64: {
    name: "SendRemainingValue",
    description: "Carry all the remaining value of the inbound message.",
  },
  128: {
    name: "SendRemainingBalance",
    description: "Carry all the remaining balance of the current smart contract.",
  },
  1: {
    name: "SendPayFwdFeesSeparately",
    description: "Pay forward fees separately from the message value.",
  },
  2: {
    name: "SendIgnoreErrors",
    description: "Ignore any errors arising while processing this message.",
  },
  16: {
    name: "SendBounceIfActionFail",
    description: "Bounce transaction in case of any errors during action phase.",
  },
  32: {
    name: "SendDestroyIfZero",
    description: "Current account will be destroyed if its resulting balance is zero.",
  },
} as const

export function parseSendMode(mode: number) {
  const flags = []
  for (const [value, constant] of Object.entries(SEND_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value)
    if (flagValue === 0) continue
    if (mode & flagValue) {
      flags.push({ name: constant.name, value: flagValue, description: constant.description })
    }
  }
  if (flags.length === 0 && (mode === 0 || mode === undefined)) {
    flags.push({
      name: SEND_MODE_CONSTANTS[0].name,
      value: 0,
      description: SEND_MODE_CONSTANTS[0].description,
    })
  }
  return flags
}

export interface ContractData {
  displayName: string
  address: Address
  letter: string
  abi?: any
}
