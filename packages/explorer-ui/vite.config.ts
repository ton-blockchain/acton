import path from "node:path"
import {createRequire} from "node:module"

import react from "@vitejs/plugin-react"
import {defineConfig, loadEnv, type ProxyOptions} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))

const toncenterDevProxy = (
  prefix: string,
  endpoint: "blocks" | "getBlockTransactions" | "getShards" | "traces" | "transactions",
  apiUrl: string,
  apiKey?: string,
): ProxyOptions => {
  const upstream = new URL(apiUrl)
  const upstreamPath = `${upstream.pathname.replace(/\/$/, "")}/${endpoint}`
  return {
    target: upstream.origin,
    changeOrigin: true,
    rewrite: requestPath => requestPath.replace(prefix, upstreamPath),
    headers: apiKey ? {"X-API-Key": apiKey} : undefined,
  }
}

const toncenterNetworkDevProxies = (
  network: "mainnet" | "testnet",
  apiV2Url: string,
  apiV3Url: string,
  apiKey?: string,
): Record<string, ProxyOptions> => {
  const endpoints = [
    ["v2", "getBlockTransactions", apiV2Url],
    ["v2", "getShards", apiV2Url],
    ["v3", "blocks", apiV3Url],
    ["v3", "traces", apiV3Url],
    ["v3", "transactions", apiV3Url],
  ] as const
  return Object.fromEntries(
    endpoints.map(([version, endpoint, apiUrl]) => {
      const prefix = `/api/toncenter/${network}/${version}/${endpoint}`
      return [prefix, toncenterDevProxy(prefix, endpoint, apiUrl, apiKey)] as const
    }),
  )
}

export default defineConfig(({mode}) => {
  const loadedEnv = loadEnv(mode, import.meta.dirname, "")
  const envValue = (...names: string[]): string | undefined =>
    names
      .map(name => loadedEnv[name])
      .find(value => value?.trim())
      ?.trim()

  const mainnetApiV2Url =
    envValue(
      "TONCENTER_MAINNET_API_V2_URL",
      "TONCENTER_API_V2_URL",
      "VITE_EXPLORER_MAINNET_TONCENTER_API_V2_URL",
      "VITE_EXPLORER_TONCENTER_API_V2_URL",
    ) ?? "https://toncenter.com/api/v2"
  const mainnetApiV3Url =
    envValue(
      "TONCENTER_MAINNET_API_V3_URL",
      "TONCENTER_API_V3_URL",
      "VITE_EXPLORER_MAINNET_TONCENTER_API_V3_URL",
      "VITE_EXPLORER_TONCENTER_API_V3_URL",
    ) ?? "https://toncenter.com/api/v3"
  const testnetApiV2Url =
    envValue("TONCENTER_TESTNET_API_V2_URL", "VITE_EXPLORER_TESTNET_TONCENTER_API_V2_URL") ??
    "https://testnet.toncenter.com/api/v2"
  const testnetApiV3Url =
    envValue("TONCENTER_TESTNET_API_V3_URL", "VITE_EXPLORER_TESTNET_TONCENTER_API_V3_URL") ??
    "https://testnet.toncenter.com/api/v3"
  const mainnetApiKey = envValue(
    "TONCENTER_MAINNET_API_KEY",
    "TONCENTER_API_KEY",
    "VITE_EXPLORER_MAINNET_TONCENTER_API_KEY",
    "VITE_EXPLORER_TONCENTER_API_KEY",
  )
  const testnetApiKey = envValue(
    "TONCENTER_TESTNET_API_KEY",
    "VITE_EXPLORER_TESTNET_TONCENTER_API_KEY",
  )

  return {
    plugins: [
      themeBootstrap({storageKey: "explorerTheme"}),
      react(),
      nodePolyfills({
        include: ["buffer", "path"],
        globals: {
          Buffer: true,
        },
      }),
    ],
    resolve: {
      dedupe: ["@acton/ui", "react", "react-dom"],
      alias: {
        "@acton/transaction-ui": path.resolve(import.meta.dirname, "../transaction-ui/src"),
        "@tasm-spec": path.resolve(import.meta.dirname, "../../crates/tasm-core/spec"),
        "vite-plugin-node-polyfills/shims/buffer": path.resolve(
          nodePolyfillsRoot,
          "shims/buffer/index.ts",
        ),
      },
    },
    build: {
      outDir: "dist",
      emptyOutDir: true,
    },
    server: {
      port: 3007,
      proxy: {
        ...toncenterNetworkDevProxies("mainnet", mainnetApiV2Url, mainnetApiV3Url, mainnetApiKey),
        ...toncenterNetworkDevProxies("testnet", testnetApiV2Url, testnetApiV3Url, testnetApiKey),
      },
    },
  }
})
