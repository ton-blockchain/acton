import type {TonClient} from "./client"
import type {JettonMasterMetadata, JettonWallet} from "./types"

import {toRawAddress} from "../components/utils"

const MAINNET_USDT_MASTER_RAW_ADDRESS =
  "0:b113a994b5024a16719f69139328eb759596c38a25f59028b146fecdc3621dfe"

export async function loadJettonWalletsWithMasters(
  client: TonClient,
  ownerAddresses: readonly string[],
): Promise<readonly JettonWallet[]> {
  const tokenWallets = await client.getJettonWallets([...ownerAddresses])
  const missingJettonAddresses = new Set<string>()
  for (const tokenWallet of tokenWallets) {
    if (!tokenWallet.master) {
      missingJettonAddresses.add(tokenWallet.jetton)
    }
  }

  if (missingJettonAddresses.size === 0) {
    return tokenWallets
  }

  const missingMasters = await client.getJettonMasters([...missingJettonAddresses])
  const missingMastersByAddress = new Map(
    missingMasters.map(master => [toRawAddress(master.address), master] as const),
  )

  return tokenWallets.map(tokenWallet => ({
    ...tokenWallet,
    master: tokenWallet.master ?? missingMastersByAddress.get(toRawAddress(tokenWallet.jetton)),
  }))
}

/** Orders token wallets for user-facing lists, keeping mainnet USD₮ discoverable */
export function sortJettonWalletsForDisplay(wallets: readonly JettonWallet[]): JettonWallet[] {
  return [...wallets].sort(compareJettonWalletDisplayOrder)
}

function compareJettonWalletDisplayOrder(left: JettonWallet, right: JettonWallet): number {
  const leftIsUsdt = toRawAddress(left.jetton) === MAINNET_USDT_MASTER_RAW_ADDRESS
  const rightIsUsdt = toRawAddress(right.jetton) === MAINNET_USDT_MASTER_RAW_ADDRESS
  if (leftIsUsdt !== rightIsUsdt) {
    return leftIsUsdt ? -1 : 1
  }

  return compareJettonWalletAmount(left, right)
}

function compareJettonWalletAmount(left: JettonWallet, right: JettonWallet): number {
  const leftAmount = parseJettonBalance(left.balance)
  const rightAmount = parseJettonBalance(right.balance)
  const leftDecimals = parseJettonDecimals(left.master)
  const rightDecimals = parseJettonDecimals(right.master)
  const leftScaled = leftAmount * 10n ** BigInt(rightDecimals)
  const rightScaled = rightAmount * 10n ** BigInt(leftDecimals)
  if (leftScaled === rightScaled) {
    const leftSymbol = left.master?.jetton_content.symbol ?? ""
    const rightSymbol = right.master?.jetton_content.symbol ?? ""
    return leftSymbol.localeCompare(rightSymbol)
  }
  return leftScaled > rightScaled ? -1 : 1
}

function parseJettonBalance(value: string): bigint {
  try {
    return BigInt(value)
  } catch {
    return 0n
  }
}

function parseJettonDecimals(master: JettonMasterMetadata | undefined): number {
  const decimals = Number(master?.jetton_content.decimals)
  return Number.isInteger(decimals) && decimals >= 0 && decimals <= 36 ? decimals : 9
}
