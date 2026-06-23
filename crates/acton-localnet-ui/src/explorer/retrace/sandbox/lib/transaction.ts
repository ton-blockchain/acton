import {type Address, Cell, type OutAction, type Transaction} from "@ton/core"

import type {ContractData} from "@retrace/sandbox/lib/contract"
import type {TestData} from "@retrace/sandbox/lib/test-data"

/**
 * Processed transaction info with all necessary transport.
 */
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
  readonly data: TransactionInfoData
  readonly parent: TransactionInfo | undefined
  readonly children: readonly TransactionInfo[]
}

export type TransactionInfoData = InternalTransactionInfoData | ExternalTransactionInfoData

export interface InternalTransactionInfoData {
  readonly dummy?: number
}

export interface ExternalTransactionInfoData {
  readonly dummy?: number
}

export type ComputeInfo =
  | "skipped"
  | {
      /**
       * If the phase is successful
       */
      readonly success: boolean
      /**
       * Exit code of this phase
       */
      readonly exitCode: number
      /**
       * Count of steps that VM executes until the end
       */
      readonly vmSteps: number
      /**
       * Gas used for this phase
       */
      readonly gasUsed: bigint
      /**
       * Gas fees for this phase
       */
      readonly gasFees: bigint
    }

export interface TransactionMoney {
  /**
   * Sum of all out internal messages values
   */
  readonly sentTotal: bigint
  /**
   * The total fees collected during the transaction execution,
   * including TON coin and potentially some extra-currencies.
   */
  readonly totalFees: bigint
  readonly forwardFee: bigint
}

/**
 * Search for ABI type for given opcode
 */
export function findOpcodeABI(tx: TransactionInfo, contracts: Map<string, ContractData>) {
  if (tx.opcode === undefined) return undefined

  const txAddress = tx?.address
  if (!txAddress) return undefined

  const contract = contracts.get(txAddress.toString())
  if (contract?.meta?.abi) {
    // first search in abi of tx contract
    const found = contract?.meta?.abi.types?.find(it => it.header === tx.opcode)
    if (found) return found
  }

  for (const contract of [...contracts.values()]) {
    // but if we send a message from another contract, we may need to search everywhere
    const found = contract?.meta?.abi?.types?.find(it => it.header === tx.opcode)
    if (found) return found
  }
  return undefined
}

export function computeSendMode(tx: TransactionInfo, test: TestData) {
  // X -> Y -> Z
  //      |  ^ for this
  //      |
  //      sender
  const sender = tx.transaction.inMessage?.info?.src
  if (!sender) return undefined

  const txsToSender = test.transactions.filter(
    it => it.transaction.inMessage?.info?.dest?.toString() === sender?.toString(),
  )

  if (txsToSender.length === 0) return undefined

  for (const txToSender of txsToSender) {
    const outActions = txToSender.outActions
    if (outActions.length === 0) continue

    const sendMessages = outActions.filter(it => it.type === "sendMsg")
    if (sendMessages.length === 0) continue

    for (const sendMessage of sendMessages) {
      if (sendMessage.outMsg?.info?.dest?.toString() === tx.address?.toString()) {
        return sendMessage.mode as number
      }
    }
  }

  return undefined
}

