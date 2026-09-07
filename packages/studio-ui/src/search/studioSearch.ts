import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {formatAddress, hashToHex, parseAddress} from "@acton/explorer-core/components/utils"
import {createExplorerRoutes} from "@acton/explorer-core/hooks/useExplorerRoutes"

import {supports} from "../environmentCapabilities"
import {getContractIdentity} from "../localnet/dashboard/contracts/contractPresentation"
import {getEnvironmentNavigation} from "../localnet/dashboard/environmentNavigationItems"
import {localnetContractPath, localnetPath} from "../localnet/routes"
import type {StudioEnvironment, StudioWallet} from "../studioApi"
import {studioPages} from "../studioPages"
import {studioEnvironmentPath} from "../studioRoutes"

type SearchKind = "page" | "environment" | "contract" | "wallet" | "address" | "transaction"

/** Search history stores public destinations, never wallet records or runtime credentials */
export interface SearchDestination {
  readonly kind: SearchKind
  readonly title: string
  readonly value: string
  readonly environmentId?: string
}

/** A resolved destination, with identity and context suitable for a shared SearchInput row */
export interface StudioSearchResult extends SearchDestination {
  readonly id: string
  readonly href: string
  readonly description: string
  readonly group: string
  readonly keywords: string
}

const HISTORY_KEY = "actonStudioSearchHistory"
const HISTORY_LIMIT = 8

/** Resolve history against today's environments and route capabilities before offering navigation */
export function resolveSearchDestination(
  destination: SearchDestination,
  environments: readonly StudioEnvironment[],
): StudioSearchResult | undefined {
  const environment = environments.find(item => item.id === destination.environmentId)
  if (destination.environmentId && !environment) return undefined

  const basePath = environment ? studioEnvironmentPath(environment) : ""
  const addressFormat = {testOnly: environment?.network.testOnly ?? false}
  const routes = createExplorerRoutes(localnetPath(basePath, "/explorer"), addressFormat, basePath)
  let href: string
  let detail: string
  let title = destination.title

  switch (destination.kind) {
    case "environment":
      if (!environment) return undefined
      href = basePath
      title = environment.name
      detail = `${environment.lifecycle === "external" ? "Network" : "Environment"} · ${environment.status}`
      break
    case "page":
      href = localnetPath(basePath, destination.value)
      detail = "Page"
      break
    case "transaction": {
      const hash = hashToHex(destination.value)
      if (!environment || !supports(environment, "explorer") || !hash) return undefined
      href = routes.transactionPath(hash)
      detail = `Transaction · ${hash.slice(0, 8)}…${hash.slice(-8)}`
      break
    }
    default: {
      const address = parseAddress(destination.value)
      const capability =
        destination.kind === "contract"
          ? "contracts"
          : destination.kind === "wallet"
            ? "wallets"
            : "explorer"
      if (!environment || !supports(environment, capability) || !address) return undefined
      const friendly = address.toString(addressFormat)
      href =
        destination.kind === "contract"
          ? localnetContractPath(basePath, friendly)
          : routes.addressPath(friendly)
      const kind =
        destination.kind === "contract"
          ? "Contract"
          : destination.kind === "wallet"
            ? "Wallet"
            : "Account"
      detail = `${kind} · ${formatAddress(friendly, true, addressFormat)}`
    }
  }

  return {
    ...destination,
    title,
    id: href,
    href,
    description:
      environment && destination.kind !== "environment"
        ? `${detail} · ${environment.name}`
        : detail,
    group:
      destination.kind === "page"
        ? "Pages"
        : destination.kind === "environment"
          ? "Environments"
          : "Objects",
    keywords: `${destination.title} ${destination.value} ${environment?.name ?? ""}`,
  }
}

/** Builds a small local index from Studio's existing registries, without scanning chain assets */
export function buildStudioSearchIndex(
  environments: readonly StudioEnvironment[],
  environment: StudioEnvironment | undefined,
  contracts: readonly LocalnetContract[],
  wallets: readonly StudioWallet[],
): StudioSearchResult[] {
  const destinations: SearchDestination[] = studioPages.map(page => ({
    kind: "page",
    title: page.label,
    value: page.path,
  }))
  destinations.push(
    ...environments.map(item => ({
      kind: "environment" as const,
      title: item.name,
      value: "",
      environmentId: item.id,
    })),
  )

  for (const target of environment ? [environment] : environments) {
    const navigation = getEnvironmentNavigation(target)
    const pages = [
      ...navigation.primaryItems.map(item => ({...item, label: "Environment home"})),
      ...navigation.visibleExplorerItems.map(item => ({
        ...item,
        label: item.label === "Overview" ? "Explorer" : `Explorer ${item.label}`,
      })),
      ...navigation.visibleNetworkItems.map(item => ({
        ...item,
        label: item.label === "Overview" ? "Network overview" : `Network ${item.label}`,
      })),
      ...navigation.contractItems.map(item => ({
        ...item,
        label: item.label === "Overview" ? "Contracts" : `Contract ${item.label}`,
      })),
      ...navigation.visibleStandaloneItems,
      ...navigation.visibleEnvironmentItems,
      ...navigation.visibleApiReferenceItems,
      ...(navigation.apiCallsItem ? [navigation.apiCallsItem] : []),
      ...(navigation.integrateItem ? [navigation.integrateItem] : []),
    ]
    destinations.push(
      ...pages.map(page => ({
        kind: "page" as const,
        title: page.label,
        value: page.path,
        environmentId: target.id,
      })),
    )

    if (environment && target.id === environment.id && environment.status === "running") {
      destinations.push(
        ...contracts.map(contract => ({
          kind: "contract" as const,
          title: getContractIdentity(contract).title,
          value: parseAddress(contract.address)?.toRawString() ?? contract.address,
          environmentId: environment.id,
        })),
      )
      destinations.push(
        ...wallets.map(wallet => ({
          kind: "wallet" as const,
          title: wallet.name,
          value: parseAddress(wallet.address)?.toRawString() ?? wallet.address,
          environmentId: environment.id,
        })),
      )
    }
  }

  return destinations.flatMap(destination => {
    const result = resolveSearchDestination(destination, environments)
    return result ? [result] : []
  })
}

