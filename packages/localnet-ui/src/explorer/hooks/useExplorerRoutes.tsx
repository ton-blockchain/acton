import {useMemo} from "react"
import type {FC, ReactNode} from "react"

import {ExplorerRoutesContext, createExplorerRoutes} from "./explorerRoutesContext"
import {useAddressFormat} from "./useNetworkInfo"

interface ExplorerRoutesProviderProps {
  readonly basePath?: string
  readonly children: ReactNode
}

export const ExplorerRoutesProvider: FC<ExplorerRoutesProviderProps> = ({
  basePath = "/explorer",
  children,
}) => {
  const addressFormat = useAddressFormat()
  const routes = useMemo(
    () => createExplorerRoutes(basePath, addressFormat),
    [addressFormat, basePath],
  )

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
