import {invoke} from "@tauri-apps/api/core"
import {
  ApiClientToncenter,
  Network,
  parseUnits,
  SendModeBase,
  SendModeFlag,
  WalletV4R2Adapter,
  WalletV5R1Adapter,
  wrapWalletInterface,
  type TransactionRequest,
  type Wallet,
  type WalletSigner,
} from "@ton/walletkit"

import {WALLET_NETWORKS, type WalletRecord} from "../domain/wallet"
import {isTauriRuntime} from "./walletVault"

export interface GramTransferInput {
  readonly recipient: string
  readonly amount: string
  readonly comment?: string
}

export interface GramTransferPreview {
  readonly inputNano?: string
  readonly outputNano?: string
}

export interface SentGramTransfer {
  readonly messageHash: string
}

export async function previewGramTransfer(
  record: WalletRecord,
  input: GramTransferInput,
): Promise<GramTransferPreview> {
  const wallet = await createPreviewWallet(record)
  const request = await createTransferRequest(wallet, input)
  const preview = await wallet.getTransactionPreview(request)

  if (preview.result !== "success") {
    throw new Error(preview.error?.message ?? "The transaction would fail on-chain.")
  }

  return {
    inputNano: preview.moneyFlow?.inputs,
    outputNano: preview.moneyFlow?.outputs,
  }
}

export async function sendGramTransfer(
  record: WalletRecord,
  input: GramTransferInput,
): Promise<SentGramTransfer> {
  if (!isTauriRuntime()) {
    throw new Error("Open the desktop app to sign and submit this transfer.")
  }
  const amountNano = parseTransferAmount(input.amount)

  return await invoke<SentGramTransfer>("send_gram_transfer", {
    request: {
      walletId: record.id,
      recipient: input.recipient.trim(),
      amountNano,
      comment: input.comment?.trim() || undefined,
    },
  })
}

async function createPreviewWallet(record: WalletRecord): Promise<Wallet> {
  const signer: WalletSigner = {
    publicKey: record.publicKey as WalletSigner["publicKey"],
    sign: () => Promise.reject(new Error("Preview signing is unavailable")),
  }
  const networkDefinition = WALLET_NETWORKS[record.network]
  const network = record.network === "mainnet" ? Network.mainnet() : Network.testnet()
  const client = new ApiClientToncenter({
    endpoint: networkDefinition.endpoint,
    network,
  })
  const adapter =
    record.version === "v4r2"
      ? await WalletV4R2Adapter.create(signer, {client, network})
      : await WalletV5R1Adapter.create(signer, {client, network})

  if (adapter.getAddress({testnet: record.network === "testnet"}) !== record.address) {
    throw new Error("The public key does not match this wallet address.")
  }

  return await wrapWalletInterface(adapter)
}

async function createTransferRequest(
  wallet: Wallet,
  input: GramTransferInput,
): Promise<TransactionRequest> {
  return await wallet.createTransferTonTransaction({
    recipientAddress: input.recipient.trim(),
    transferAmount: parseTransferAmount(input.amount),
    comment: input.comment?.trim() || undefined,
    mode: {
      base: SendModeBase.ORDINARY,
      flags: [SendModeFlag.PAY_GAS_SEPARATELY, SendModeFlag.IGNORE_ERRORS],
    },
  })
}

function parseTransferAmount(amount: string): string {
  const amountNano = parseUnits(amount.trim(), 9)
  if (amountNano <= 0n) {
    throw new Error("Enter an amount greater than zero.")
  }
  return amountNano.toString()
}
