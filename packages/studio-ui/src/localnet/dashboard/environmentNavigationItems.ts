import {
  Activity,
  Archive,
  Binary,
  Cable,
  HandCoins,
  LayoutGrid,
  Settings2,
  Wallet,
  Waypoints,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"

import {supports, supportsAny} from "../../environmentCapabilities"
import type {StudioEnvironment} from "../../studioApi"

/** A navigable environment page shared by the sidebar and Studio search */
export interface SidebarItem {
  readonly label: string
  readonly icon: LucideIcon
  readonly path: string
}

/** A page whose icon and context are supplied by its navigation group */
export interface NestedSidebarItem {
  readonly label: string
  readonly path: string
}

const primaryItems: SidebarItem[] = [{label: "Home", icon: LayoutGrid, path: "/dashboard"}]

const explorerItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/explorer"},
  {label: "Blocks", path: "/explorer/blocks"},
  {label: "Config", path: "/explorer/config"},
  {label: "Elections", path: "/explorer/elections"},
  {label: "Tokens", path: "/explorer/tokens"},
  {label: "NFTs", path: "/explorer/nfts"},
]

const networkItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/network"},
  {label: "Nodes", path: "/network/nodes"},
  {label: "Validators", path: "/network/validators"},
  {label: "Stats", path: "/network/stats"},
  {label: "Config", path: "/network/config"},
  {label: "Activity", path: "/network/activity"},
  {label: "Health", path: "/network/health"},
]

const contractItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/contracts"},
  {label: "Sources", path: "/contracts/sources"},
  {label: "ABI", path: "/contracts/abi"},
]

const standaloneItems: SidebarItem[] = [
  {label: "Simulator", icon: Waypoints, path: "/simulator"},
  {label: "Cell Inspector", icon: Binary, path: "/cell-inspector"},
]

const environmentItems: SidebarItem[] = [
  {label: "Wallets", icon: Wallet, path: "/wallets"},
  {label: "Faucet", icon: HandCoins, path: "/faucet"},
  {label: "Snapshots", icon: Archive, path: "/snapshots"},
  {label: "Admin actions", icon: Settings2, path: "/admin"},
]

const apiCallsItem: SidebarItem = {
  label: "API Calls",
  icon: Activity,
  path: "/api-calls",
}

const integrateItem: SidebarItem = {
  label: "Integrate",
  icon: Cable,
  path: "/integrate",
}

const apiReferenceItems: NestedSidebarItem[] = [
  {label: "v2 API", path: "/api-reference/v2"},
  {label: "v3 API", path: "/api-reference/v3"},
  {label: "Admin API", path: "/api-reference/admin"},
  {label: "Config API", path: "/api-reference/config"},
  {label: "Control API", path: "/api-reference/control"},
]

/** Keeps search and the sidebar on the same capability-filtered set of routes */
export function getEnvironmentNavigation(environment: StudioEnvironment | undefined) {
  const visibleStandaloneItems = supports(environment, "simulator") ? standaloneItems : []
  const visibleExplorerItems = explorerItems.filter(
    item => item.path !== "/explorer/elections" || environment?.lifecycle === "external",
  )
  const visibleNetworkItems = networkItems.filter(item =>
    item.path === "/network/config"
      ? supports(environment, "controlApi")
      : environment?.config.kind === "fullTonNetwork" &&
        (item.path !== "/network/health" || supports(environment, "health")),
  )
  const visibleEnvironmentItems = environmentItems.filter(item =>
    item.path === "/admin"
      ? environment?.config.kind === "fullTonNetwork" && environment.lifecycle === "managed"
      : item.path === "/wallets"
        ? supports(environment, "wallets")
        : item.path === "/snapshots"
          ? supports(environment, "snapshots")
          : supportsAny(environment, "testnetFaucet", "gramFaucet", "jettonFaucet"),
  )
  const visibleApiReferenceItems = apiReferenceItems.filter(item => {
    if (item.path === "/api-reference/v2") return supports(environment, "apiV2")
    if (item.path === "/api-reference/v3") return supports(environment, "apiV3")
    if (item.path === "/api-reference/config") return supports(environment, "configApi")
    if (item.path === "/api-reference/admin") {
      return environment?.config.kind === "fullTonNetwork" && supports(environment, "controlApi")
    }
    return environment?.config.kind !== "fullTonNetwork" && supports(environment, "controlApi")
  })

  return {
    primaryItems,
    visibleStandaloneItems,
    visibleExplorerItems: supports(environment, "explorer") ? visibleExplorerItems : [],
    visibleNetworkItems,
    visibleEnvironmentItems,
    visibleApiReferenceItems,
    contractItems: supports(environment, "contracts") ? contractItems : [],
    integrateItem: supports(environment, "integration") ? integrateItem : undefined,
    apiCallsItem: supports(environment, "apiCalls") ? apiCallsItem : undefined,
  }
}