export const SEND_MODE_CONSTANTS = {
  0: {
    name: "SendDefaultMode",
    description:
      "Ordinary message (default).\n\nSee: <https://docs.tact-lang.org/book/message-mode#base-modes>",
  },
  64: {
    name: "SendRemainingValue",
    description:
      "Carry all the remaining value of the inbound message in addition to the value initially indicated in the new message.\n\nSee: <https://docs.tact-lang.org/book/message-mode#base-modes>",
  },
  128: {
    name: "SendRemainingBalance",
    description:
      "Carry **all the remaining balance** of the current smart contract instead of the value originally indicated in the message.\n\nSee: <https://docs.tact-lang.org/book/message-mode#base-modes>",
  },
  1024: {
    name: "SendOnlyEstimateFee",
    description:
      "Doesn't send the message, only estimates the forward fees if the message-sending function computes those.\n\nSee:\n* <https://docs.tact-lang.org/book/message-mode#base-modes>\n* <https://docs.tact-lang.org/book/send#message-sending-functions>",
  },
  1: {
    name: "SendPayFwdFeesSeparately",
    description:
      "Pay forward fees separately from the message value.\n\nSee: <https://docs.tact-lang.org/book/message-mode#optional-flags>",
  },
  2: {
    name: "SendIgnoreErrors",
    description:
      "Ignore any errors arising while processing this message during the action phase.\n\nSee: <https://docs.tact-lang.org/book/message-mode#optional-flags>",
  },
  16: {
    name: "SendBounceIfActionFail",
    description:
      "Bounce transaction in case of any errors during action phase. Has no effect if flag +2, `SendIgnoreErrors` is used.\n\nSee: <https://docs.tact-lang.org/book/message-mode#optional-flags>",
  },
  32: {
    name: "SendDestroyIfZero",
    description:
      "Current account (contract) will be destroyed if its resulting balance is zero. This flag is often used with mode 128, `SendRemainingBalance`.\n\nSee: <https://docs.tact-lang.org/book/message-mode#optional-flags>",
  },
} as const

export interface SendModeInfo {
  readonly name: string
  readonly value: number
  readonly description: string
}

/**
 * Parse sends mode number into an array of constants
 */
export function parseSendMode(mode: number): SendModeInfo[] {
  const flags: SendModeInfo[] = []

  for (const [value, constant] of Object.entries(SEND_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value)
    if (mode & flagValue) {
      flags.push({
        name: constant.name,
        value: flagValue,
        description: constant.description,
      })
    }
  }

  if (flags.length === 0 && mode === 0) {
    flags.push({
      name: SEND_MODE_CONSTANTS[0].name,
      value: 0,
      description: SEND_MODE_CONSTANTS[0].description,
    })
  }

  return flags
}

export const RESERVE_MODE_CONSTANTS = {
  0: {
    name: "ReserveExact",
    description: "Reserves exactly the specified amount of nanoToncoin.",
  },
  1: {
    name: "ReserveAllExcept",
    description: "Reserves all but the specified amount of nanoToncoin.",
  },
  2: {
    name: "ReserveAtMost",
    description: "Reserves at most the specified amount of nanoToncoin.",
  },
  4: {
    name: "ReserveAddOriginalBalance",
    description:
      "Increases the amount by the original balance of the current account (before the compute phase), including all extra currencies.",
  },
  8: {
    name: "ReserveInvertSign",
    description: "Negates the amount value before performing the reservation.",
  },
  16: {
    name: "ReserveBounceIfActionFail",
    description: "Bounces the transaction if the reservation fails.",
  },
} as const

export interface ReserveModeInfo {
  readonly name: string
  readonly value: number
  readonly description: string
}

/**
 * Parse reserve mode number into an array of constants
 */
export function parseReserveMode(mode: number): ReserveModeInfo[] {
  const flags: ReserveModeInfo[] = []

  // Check base modes (mutually exclusive)
  if ((mode & 3) === 0) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[0].name,
      value: 0,
      description: RESERVE_MODE_CONSTANTS[0].description,
    })
  } else if ((mode & 3) === 1) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[1].name,
      value: 1,
      description: RESERVE_MODE_CONSTANTS[1].description,
    })
  } else if ((mode & 3) === 2) {
    flags.push({
      name: RESERVE_MODE_CONSTANTS[2].name,
      value: 2,
      description: RESERVE_MODE_CONSTANTS[2].description,
    })
  }

  // Check optional flags
  for (const [value, constant] of Object.entries(RESERVE_MODE_CONSTANTS)) {
    const flagValue = Number.parseInt(value)
    if (flagValue >= 4 && mode & flagValue) {
      flags.push({
        name: constant.name,
        value: flagValue,
        description: constant.description,
      })
    }
  }

  return flags
}
