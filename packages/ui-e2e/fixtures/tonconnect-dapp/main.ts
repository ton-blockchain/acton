import {TonConnect, type SignDataPayload, type SignDataResponse} from "@tonconnect/sdk"

import {SIGN_DATA_CELL, SIGN_DATA_CELL_SCHEMA, SIGN_DATA_TEXT} from "./signDataPayloads"

type SignatureKind = "cell" | "text"

const bridgeUrl = import.meta.env.VITE_TON_CONNECT_BRIDGE_URL

const status = getElement<HTMLParagraphElement>("status")
const connectButton = getElement<HTMLButtonElement>("connect")
const connectionUrl = getElement<HTMLTextAreaElement>("connection-url")
const signTextButton = getElement<HTMLButtonElement>("sign-text")
const signCellButton = getElement<HTMLButtonElement>("sign-cell")
const result = getElement<HTMLPreElement>("result")

if (!bridgeUrl) {
  throw new Error("VITE_TON_CONNECT_BRIDGE_URL is required")
}

const connector = new TonConnect({
  manifestUrl: new URL("/tonconnect-manifest.json", globalThis.location.origin).toString(),
  analytics: {mode: "off"},
})
connector.setConnectionNetwork("-3")

connector.onStatusChange(wallet => {
  const account = wallet?.account
  status.textContent = account ? `Connected: ${account.address}` : "Not connected"
  setSignatureButtonsDisabled(!account)
})

connectButton.addEventListener("click", () => {
  connectionUrl.value = connector.connect({
    universalLink: "tc://",
    bridgeUrl,
  })
  status.textContent = "Waiting for wallet approval"
})

signTextButton.addEventListener("click", () => requestSignature("text"))
signCellButton.addEventListener("click", () => requestSignature("cell"))

async function requestSignature(kind: SignatureKind): Promise<void> {
  const {account} = connector
  if (!account) {
    return
  }

  const payload: SignDataPayload =
    kind === "text"
      ? {
          type: "text",
          text: SIGN_DATA_TEXT,
          network: account.chain,
          from: account.address,
        }
      : {
          type: "cell",
          schema: SIGN_DATA_CELL_SCHEMA,
          cell: SIGN_DATA_CELL,
          network: account.chain,
          from: account.address,
        }

  setSignatureButtonsDisabled(true)
  status.textContent = `Waiting for ${kind} signature`
  result.textContent = `${kind} signature requested`

  try {
    const signed = await connector.signData(payload)
    showSignatureResult(signed)
    status.textContent = `${kind === "cell" ? "Cell" : "Text"} signature received`
  } catch (error) {
    result.textContent = error instanceof Error ? error.message : String(error)
    status.textContent = "Signature failed"
  } finally {
    setSignatureButtonsDisabled(false)
  }
}

function setSignatureButtonsDisabled(disabled: boolean): void {
  signTextButton.disabled = disabled
  signCellButton.disabled = disabled
}

function showSignatureResult(signed: SignDataResponse): void {
  result.textContent = JSON.stringify(signed, null, 2)
}

function getElement<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id)
  if (!element) {
    throw new Error(`Missing #${id}`)
  }
  return element as T
}
