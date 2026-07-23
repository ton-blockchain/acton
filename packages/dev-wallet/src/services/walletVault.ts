import {invoke} from "@tauri-apps/api/core"

import type {WalletRecord} from "../domain/wallet"

const BROWSER_WALLETS_KEY = "acton-dev-wallet:wallets"
const BROWSER_SECRETS_KEY = "acton-dev-wallet:session-secrets"

interface BrowserSecrets {
  readonly [walletId: string]: string
}

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in globalThis
}

export function getVaultKind(): "native" | "browser-preview" {
  return isTauriRuntime() ? "native" : "browser-preview"
}

export async function listWallets(): Promise<readonly WalletRecord[]> {
  if (isTauriRuntime()) {
    return invoke<WalletRecord[]>("list_wallets")
  }

  try {
    const serialized = localStorage.getItem(BROWSER_WALLETS_KEY)
    return serialized ? (JSON.parse(serialized) as WalletRecord[]) : []
  } catch {
    return []
  }
}

export async function saveWallet(record: WalletRecord, mnemonic: readonly string[]): Promise<void> {
  const secret = mnemonic.join(" ")
  if (isTauriRuntime()) {
    await invoke("save_wallet", {record, mnemonic: secret})
    return
  }

  const wallets = await listWallets()
  const nextWallets = [...wallets.filter(wallet => wallet.id !== record.id), record]
  localStorage.setItem(BROWSER_WALLETS_KEY, JSON.stringify(nextWallets))

  const secrets = readBrowserSecrets()
  sessionStorage.setItem(BROWSER_SECRETS_KEY, JSON.stringify({...secrets, [record.id]: secret}))
}

export async function removeWallet(walletId: string): Promise<void> {
  if (isTauriRuntime()) {
    await invoke("remove_wallet", {walletId})
    return
  }

  const wallets = await listWallets()
  localStorage.setItem(
    BROWSER_WALLETS_KEY,
    JSON.stringify(wallets.filter(wallet => wallet.id !== walletId)),
  )
  const secrets = {...readBrowserSecrets()}
  delete secrets[walletId]
  sessionStorage.setItem(BROWSER_SECRETS_KEY, JSON.stringify(secrets))
}

function readBrowserSecrets(): BrowserSecrets {
  try {
    const serialized = sessionStorage.getItem(BROWSER_SECRETS_KEY)
    return serialized ? (JSON.parse(serialized) as BrowserSecrets) : {}
  } catch {
    return {}
  }
}
