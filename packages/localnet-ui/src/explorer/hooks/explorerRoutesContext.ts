import {createContext} from "react"

import {normalizeAddress, type AddressFormatOptions} from "../components/utils"

export interface ExplorerRoutes {
  readonly rootPath: string
  readonly blocksPath: string
  readonly abiPath: string
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

export const createExplorerRoutes = (
  basePath: string,
  addressFormat: AddressFormatOptions = {testOnly: false},
): ExplorerRoutes => {
  const base = basePath.replace(/\/$/, "")
  const path = (suffix = "") => `${base}${suffix}` || "/"

  return {
    rootPath: path(),
    blocksPath: path("/blocks"),
    abiPath: path("/abi"),
    cellPath: path("/cell"),
    emulatePath: path("/emulate"),
    sourcesPath: path("/sources"),
    favoritesPath: path("/favorites"),
    abiDetailsPath: slug => path(`/abi/${encodeURIComponent(slug)}`),
    addressPath: address =>
      path(`/address/${encodeURIComponent(normalizeAddress(address, addressFormat))}`),
    blockPath: (workchain, shard, seqno) =>
      `/block/${workchain}/${encodeURIComponent(shard)}/${seqno}`,
    transactionPath: hash => path(`/tx/${encodeURIComponent(hash)}`),
    transactionTracePath: hash => path(`/tx/${encodeURIComponent(hash)}/trace`),
  }
}

export const ExplorerRoutesContext = createContext<ExplorerRoutes>(
  createExplorerRoutes("/explorer"),
)
