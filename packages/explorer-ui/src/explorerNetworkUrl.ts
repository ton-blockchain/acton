export const EXPLORER_NETWORK_QUERY_PARAM = "network"

export type ExplorerNetworkUrlId = "mainnet" | "testnet" | `custom:${string}`

export function explorerNetworkSearch(search: string, networkId: ExplorerNetworkUrlId): string {
  const searchParams = new URLSearchParams(search)
  if (networkId === "mainnet" || networkId === "testnet") {
    searchParams.set(EXPLORER_NETWORK_QUERY_PARAM, networkId)
  } else {
    searchParams.delete(EXPLORER_NETWORK_QUERY_PARAM)
  }
  return searchParams.toString()
}
