import type {FC, ReactNode} from "react"
import {useMemo} from "react"

import {createExplorerRoutes, ExplorerRoutesContext} from "./explorerRoutesContext"

interface ExplorerRoutesProviderProps {
  readonly basePath?: string
  readonly children: ReactNode
}

export const ExplorerRoutesProvider: FC<ExplorerRoutesProviderProps> = ({
  basePath = "/explorer",
  children,
}) => {
  const routes = useMemo(() => createExplorerRoutes(basePath), [basePath])

  return <ExplorerRoutesContext.Provider value={routes}>{children}</ExplorerRoutesContext.Provider>
}
