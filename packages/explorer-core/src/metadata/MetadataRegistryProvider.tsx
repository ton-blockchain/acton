import {createContext, useContext} from "react"
import type {FC, ReactNode} from "react"

import {NullMetadataRegistry} from "./nullRegistry"
import type {ExplorerMetadataRegistry} from "./types"

const fallbackRegistry = new NullMetadataRegistry()

const MetadataRegistryContext = createContext<ExplorerMetadataRegistry>(fallbackRegistry)

export const MetadataRegistryProvider: FC<{
  readonly registry: ExplorerMetadataRegistry
  readonly children: ReactNode
}> = ({registry, children}) => {
  return (
    <MetadataRegistryContext.Provider value={registry}>{children}</MetadataRegistryContext.Provider>
  )
}

export function useMetadataRegistry(): ExplorerMetadataRegistry {
  return useContext(MetadataRegistryContext)
}
