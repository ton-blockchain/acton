import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  RelativeTime,
  Select,
  TokenAmount,
} from "@acton/ui"
import {useCallback, useEffect, useRef, useState} from "react"
import type {FC, ReactNode} from "react"

import type {TonClient} from "../api/client"
import type {JettonMaster} from "../api/types"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {
  TOKEN_IMAGE_SOURCE_KEYS,
  TOKEN_PLACEHOLDER_IMAGE,
  getImageSources,
  replaceBrokenImageWithFallback,
} from "../components/imageFallbacks"
import {parseAddress} from "../components/utils"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useOpenExplorerPath} from "../hooks/useOpenExplorerPath"

import styles from "./TokenCatalogPage.module.css"

const TOKEN_PAGE_SIZE = 50
const RECENT_TRANSFER_BATCH_SIZE = 500

type TokenCatalogOrder = "recent" | "all"

interface TokenCatalogPageProps {
  readonly client: TonClient
  readonly embedded?: boolean
}

interface TokenCatalogLayoutProps {
  readonly children: ReactNode
  readonly embedded: boolean
  readonly order: TokenCatalogOrder
  readonly onOrderChange: (order: TokenCatalogOrder) => void
}

interface TokenCatalogItem {
  readonly token: JettonMaster
  readonly lastActivityAt?: number
}

interface TokenCatalogState {
  readonly items: TokenCatalogItem[]
  readonly hasMore: boolean
  readonly isLoading: boolean
  readonly isLoadingMore: boolean
  readonly nextOffset: number
  readonly error?: string
  readonly loadMoreError?: string
}