/** Ranks exact identities ahead of partial matches and preserves room for each result category */
export function searchStudio(
  query: string,
  index: readonly StudioSearchResult[],
  environments: readonly StudioEnvironment[],
  environment: StudioEnvironment | undefined,
): StudioSearchResult[] {
  const trimmed = query.trim()
  const terms = trimmed.toLocaleLowerCase().split(/\s+/)
  const address = parseAddress(trimmed)
  const transaction = address ? undefined : hashToHex(trimmed)
  const rawAddress = address?.toRawString()
  const identifier = rawAddress ?? transaction
  const ranked = index.flatMap(result => {
    const title = result.title.toLocaleLowerCase()
    const exactAddress = rawAddress !== undefined && result.value === rawAddress
    if (!exactAddress && !terms.every(term => result.keywords.toLocaleLowerCase().includes(term)))
      return []
    const score = exactAddress
      ? 1200
      : title === terms.join(" ")
        ? 1000
        : title.startsWith(terms.join(" "))
          ? 800
          : terms.every(term => title.includes(term))
            ? 600
            : 400
    return [{result, score: score + (result.environmentId === environment?.id ? 20 : 0)}]
  })

  if (identifier) {
    // An identifier does not identify its network. Outside an environment, each row is an explicit choice.
    const targets = environment ? [environment] : environments
    for (const target of targets) {
      if (target.status !== "running" || !supports(target, "explorer")) continue
      const result = resolveSearchDestination(
        {
          kind: address ? "address" : "transaction",
          title: `Open ${address ? "address" : "transaction"}${environment ? " in Explorer" : ` in ${target.name}`}`,
          value: identifier,
          environmentId: target.id,
        },
        environments,
      )
      if (result)
        ranked.push({
          result: {...result, group: environment ? "Objects" : "Choose an environment"},
          score: 1100,
        })
    }
  }

  ranked.sort((a, b) => b.score - a.score || a.result.title.localeCompare(b.result.title))
  const groups = new Map<string, StudioSearchResult[]>()
  for (const {result} of ranked) {
    const group = groups.get(result.group) ?? []
    const limit = result.group === "Choose an environment" ? environments.length : 6
    if (group.length < limit && !group.some(item => item.id === result.id)) group.push(result)
    groups.set(result.group, group)
  }
  return [...groups.values()].flat()
}

/** History is optional; denied storage or old malformed data must never disable search */
export function readSearchHistory(): SearchDestination[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]")
    if (!Array.isArray(value)) return []
    return value
      .filter(
        (item): item is SearchDestination =>
          item &&
          typeof item.title === "string" &&
          typeof item.value === "string" &&
          ["page", "environment", "contract", "wallet", "address", "transaction"].includes(
            item.kind,
          ) &&
          (item.environmentId === undefined || typeof item.environmentId === "string"),
      )
      .slice(0, HISTORY_LIMIT)
  } catch {
    return []
  }
}

/** Only public destination metadata is persisted, scoped by the environment's stable identity */
export function writeSearchHistory(history: readonly SearchDestination[]): void {
  try {
    localStorage.setItem(
      HISTORY_KEY,
      JSON.stringify(
        history
          .slice(0, HISTORY_LIMIT)
          .map(({kind, title, value, environmentId}) => ({kind, title, value, environmentId})),
      ),
    )
  } catch {
    // Search remains usable in browsers that disallow persistent storage.
  }
}

/** Prefer fresh labels and discard missing current-registry objects after import or restore */
export function recentSearchResults(
  history: readonly SearchDestination[],
  index: readonly StudioSearchResult[],
  environments: readonly StudioEnvironment[],
  environment: StudioEnvironment | undefined,
  objectsLoaded: boolean,
): StudioSearchResult[] {
  return history
    .flatMap(destination => {
      if (environment && destination.environmentId && destination.environmentId !== environment.id)
        return []
      const resolved = resolveSearchDestination(destination, environments)
      if (!resolved) return []
      const live = index.find(item => item.id === resolved.id && item.kind === resolved.kind)
      if (!live && (destination.kind === "page" || destination.kind === "environment")) return []
      if (
        !live &&
        objectsLoaded &&
        destination.environmentId === environment?.id &&
        (destination.kind === "contract" || destination.kind === "wallet")
      )
        return []
      return [{...(live ?? resolved), group: "Recent"}]
    })
    .filter((item, index, items) => items.findIndex(other => other.id === item.id) === index)
    .slice(0, 5)
}
