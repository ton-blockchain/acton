import {
  LocalStorageAdapter,
  Network,
  Signer,
  TonWalletKit,
  WalletV4R2Adapter,
  WalletV5R1Adapter,
  createDeviceInfo,
  createWalletManifest,
  type Wallet,
} from "@ton/walletkit"

import type {StartupWalletRecord} from "./types"

const TON_CONNECT_BRIDGE_URL = "https://bridge.tonapi.io/bridge"
export const ACTON_TON_CONNECT_URL = "https://ton-blockchain.github.io/acton"
const TONKEEPER_TON_CONNECT_URL = "https://tonkeeper.com"
const TONKEEPER_TON_CONNECT_ICON_URL = "https://tonkeeper.com/assets/tonconnect-icon.png"
export const ACTON_WALLET_APP_NAME = "Tonkeeper"
export const ACTON_WALLET_JS_BRIDGE_KEY = "tonkeeper"

function getWalletOrigin(): string {
  if (globalThis.location === undefined) {
    return "http://localhost:3006"
  }

  return globalThis.location.origin
}

function getApiEndpoint(host: string): string {
  if (host.length > 0) {
    return host
  }

  return getWalletOrigin()
}

export function getWalletNetwork(): Network {
  return Network.testnet()
}

export function getWalletNetworkLabel(): string {
  return "Localnet"
}

export function createWalletKit(host: string): TonWalletKit {
  const origin = getWalletOrigin()
  const walletUrl = `${origin}/wallets`
  const apiEndpoint = getApiEndpoint(host)

  return new TonWalletKit({
    deviceInfo: createDeviceInfo({
      appName: ACTON_WALLET_APP_NAME,
      appVersion: "0.1.0",
      features: [
        "SendTransaction",
        {name: "SendTransaction", maxMessages: 4},
        {name: "SignData", types: ["text", "binary", "cell"]},
      ],
    }),
    walletManifest: createWalletManifest({
      name: ACTON_WALLET_APP_NAME,
      appName: ACTON_WALLET_APP_NAME,
      imageUrl: TONKEEPER_TON_CONNECT_ICON_URL,
      aboutUrl: TONKEEPER_TON_CONNECT_URL,
      universalLink: walletUrl,
      bridgeUrl: TON_CONNECT_BRIDGE_URL,
      jsBridgeKey: ACTON_WALLET_JS_BRIDGE_KEY,
      injected: false,
      embedded: false,
      platforms: ["chrome", "firefox", "safari", "android", "ios", "windows", "macos", "linux"],
    }),
    networks: {
      [Network.mainnet().chainId]: {
        apiClient: {
          url: apiEndpoint,
        },
      },
      [Network.testnet().chainId]: {
        apiClient: {
          url: apiEndpoint,
        },
      },
    },
    storage: new LocalStorageAdapter({prefix: "acton-localnet-walletkit:"}),
    dev: {
      disableManifestDomainCheck: true,
    },
  })
}

export async function addStartupWalletToKit(
  kit: TonWalletKit,
  walletRecord: StartupWalletRecord,
): Promise<Wallet | undefined> {
  const signer = await Signer.fromMnemonic([...walletRecord.mnemonic])
  const network = getWalletNetwork()
  const client = kit.getApiClient(network)
  const options = {
    client,
    network,
    walletId: walletRecord.wallet_id,
  }

  const adapter =
    walletRecord.version === "v4r2"
      ? await WalletV4R2Adapter.create(signer, options)
      : await WalletV5R1Adapter.create(signer, options)

  return kit.addWallet(adapter)
}
