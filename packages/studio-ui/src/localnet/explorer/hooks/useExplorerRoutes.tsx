import {useMemo} from "react"
import type {FC, ReactNode} from "react"

import {
  ExplorerRoutesContext,
  createExplorerRoutes,
  type ExplorerRouteOverrides,
} from "./explorerRoutesContext"
import {useAddressFormat} from "./useNetworkInfo"

interface ExplorerRoutesProviderProps extends ExplorerRouteOverrides {
  readonly basePath?: string
  readonly children: ReactNode
  readonly localnetBasePath?: string
}

export const ExplorerRoutesProvider: FC<ExplorerRoutesProviderProps> = ({
  basePath = "/explorer",
  cellPath,
  children,
  emulatePath,
  localnetBasePath,
}) => {
  const addressFormat = useAddressFormat()
  const routes = useMemo(
    () =>
      createExplorerRoutes(basePath, addressFormat, localnetBasePath, {
        cellPath,
        emulatePath,
      }),
    [addressFormat, basePath, cellPath, emulatePath, localnetBasePath],
  )

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
