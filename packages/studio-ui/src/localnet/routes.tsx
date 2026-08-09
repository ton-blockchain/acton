import {createContext, useContext, useMemo} from "react"
import type {FC, ReactNode} from "react"

interface LocalnetRoutes {
  readonly basePath: string
  readonly path: (path: string) => string
}

const LocalnetRoutesContext = createContext<LocalnetRoutes | undefined>(undefined)

interface LocalnetRoutesProviderProps {
  readonly basePath?: string
  readonly children: ReactNode
}

export const LocalnetRoutesProvider: FC<LocalnetRoutesProviderProps> = ({
  basePath = "",
  children,
}) => {
  const routes = useMemo(
    () => ({
      basePath,
      path: (path: string) => localnetPath(basePath, path),
    }),
    [basePath],
  )

  return <LocalnetRoutesContext.Provider value={routes}>{children}</LocalnetRoutesContext.Provider>
}

export function useLocalnetRoutes() {
  const routes = useContext(LocalnetRoutesContext)
  if (!routes) throw new Error("Localnet routes are not available")
  return routes
}

export function localnetPath(basePath: string, path: string) {
  const normalizedBasePath = basePath === "/" ? "" : basePath.replace(/\/+$/, "")
  const normalizedPath = path.startsWith("/") ? path : `/${path}`
  return `${normalizedBasePath}${normalizedPath}` || "/"
}

export function localnetContractPath(
  basePath: string,
  address: string,
  section?: "abi" | "raw-abi",
): string {
  const root = localnetPath(basePath, `/contracts/${encodeURIComponent(address)}`)
  return section ? `${root}/${encodeURIComponent(section)}` : root
}
