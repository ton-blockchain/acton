import {
  ApiClientToncenter,
  LocalStorageAdapter,
  Network,
  Signer,
  TonWalletKit,
  createDeviceInfo,
  createWalletManifest,
  type Wallet,
} from "@ton/walletkit"

import {createLocalnetWalletV4R2Adapter, createLocalnetWalletV5R1Adapter} from "./localnetAdapters"
import type {StartupWalletRecord} from "./types"

const TON_CONNECT_BRIDGE_URL =
  import.meta.env.VITE_TON_CONNECT_BRIDGE_URL?.trim() || "https://bridge.tonapi.io/bridge"
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

function getApiEndpoint(apiEndpoint: string): string {
  if (apiEndpoint.length > 0) {
    return apiEndpoint
  }

  return getWalletOrigin()
}

function createLocalnetFetch(apiEndpoint: string, localnetApiToken?: string): typeof fetch {
  const endpointUrl = new URL(apiEndpoint)
  const endpointPath = endpointUrl.pathname.replace(/\/$/, "")
  const token = localnetApiToken?.trim()

  const fetchThroughLocalnetEndpoint: typeof fetch = (input, init) => {
    // WalletKit resolves Toncenter paths from the origin root. Reattach those paths to
    // the environment endpoint so direct localnets and Studio proxies share one transport.
    const requestUrl = new URL(input instanceof Request ? input.url : input.toString())
    const targetUrl = new URL(endpointUrl)
    targetUrl.pathname = `${endpointPath}${requestUrl.pathname}`
    targetUrl.search = requestUrl.search
    targetUrl.hash = requestUrl.hash

    const headers = new Headers(init?.headers)
    if (token) {
      headers.set("Authorization", `Bearer ${token}`)
    }

    return fetch(targetUrl, {...init, headers})
  }
  return fetchThroughLocalnetEndpoint
}

function createLocalnetApiClient(
  endpoint: string,
  network: Network,
  localnetApiToken?: string,
): ApiClientToncenter {
  return new ApiClientToncenter({
    endpoint,
    network,
    fetchApi: createLocalnetFetch(endpoint, localnetApiToken),
  })
}

export function getWalletNetwork(): Network {
  return Network.testnet()
}

export function getWalletNetworkLabel(): string {
  return "Localnet"
}

export function createWalletKit(apiBaseUrl: string, localnetApiToken?: string): TonWalletKit {
  const origin = getWalletOrigin()
  const walletUrl = origin
  const apiEndpoint = getApiEndpoint(apiBaseUrl)
  const mainnet = Network.mainnet()
  const testnet = Network.testnet()

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
      [mainnet.chainId]: {
        apiClient: createLocalnetApiClient(apiEndpoint, mainnet, localnetApiToken),
      },
      [testnet.chainId]: {
        apiClient: createLocalnetApiClient(apiEndpoint, testnet, localnetApiToken),
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
      ? await createLocalnetWalletV4R2Adapter(signer, options)
      : await createLocalnetWalletV5R1Adapter(signer, options)

  return kit.addWallet(adapter)
}