export const TokenCatalogPage: FC<TokenCatalogPageProps> = ({client, embedded = false}) => {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const currentClient = useRef(client)
  const [order, setOrder] = useState<TokenCatalogOrder>(embedded ? "all" : "recent")
  const currentOrder = useRef(order)
  const loadMoreRef = useRef<HTMLDivElement>(null)
  const [state, setState] = useState<TokenCatalogState>({
    items: [],
    hasMore: false,
    isLoading: true,
    isLoadingMore: false,
    nextOffset: 0,
  })

  useEffect(() => {
    let active = true
    currentClient.current = client
    currentOrder.current = order
    setState({
      items: [],
      hasMore: false,
      isLoading: true,
      isLoadingMore: false,
      nextOffset: 0,
    })

    void loadTokenBatch(client, order, 0, [])
      .then(batch => {
        if (!active) return
        setState({
          ...batch,
          isLoading: false,
          isLoadingMore: false,
        })
      })
      .catch((error: unknown) => {
        if (!active) return
        setState({
          items: [],
          hasMore: false,
          isLoading: false,
          isLoadingMore: false,
          nextOffset: 0,
          error: error instanceof Error ? error.message : "Failed to load tokens",
        })
      })

    return () => {
      active = false
    }
  }, [client, order])

  const loadMore = useCallback(() => {
    if (state.isLoadingMore || !state.hasMore) return
    setState(current => ({...current, isLoadingMore: true, loadMoreError: undefined}))

    void loadTokenBatch(
      client,
      order,
      state.nextOffset,
      state.items.map(item => item.token.address),
    )
      .then(batch => {
        if (currentClient.current !== client || currentOrder.current !== order) return
        setState(current => ({
          ...current,
          items: [...current.items, ...batch.items],
          hasMore: batch.hasMore,
          isLoadingMore: false,
          nextOffset: batch.nextOffset,
        }))
      })
      .catch((error: unknown) => {
        if (currentClient.current !== client || currentOrder.current !== order) return
        setState(current => ({
          ...current,
          isLoadingMore: false,
          loadMoreError: error instanceof Error ? error.message : "Failed to load more tokens",
        }))
      })
  }, [client, order, state.hasMore, state.isLoadingMore, state.items, state.nextOffset])

  const showLoadMore = !state.isLoading && state.hasMore

  useEffect(() => {
    const target = loadMoreRef.current
    if (
      !showLoadMore ||
      state.isLoadingMore ||
      state.loadMoreError ||
      !target ||
      typeof IntersectionObserver === "undefined"
    ) {
      return
    }

    let requested = false
    const observer = new IntersectionObserver(
      entries => {
        if (requested || !entries.some(entry => entry.isIntersecting)) return
        requested = true
        loadMore()
      },
      {rootMargin: "240px 0px"},
    )

    observer.observe(target)
    return () => observer.disconnect()
  }, [loadMore, showLoadMore, state.isLoadingMore, state.loadMoreError])

  return (
    <TokenCatalogLayout embedded={embedded} order={order} onOrderChange={setOrder}>
      <section
        className={styles.tableLayout}
        aria-busy={state.isLoading || state.isLoadingMore}
        aria-label={state.isLoading ? "Loading tokens" : undefined}
      >
        <DataTable minWidth={order === "recent" ? "63rem" : "54rem"}>
          <DataTableTable aria-label="Tokens" layout="fixed">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell>Token</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="16rem">
                  Supply
                </DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="8rem">Mintable</DataTableHeaderCell>
                {order === "recent" ? (
                  <DataTableHeaderCell columnWidth="9rem">Last activity</DataTableHeaderCell>
                ) : undefined}
                <DataTableHeaderCell columnWidth="17rem">Address</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {state.error ? (
                <DataTableEmpty colSpan={order === "recent" ? 5 : 4}>{state.error}</DataTableEmpty>
              ) : state.isLoading ? (
                <DataTableSkeletonRows
                  columns={order === "recent" ? 5 : 4}
                  rows={6}
                  alignments={
                    order === "recent"
                      ? ["left", "right", "left", "left", "left"]
                      : ["left", "right", "left", "left"]
                  }
                  widths={
                    order === "recent"
                      ? ["14rem", "8rem", "4rem", "6rem", "14rem"]
                      : ["14rem", "8rem", "4rem", "14rem"]
                  }
                  rowKeyPrefix="token-catalog-skeleton"
                />
              ) : state.items.length === 0 ? (
                <DataTableEmpty colSpan={order === "recent" ? 5 : 4}>
                  No tokens found
                </DataTableEmpty>
              ) : (
                state.items.map(item => {
                  const {token} = item
                  const href = routes.addressPath(token.address)
                  const imageSources = getImageSources(
                    token.jetton_content,
                    TOKEN_IMAGE_SOURCE_KEYS,
                  )

                  return (
                    <DataTableRow
                      key={token.address}
                      interactive
                      tabIndex={0}
                      onClick={event => openPath(href, event)}
                      onKeyDown={event => {
                        if (event.target !== event.currentTarget) return
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault()
                          openPath(href)
                        }
                      }}
                    >
                      <DataTableCell>
                        <div className={styles.tokenIdentity}>
                          <span className={styles.tokenImageFrame}>
                            <img
                              src={imageSources[0] ?? TOKEN_PLACEHOLDER_IMAGE}
                              alt=""
                              className={styles.tokenImage}
                              loading="lazy"
                              onError={event => replaceBrokenImageWithFallback(event, imageSources)}
                            />
                          </span>
                          <span className={styles.tokenText}>
                            <strong className={styles.tokenName}>
                              {token.jetton_content.name || "Unknown Jetton"}
                            </strong>
                            <span className={styles.tokenSymbol}>
                              {token.jetton_content.symbol || "Unknown symbol"}
                            </span>
                          </span>
                        </div>
                      </DataTableCell>
                      <DataTableCell align="right" tone="strong">
                        <TokenAmount
                          decimals={token.jetton_content.decimals}
                          symbol={token.jetton_content.symbol}
                          useGrouping
                          value={token.total_supply}
                        />
                      </DataTableCell>
                      <DataTableCell>
                        <span className={token.mintable ? styles.positive : styles.muted}>
                          {token.mintable ? "Yes" : "No"}
                        </span>
                      </DataTableCell>
                      {order === "recent" ? (
                        <DataTableCell>
                          {item.lastActivityAt ? (
                            <RelativeTime unit="seconds" value={item.lastActivityAt} />
                          ) : (
                            <span className={styles.muted}>—</span>
                          )}
                        </DataTableCell>
                      ) : undefined}
                      <DataTableCell>
                        <ExplorerAddressChip address={token.address} resolveName={false} />
                      </DataTableCell>
                    </DataTableRow>
                  )
                })
              )}
            </DataTableBody>
          </DataTableTable>
          {!state.error && showLoadMore ? (
            <div ref={loadMoreRef} className={styles.pagination}>
              {state.loadMoreError ? (
                <span className={styles.loadMoreError} role="alert">
                  {state.loadMoreError}
                </span>
              ) : undefined}
              <div className={styles.paginationControls}>
                <button
                  type="button"
                  className={styles.paginationButton}
                  disabled={state.isLoadingMore}
                  onClick={loadMore}
                >
                  {state.isLoadingMore ? "Loading..." : state.loadMoreError ? "Retry" : "Load more"}
                </button>
              </div>
            </div>
          ) : undefined}
        </DataTable>
      </section>
    </TokenCatalogLayout>
  )
}

