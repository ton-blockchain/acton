import type {TonClient} from "./client"
import type {JettonMasterMetadata, JettonWallet} from "./types"

import {toRawAddress} from "../components/utils"

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

export function sortJettonWalletsByAmount(wallets: readonly JettonWallet[]): JettonWallet[] {
  return [...wallets].sort(compareJettonWalletAmount)
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
