import {useMemo} from "react"
import type {FC, ReactNode} from "react"

import {ExplorerRoutesContext, createExplorerRoutes} from "./explorerRoutesContext"
import {useAddressFormat} from "./useNetworkInfo"

interface ExplorerRoutesProviderProps {
  readonly basePath?: string
  readonly children: ReactNode
  readonly localnetBasePath?: string
}

export const ExplorerRoutesProvider: FC<ExplorerRoutesProviderProps> = ({
  basePath = "/explorer",
  children,
  localnetBasePath,
}) => {
  const addressFormat = useAddressFormat()
  const routes = useMemo(
    () => createExplorerRoutes(basePath, addressFormat, localnetBasePath),
    [addressFormat, basePath, localnetBasePath],
  )

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