function TokenCatalogLayout({children, embedded, order, onOrderChange}: TokenCatalogLayoutProps) {
  return (
    <div className={`${styles.page} ${embedded ? styles.pageEmbedded : ""}`}>
      {embedded ? undefined : (
        <>
          <ExplorerBreadcrumbs items={[{label: "Tokens"}]} />
          <section className={styles.hero}>
            <h1 className={styles.title}>Tokens</h1>
            <Select
              aria-label="Token list order"
              size="sm"
              value={order}
              onChange={event => onOrderChange(event.currentTarget.value as TokenCatalogOrder)}
            >
              <option value="recent">Recently active</option>
              <option value="all">All tokens</option>
            </Select>
          </section>
        </>
      )}
      <div className={styles.content}>{children}</div>
    </div>
  )
}

async function loadTokenBatch(
  client: TonClient,
  order: TokenCatalogOrder,
  offset: number,
  loadedAddresses: readonly string[],
): Promise<{
  readonly items: TokenCatalogItem[]
  readonly hasMore: boolean
  readonly nextOffset: number
}> {
  if (order === "recent") {
    return loadRecentlyActiveTokenBatch(client, offset, loadedAddresses)
  }

  const items = await client.getJettonMasters(undefined, TOKEN_PAGE_SIZE + 1, offset)
  return {
    items: items.slice(0, TOKEN_PAGE_SIZE).map(token => ({token})),
    hasMore: items.length > TOKEN_PAGE_SIZE,
    nextOffset: offset + TOKEN_PAGE_SIZE,
  }
}

async function loadRecentlyActiveTokenBatch(
  client: TonClient,
  offset: number,
  loadedAddresses: readonly string[],
): Promise<{
  readonly items: TokenCatalogItem[]
  readonly hasMore: boolean
  readonly nextOffset: number
}> {
  const transfers = await client.getJettonTransfers(RECENT_TRANSFER_BATCH_SIZE, offset)
  const seenAddresses = new Set(loadedAddresses.map(tokenAddressKey))
  const candidates: Array<{readonly address: string; readonly lastActivityAt: number}> = []
  let nextOffset = offset + transfers.length
  let hasBufferedToken = false

  for (const [index, transfer] of transfers.entries()) {
    if (transfer.transaction_aborted) continue

    const addressKey = tokenAddressKey(transfer.jetton_master)
    if (seenAddresses.has(addressKey)) continue

    if (candidates.length === TOKEN_PAGE_SIZE) {
      nextOffset = offset + index
      hasBufferedToken = true
      break
    }

    seenAddresses.add(addressKey)
    candidates.push({
      address: transfer.jetton_master,
      lastActivityAt: transfer.transaction_now,
    })
  }

  const hasMore = hasBufferedToken || transfers.length === RECENT_TRANSFER_BATCH_SIZE
  if (candidates.length === 0) {
    return {items: [], hasMore, nextOffset}
  }

  const masters = await client.getJettonMasters(candidates.map(candidate => candidate.address))
  const mastersByAddress = new Map(masters.map(master => [tokenAddressKey(master.address), master]))
  const items = candidates.flatMap(candidate => {
    const token = mastersByAddress.get(tokenAddressKey(candidate.address))
    return token ? [{token, lastActivityAt: candidate.lastActivityAt}] : []
  })

  return {
    items,
    hasMore,
    nextOffset,
  }
}

function tokenAddressKey(address: string): string {
  return parseAddress(address)?.toRawString() ?? address
}
