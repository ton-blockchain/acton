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
  addressConverterPath,
  basePath = "/explorer",
  cellPath,
  children,
  contractsPath,
  electionsPath,
  emulatePath,
  localnetBasePath,
  sourcesPath,
}) => {
  const addressFormat = useAddressFormat()
  const routes = useMemo(
    () =>
      createExplorerRoutes(basePath, addressFormat, localnetBasePath, {
        abiPath,
        addressConverterPath,
        cellPath,
        contractsPath,
        electionsPath,
        emulatePath,
        sourcesPath,
      }),
    [
      abiPath,
      addressConverterPath,
      addressFormat,
      basePath,
      cellPath,
      contractsPath,
      electionsPath,
      emulatePath,
      localnetBasePath,
      sourcesPath,
    ],
  )

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
