import {createContext} from "react"

export interface ExplorerRoutes {
  readonly rootPath: string
  readonly blocksPath: string
  readonly abiPath: string
  readonly sourcesPath: string
  readonly favoritesPath: string
  readonly abiDetailsPath: (slug: string) => string
  readonly addressPath: (address: string) => string
  readonly transactionPath: (hash: string) => string
  readonly transactionTracePath: (hash: string) => string
}

export const createExplorerRoutes = (basePath: string): ExplorerRoutes => {
  const base = basePath.replace(/\/$/, "")
  const path = (suffix = "") => `${base}${suffix}` || "/"

  return {
    rootPath: path(),
    blocksPath: path("/blocks"),
    abiPath: path("/abi"),
    sourcesPath: path("/sources"),
    favoritesPath: path("/favorites"),
    abiDetailsPath: slug => path(`/abi/${encodeURIComponent(slug)}`),
    addressPath: address => path(`/address/${encodeURIComponent(address)}`),
    transactionPath: hash => path(`/tx/${encodeURIComponent(hash)}`),
    transactionTracePath: hash => path(`/tx/${encodeURIComponent(hash)}/trace`),
  }
}

export const ExplorerRoutesContext = createContext<ExplorerRoutes>(
  createExplorerRoutes("/explorer"),
)
