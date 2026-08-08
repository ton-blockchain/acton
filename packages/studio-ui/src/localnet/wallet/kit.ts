import {
  ApiClientToncenter,
  LocalStorageAdapter,
  Network,
  TonWalletKit,
  Uint8ArrayToHex,
  WalletV4R2Adapter,
  WalletV5R1Adapter,
  createDeviceInfo,
  createWalletManifest,
  type Hex,
  type Wallet,
  type WalletSigner,
} from "@ton/walletkit"

import {signWithStudioWallet} from "../../studioApi"

import {createLocalnetWalletV4R2Adapter, createLocalnetWalletV5R1Adapter} from "./localnetAdapters"
import type {ProjectWalletRecord} from "./types"

const TON_CONNECT_BRIDGE_URL =
  import.meta.env.VITE_TON_CONNECT_BRIDGE_URL?.trim() || "https://bridge.tonapi.io/bridge"
const TONKEEPER_WALLET_NAME = "Tonkeeper"
const TONKEEPER_WALLET_APP_NAME = "tonkeeper"
const TONKEEPER_WALLET_JS_BRIDGE_KEY = "tonkeeper"

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

function resolveNetwork(chainId: number): Network {
  if (chainId === Number(Network.mainnet().chainId)) {
    return Network.mainnet()
  }
  if (chainId === Number(Network.testnet().chainId)) {
    return Network.testnet()
  }

  return Network.custom(String(chainId))
}

export function createWalletKit(
  apiBaseUrl: string,
  environmentId: string,
  chainId: number,
  localnetApiToken?: string,
): TonWalletKit {
  const origin = getWalletOrigin()
  const walletUrl = origin
  const walletIconUrl = new URL("/favicon.svg", origin).toString()
  const apiEndpoint = getApiEndpoint(apiBaseUrl)
  const network = resolveNetwork(chainId)

  return new TonWalletKit({
    deviceInfo: createDeviceInfo({
      appName: TONKEEPER_WALLET_APP_NAME,
      appVersion: "0.1.0",
      features: [
        "SendTransaction",
        {name: "SendTransaction", maxMessages: 4},
        {name: "SignData", types: ["text", "binary", "cell"]},
      ],
    }),
    walletManifest: createWalletManifest({
      name: TONKEEPER_WALLET_NAME,
      appName: TONKEEPER_WALLET_APP_NAME,
      imageUrl: walletIconUrl,
      aboutUrl: origin,
      universalLink: walletUrl,
      bridgeUrl: TON_CONNECT_BRIDGE_URL,
      jsBridgeKey: TONKEEPER_WALLET_JS_BRIDGE_KEY,
      injected: false,
      embedded: false,
      platforms: ["chrome", "firefox", "safari", "android", "ios", "windows", "macos", "linux"],
    }),
    networks: {
      [network.chainId]: {
        apiClient: createLocalnetApiClient(apiEndpoint, network, localnetApiToken),
      },
    },
    storage: new LocalStorageAdapter({
      prefix: `acton-studio:${environmentId}:walletkit:`,
    }),
    dev: {
      disableManifestDomainCheck: true,
    },
  })
}

export async function addProjectWalletToKit(
  kit: TonWalletKit,
  walletRecord: ProjectWalletRecord,
  options: {
    readonly environmentId: string
    readonly chainId: number
    readonly useLocalnetAdapters: boolean
  },
): Promise<Wallet | undefined> {
  const signer = createStudioWalletSigner(options.environmentId, walletRecord)
  const network = resolveNetwork(options.chainId)
  const client = kit.getApiClient(network)
  const adapterOptions = {
    client,
    network,
    walletId: walletRecord.walletId,
    workchain: walletRecord.workchain,
  }

  if (walletRecord.version === "v4r2") {
    const adapter = options.useLocalnetAdapters
      ? await createLocalnetWalletV4R2Adapter(signer, adapterOptions)
      : await WalletV4R2Adapter.create(signer, adapterOptions)
    return kit.addWallet(adapter)
  }

  const adapter = options.useLocalnetAdapters
    ? await createLocalnetWalletV5R1Adapter(signer, adapterOptions)
    : await WalletV5R1Adapter.create(signer, adapterOptions)
  return kit.addWallet(adapter)
}

function createStudioWalletSigner(
  environmentId: string,
  wallet: ProjectWalletRecord,
): WalletSigner {
  return {
    publicKey: wallet.publicKey as Hex,
    sign: async bytes => {
      const response = await signWithStudioWallet(
        environmentId,
        wallet.name,
        Uint8ArrayToHex(bytes),
      )
      return response.signature as Hex
    },
  }
}
