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
  abiPath,
  basePath = "/explorer",
  cellPath,
  children,
  contractsPath,
  emulatePath,
  localnetBasePath,
  sourcesPath,
}) => {
  const addressFormat = useAddressFormat()
  const routes = useMemo(
    () =>
      createExplorerRoutes(basePath, addressFormat, localnetBasePath, {
        abiPath,
        cellPath,
        contractsPath,
        emulatePath,
        sourcesPath,
      }),
    [
      abiPath,
      addressFormat,
      basePath,
      cellPath,
      contractsPath,
      emulatePath,
      localnetBasePath,
      sourcesPath,
    ],
  )

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
