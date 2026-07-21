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
  const leftAmount = normalizeJettonAmount(left)
  const rightAmount = normalizeJettonAmount(right)
  if (leftAmount === rightAmount) {
    return 0
  }
  return leftAmount > rightAmount ? -1 : 1
}

function normalizeJettonAmount(wallet: JettonWallet): number {
  const decimals = parseJettonDecimals(wallet.master)
  const amount = Number(wallet.balance) / 10 ** decimals
  return Number.isFinite(amount) ? amount : 0
}

function parseJettonDecimals(master: JettonMasterMetadata | undefined): number {
  const decimals = Number(master?.jetton_content.decimals)
  return Number.isFinite(decimals) ? decimals : 9
}
