import type {RetraceNetworkConfig} from "@ton/retracer-core"
import {RETRACE_MAINNET_NETWORK, RETRACE_TESTNET_NETWORK} from "@ton/retracer-core"

import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"
import {TxTraceError} from "./errors"

function absoluteApiBaseUrl(baseUrl: string): string {
  const fullBase = baseUrl.startsWith("http") ? baseUrl : `${globalThis.location.origin}${baseUrl}`
  return new URL(fullBase).toString().replace(/\/$/, "")
}

export function getRetraceNetworkConfig(network: ExplorerNetworkInfo): RetraceNetworkConfig {
  if (network.api) {
    return {
      testnet: network.testOnly,
      v2BaseUrl: absoluteApiBaseUrl(network.api.v2BaseUrl),
      v3BaseUrl: absoluteApiBaseUrl(network.api.v3BaseUrl),
      toncenterApiKey: network.api.toncenterApiKey,
    }
  }

  if (network.id === "mainnet") {
    return RETRACE_MAINNET_NETWORK
  }
  if (network.id === "testnet") {
    return RETRACE_TESTNET_NETWORK
  }

  throw new TxTraceError(`Retrace is not configured for ${network.label}.`)
}
