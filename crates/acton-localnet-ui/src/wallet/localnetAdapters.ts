import {
  Address,
  beginCell,
  Cell,
  external,
  internal,
  loadStateInit,
  type MessageRelaxed,
  SendMode,
  storeMessage,
  storeMessageRelaxed,
} from "@ton/core"
import {
  type ApiClient,
  type Base64String,
  CallForSuccess,
  HexToUint8Array,
  type Network,
  type TransactionRequest,
  type WalletSigner,
  WalletV4R2Adapter,
  WalletV5R1Adapter,
} from "@ton/walletkit"

export const LOCALNET_WALLET_VALID_UNTIL = 0xff_ff_ff_ff

const WALLET_SEND_MODE = SendMode.PAY_GAS_SEPARATELY + SendMode.IGNORE_ERRORS
const WALLET_V5_ACTION_SEND_MSG = 0x0e_c3_c8_6d

interface CreateAdapterOptions {
  readonly client: ApiClient
  readonly network: Network
  readonly walletId?: number | bigint
  readonly workchain?: number
}

type SendTransactionOptions = {
  readonly fakeSignature: boolean
}

export async function createLocalnetWalletV4R2Adapter(
  signer: WalletSigner,
  options: CreateAdapterOptions,
): Promise<WalletV4R2Adapter> {
  const adapter = await WalletV4R2Adapter.create(signer, options)

  adapter.getSignedSendTransaction = async (
    input: TransactionRequest,
    _options?: SendTransactionOptions,
  ): Promise<Base64String> => {
    if (input.messages.length === 0) {
      throw new Error("WalletV4R2 does not support empty messages")
    }
    if (input.messages.length > 4) {
      throw new Error("WalletV4R2 does not support more than 4 messages")
    }

    const seqno = await getSeqno(adapter)
    const messages = input.messages.map(createWalletMessage)
    const walletContract = adapter.getContract()
    const data = walletContract.createTransfer({
      seqno,
      sendMode: WALLET_SEND_MODE,
      messages,
      timeout: LOCALNET_WALLET_VALID_UNTIL,
    })
    const signature = await adapter.sign(Uint8Array.from(data.hash()))
    const signedCell = beginCell()
      .storeBuffer(Buffer.from(HexToUint8Array(signature)))
      .storeSlice(data.asSlice())
      .endCell()
    const ext = external({
      to: walletContract.address,
      init: walletContract.init,
      body: signedCell,
    })
    return beginCell().store(storeMessage(ext)).endCell().toBoc().toString("base64") as Base64String
  }

  return adapter
}

export async function createLocalnetWalletV5R1Adapter(
  signer: WalletSigner,
  options: CreateAdapterOptions,
): Promise<WalletV5R1Adapter> {
  const adapter = await WalletV5R1Adapter.create(signer, options)

  adapter.getSignedSendTransaction = async (
    input: TransactionRequest,
    options?: SendTransactionOptions,
  ): Promise<Base64String> => {
    const actions = packWalletV5Actions(input.messages.map(createWalletMessage))
    const seqno = await getSeqno(adapter)
    const walletContract = adapter.getContract()
    const walletId = (await walletContract.walletId).serialized

    if (!walletId) {
      throw new Error("Failed to get walletId")
    }

    const transfer = await adapter.createBodyV5(seqno, walletId, actions, {
      fakeSignature: options?.fakeSignature ?? false,
      validUntil: LOCALNET_WALLET_VALID_UNTIL,
    })
    const ext = external({
      to: walletContract.address,
      init: walletContract.init,
      body: transfer,
    })
    return beginCell().store(storeMessage(ext)).endCell().toBoc().toString("base64") as Base64String
  }

  return adapter
}

async function getSeqno(adapter: {getSeqno(): Promise<number>}): Promise<number> {
  try {
    return await CallForSuccess(async () => adapter.getSeqno(), 5, 1000)
  } catch {
    return 0
  }
}

function createWalletMessage(message: TransactionRequest["messages"][number]): MessageRelaxed {
  const parsedAddress = Address.parseFriendly(message.address)
  return internal({
    to: Address.parse(message.address),
    value: BigInt(message.amount),
    bounce: parsedAddress.isBounceable !== false,
    extracurrency: message.extraCurrency
      ? Object.fromEntries(
          Object.entries(message.extraCurrency).map(([currency, amount]) => [
            Number(currency),
            BigInt(amount),
          ]),
        )
      : undefined,
    body: message.payload ? Cell.fromBase64(message.payload) : undefined,
    init: message.stateInit
      ? loadStateInit(Cell.fromBase64(message.stateInit).asSlice())
      : undefined,
  })
}

function packWalletV5Actions(messages: readonly MessageRelaxed[]): Cell {
  const actions = messages.map(message =>
    beginCell()
      .storeUint(WALLET_V5_ACTION_SEND_MSG, 32)
      .storeUint(WALLET_SEND_MODE, 8)
      .storeRef(beginCell().store(storeMessageRelaxed(message)).endCell())
      .endCell(),
  )

  const builder = beginCell()
  if (actions.length === 0) {
    builder.storeUint(0, 1)
  } else {
    builder.storeMaybeRef(packWalletV5OutActions(actions.slice().reverse()))
  }

  return builder.storeUint(0, 1).endCell()
}

function packWalletV5OutActions(actions: readonly Cell[]): Cell {
  if (actions.length === 0) {
    return beginCell().endCell()
  }

  const [action, ...rest] = actions
  return beginCell()
    .storeRef(packWalletV5OutActions(rest))
    .storeSlice(action.beginParse())
    .endCell()
}
