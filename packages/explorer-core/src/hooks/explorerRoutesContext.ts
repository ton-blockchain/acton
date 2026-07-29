import {createContext} from "react"

import {normalizeAddress, type AddressFormatOptions} from "../components/utils"

export interface ExplorerRoutes {
  readonly rootPath: string
  readonly blocksPath: string
  readonly abiPath: string
  readonly contractsPath?: string
  readonly cellPath: string
  readonly emulatePath: string
  readonly sourcesPath: string
  readonly favoritesPath: string
  readonly abiDetailsPath: (slug: string) => string
  readonly addressPath: (address: string) => string
  readonly blockPath: (workchain: number, shard: string, seqno: number) => string
  readonly transactionPath: (hash: string) => string
  readonly transactionTracePath: (hash: string) => string
}

export interface ExplorerRouteOverrides {
  readonly abiPath?: string
  readonly cellPath?: string
  readonly contractsPath?: string
  readonly emulatePath?: string
  readonly sourcesPath?: string
}

export const createExplorerRoutes = (
  basePath: string,
  addressFormat: AddressFormatOptions = {testOnly: false},
  localnetBasePath = "",
  overrides: ExplorerRouteOverrides = {},
): ExplorerRoutes => {
  const base = basePath.replace(/\/$/, "")
  const localnetBase = localnetBasePath.replace(/\/$/, "")
  const path = (suffix = "") => `${base}${suffix}` || "/"
  const localnetPath = (suffix: string) => `${localnetBase}${suffix}` || "/"
  const abiPath = overrides.abiPath ?? path("/abi")

  return {
    rootPath: path(),
    blocksPath: path("/blocks"),
    abiPath,
    contractsPath: overrides.contractsPath,
    cellPath: overrides.cellPath ?? path("/cell"),
    emulatePath: overrides.emulatePath ?? path("/emulate"),
    sourcesPath: overrides.sourcesPath ?? path("/sources"),
    favoritesPath: path("/favorites"),
    abiDetailsPath: slug => `${abiPath}/${encodeURIComponent(slug)}`,
    addressPath: address =>
      path(`/address/${encodeURIComponent(normalizeAddress(address, addressFormat))}`),
    blockPath: (workchain, shard, seqno) =>
      localnetPath(`/block/${workchain}/${encodeURIComponent(shard)}/${seqno}`),
    transactionPath: hash => path(`/tx/${encodeURIComponent(hash)}`),
    transactionTracePath: hash => path(`/tx/${encodeURIComponent(hash)}/trace`),
  }
}

export const ExplorerRoutesContext = createContext<ExplorerRoutes>(
  createExplorerRoutes("/explorer"),
)
