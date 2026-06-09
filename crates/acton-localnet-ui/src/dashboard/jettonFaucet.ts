import {Address, beginCell, storeMessage, type Message} from "@ton/core"

const MINT_NEW_JETTONS_OPCODE = 0x64_2b_7d_07
const INTERNAL_TRANSFER_STEP_OPCODE = 0x17_8d_45_19

const DEFAULT_MINT_MESSAGE_VALUE = 100_000_000n
const DEFAULT_MINT_TON_AMOUNT = 50_000_000n
const DEFAULT_FORWARD_TON_AMOUNT = 20_000_000n

interface BuildJettonMintMessageParams {
  readonly minter: Address
  readonly admin: Address
  readonly recipient: Address
  readonly jettonAmount: bigint
  readonly messageValue?: bigint
  readonly mintTonAmount?: bigint
  readonly forwardTonAmount?: bigint
}

export function buildJettonMintInternalMessageBoc({
  minter,
  admin,
  recipient,
  jettonAmount,
  messageValue = DEFAULT_MINT_MESSAGE_VALUE,
  mintTonAmount = DEFAULT_MINT_TON_AMOUNT,
  forwardTonAmount = DEFAULT_FORWARD_TON_AMOUNT,
}: BuildJettonMintMessageParams): string {
  const internalTransfer = beginCell()
    .storeUint(INTERNAL_TRANSFER_STEP_OPCODE, 32)
    .storeUint(0, 64)
    .storeCoins(jettonAmount)
    .storeAddress(void 0)
    .storeAddress(void 0)
    .storeCoins(forwardTonAmount)
    .storeBit(0)
    .endCell()

  const body = beginCell()
    .storeUint(MINT_NEW_JETTONS_OPCODE, 32)
    .storeUint(0, 64)
    .storeAddress(recipient)
    .storeCoins(mintTonAmount)
    .storeRef(internalTransfer)
    .endCell()

  const message: Message = {
    info: {
      type: "internal",
      ihrDisabled: true,
      bounce: false,
      bounced: false,
      src: admin,
      dest: minter,
      value: {coins: messageValue},
      ihrFee: 0n,
      forwardFee: 0n,
      createdLt: 0n,
      createdAt: 0,
    },
    body,
  }

  return beginCell().store(storeMessage(message)).endCell().toBoc().toString("base64")
}

export function parseJettonAmount(value: string, decimals: number): bigint | undefined {
  const trimmed = value.trim()
  if (!trimmed) return undefined

  const amountPattern =
    decimals === 0 ? /^(\d+)$/ : new RegExp(`^(\\d+)(?:\\.(\\d{0,${decimals}}))?$`)
  const match = trimmed.match(amountPattern)
  if (!match) return undefined

  const [, wholePart, fractionPart = ""] = match
  const scale = 10n ** BigInt(decimals)
  const fraction = decimals === 0 ? 0n : BigInt(fractionPart.padEnd(decimals, "0"))
  const amount = BigInt(wholePart) * scale + fraction
  return amount > 0n ? amount : undefined
}

export function normalizeJettonDecimals(value: unknown): number {
  if (typeof value !== "string" || !/^\d+$/.test(value)) return 9

  const decimals = Number(value)
  if (!Number.isSafeInteger(decimals) || decimals < 0 || decimals > 30) return 9
  return decimals
}
