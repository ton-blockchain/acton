import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  ChevronsRight,
  Download,
  ExternalLink,
  FileJson,
  Star,
} from "lucide-react"
import {useNavigate, useParams} from "react-router"
import {
  BlockChip,
  BooleanValue,
  Button,
  CopyButton,
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  DateTime,
  formatDateTimeLocalInput,
  GramAmount,
  InlineAction,
  InlineActions,
  Input,
  NumberValue,
  Popover,
  RelativeTime,
  shortenMiddle,
  formatToncenterBlockId,
} from "@acton/ui"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {FC, FormEvent, ReactNode} from "react"

import type {RawBlockNetwork, TonClient} from "../api/client"
import {
  loadBlockTransactionsPage,
  type BlockTransactionListItem,
  type BlockTransactionsCursor,
} from "../api/blockTransactions"
import type {LoadNetworkTps} from "../api/networkStats"
import type {V3Block, V3BlockId, V3TransactionListItem} from "../api/types"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../components/DeveloperTransactionList"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {GlobalCapabilities} from "../components/GlobalCapabilities"
import {NetworkTpsPanel} from "../components/NetworkTpsPanel"
import {hashToHex} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useFavoriteBlocks} from "../hooks/useFavoriteBlocks"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../hooks/useTransactionMessageNames"

import styles from "./BlocksPage.module.css"

const BLOCKS_PAGE_LIMIT = 8
const LAST_TRANSACTION_MESSAGES_LIMIT = 5
const LAST_TRANSACTIONS_FETCH_LIMIT = 12
const BLOCK_TRANSACTIONS_INITIAL_LIMIT = 100
const BLOCK_TRANSACTIONS_LOAD_MORE_LIMIT = 100
const BLOCKS_REFRESH_MS = 2000
const MASTERCHAIN_SHARD = "8000000000000000"
const MIN_BLOCK_UNIX_TIME = 0
interface BlocksPageProps {
  readonly client: TonClient
  readonly loadNetworkTps?: LoadNetworkTps
}

interface BlockDetailsPageProps extends BlocksPageProps {
  readonly latest?: boolean
  readonly transactionsLoadMoreLimit?: number
}

interface BlocksPageState {
  readonly transactions: readonly V3TransactionListItem[]
  readonly masterchainBlocks: readonly V3Block[]
  readonly workchainBlocks: readonly V3Block[]
  readonly isLoading: boolean
  readonly error?: string
}

interface BlockDetailsState {
  readonly block?: V3Block
  readonly latestBlock?: V3Block
  readonly shardchainBlocks: readonly V3Block[]
  readonly transactions: readonly BlockTransactionListItem[]
  readonly transactionsCursor?: BlockTransactionsCursor
  readonly isLoading: boolean
  readonly areTransactionsUnavailable: boolean
  readonly isLoadingMoreTransactions: boolean
  readonly hasMoreTransactions: boolean
  readonly loadMoreTransactionsError?: string
  readonly error?: string
}

export const BlocksPage: FC<BlocksPageProps> = ({client, loadNetworkTps}) => {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames, updateDomains} = useAddressBook()
  const [state, setState] = useState<BlocksPageState>({
    transactions: [],
    masterchainBlocks: [],
    workchainBlocks: [],
    isLoading: true,
  })
  const {addresses, messageNamesByAddress} = useTransactionMessageNames(client, state.transactions)

  useEffect(() => {
    void prefetchNames(addresses)
  }, [addresses, prefetchNames])

  useEffect(() => {
    let isActive = true
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadBlocksPage = async (showLoading: boolean) => {
      if (showLoading) {
        setState(current => ({
          ...current,
          isLoading: true,
          error: undefined,
        }))
      }
      try {
        const [transactions, masterchainBlocks, workchainBlocks] = await Promise.all([
          client.getRecentTransactions(LAST_TRANSACTIONS_FETCH_LIMIT),
          client.getBlocks({
            workchain: -1,
            limit: BLOCKS_PAGE_LIMIT,
            sort: "desc",
          }),
          client.getBlocks({
            workchain: 0,
            limit: BLOCKS_PAGE_LIMIT,
            sort: "desc",
          }),
        ])

        if (!isActive) {
          return
        }

        updateDomains(transactions.address_book)
        setState({
          transactions: transactions.transactions,
          masterchainBlocks: masterchainBlocks.blocks,
          workchainBlocks: workchainBlocks.blocks,
          isLoading: false,
        })
      } catch (error) {
        if (!isActive) {
          return
        }
        setState(current => ({
          ...current,
          isLoading: false,
          error:
            current.masterchainBlocks.length === 0 && current.workchainBlocks.length === 0
              ? error instanceof Error
                ? error.message
                : "Failed to load blocks"
              : undefined,
        }))
      } finally {
        if (isActive) {
          timeoutId = globalThis.setTimeout(() => void loadBlocksPage(false), BLOCKS_REFRESH_MS)
        }
      }
    }

    void loadBlocksPage(true)

    return () => {
      isActive = false
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client, updateDomains])

  return (
    <div className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Blocks"}]} />
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Blocks</h1>
        </div>
      </section>

      {loadNetworkTps ? <NetworkTpsPanel loadNetworkTps={loadNetworkTps} /> : null}

      <section className={styles.blocksLayout}>
        {state.error ? (
          <TableStateBlock>{state.error}</TableStateBlock>
        ) : state.isLoading ? (
          <DeveloperTransactionListSkeleton
            className={styles.blocksTransactionsTable}
            title="Last transactions"
            rows={LAST_TRANSACTION_MESSAGES_LIMIT}
          />
        ) : (
          <DeveloperTransactionList
            className={styles.blocksTransactionsTable}
            title="Last transactions"
            transactions={state.transactions}
            maxRows={LAST_TRANSACTION_MESSAGES_LIMIT}
            messageNamesByAddress={messageNamesByAddress}
            onTransactionClick={(hashHex, _transaction, event) => {
              openPath(routes.transactionPath(hashHex), event)
            }}
            onAddressClick={(address, event) => {
              openPath(routes.addressPath(address), event)
            }}
          />
        )}

        <div className={styles.blocksTableGrid}>
          <BlockTableSection
            title="Last masterchain blocks"
            blocks={state.masterchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No masterchain blocks yet"
            onOpenBlock={(block, event) =>
              openPath(routes.blockPath(block.workchain, block.shard, block.seqno), event)
            }
          />
          <BlockTableSection
            title="Last workchain blocks"
            blocks={state.workchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No workchain blocks yet"
            onOpenBlock={(block, event) =>
              openPath(routes.blockPath(block.workchain, block.shard, block.seqno), event)
            }
          />
        </div>
      </section>
    </div>
  )
}

export const BlockDetailsPage: FC<BlockDetailsPageProps> = ({
  client,
  latest = false,
  transactionsLoadMoreLimit = BLOCK_TRANSACTIONS_LOAD_MORE_LIMIT,
}) => {
  const params = useParams<{
    workchain: string
    shard: string
    seqno: string
  }>()
  const navigate = useNavigate()
  const {network, nodeInfo} = useNetworkInfo()
  const {isFavorite: isFavoriteBlock, toggleFavorite: toggleFavoriteBlock} = useFavoriteBlocks()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames, updateDomains} = useAddressBook()
  const forkBlockNumber = nodeInfo?.fork_block_number ?? undefined
  const rawBlockNetwork: RawBlockNetwork | undefined =
    network.id === "mainnet" || network.id === "testnet" ? network.id : undefined
  const routeWorkchain = Number(params.workchain)
  const routeShard = params.shard ?? ""
  const routeSeqno = Number(params.seqno)
  const isPublicBlock =
    rawBlockNetwork !== undefined &&
    (forkBlockNumber === undefined ||
      forkBlockNumber === null ||
      (!latest && routeSeqno <= forkBlockNumber))
  const publicBlockNetwork: RawBlockNetwork | undefined = isPublicBlock
    ? rawBlockNetwork
    : undefined
  const [state, setState] = useState<BlockDetailsState>({
    shardchainBlocks: [],
    transactions: [],
    isLoading: true,
    areTransactionsUnavailable: false,
    isLoadingMoreTransactions: false,
    hasMoreTransactions: false,
  })
  const isLoadingMoreTransactionsRef = useRef(false)

  useEffect(() => {
    let isActive = true

    const loadBlockDetails = async () => {
      if (
        !latest &&
        (!Number.isInteger(routeWorkchain) || !Number.isInteger(routeSeqno) || !routeShard)
      ) {
        setState({
          shardchainBlocks: [],
          transactions: [],
          isLoading: false,
          areTransactionsUnavailable: false,
          isLoadingMoreTransactions: false,
          hasMoreTransactions: false,
          error: "Invalid block route.",
        })
        return
      }

      setState(current => ({
        ...current,
        isLoading: true,
        transactionsCursor: undefined,
        areTransactionsUnavailable: false,
        isLoadingMoreTransactions: false,
        hasMoreTransactions: false,
        loadMoreTransactionsError: undefined,
        error: undefined,
      }))
      try {
        const [blockResponse, latestResponse] = await Promise.all([
          latest
            ? Promise.resolve({blocks: []})
            : client.getBlocks({
                workchain: routeWorkchain,
                shard: routeShard,
                seqno: routeSeqno,
                limit: 1,
              }),
          latest
            ? client.getBlocks({workchain: -1, limit: 1, sort: "desc"})
            : client.getBlocks({
                workchain: routeWorkchain,
                shard: routeShard,
                limit: 1,
                sort: "desc",
              }),
        ])
        const block = latest
          ? latestResponse.blocks[0]
          : blockResponse.blocks.find(candidate =>
              isSameBlock(candidate, routeWorkchain, routeShard, routeSeqno),
            )

        if (!block) {
          if (isActive) {
            setState({
              latestBlock: latestResponse.blocks[0],
              shardchainBlocks: [],
              transactions: [],
              isLoading: false,
              areTransactionsUnavailable: false,
              isLoadingMoreTransactions: false,
              hasMoreTransactions: false,
              error: "Block not found.",
            })
          }
          return
        }

        const rawBlockMetadataPromise =
          publicBlockNetwork &&
          (block.gen_software_version === undefined ||
            block.gen_software_capabilities === undefined ||
            block.fees_collected === undefined)
            ? client
                .getRawBlockBoc(getExtendedBlockId(block), publicBlockNetwork)
                .then(async cell => {
                  const {parseBlockMetadata} = await import("../cell-inspector/blockParser")
                  return parseBlockMetadata(cell)
                })
                .catch(() => undefined)
            : Promise.resolve(undefined)
        const [transactionsResponse, shardchainResponse, rawBlockMetadata] = await Promise.all([
          loadBlockTransactionsPage(client, block, BLOCK_TRANSACTIONS_INITIAL_LIMIT),
          block.workchain === -1
            ? client.getMasterchainBlockShards(block.seqno)
            : Promise.resolve({blocks: []}),
          rawBlockMetadataPromise,
        ])

        updateDomains(transactionsResponse.addressBook)

        if (!isActive) {
          return
        }

        setState({
          block: rawBlockMetadata
            ? {
                ...block,
                gen_software_version:
                  block.gen_software_version ?? rawBlockMetadata.genSoftwareVersion,
                gen_software_capabilities:
                  block.gen_software_capabilities ??
                  rawBlockMetadata.genSoftwareCapabilities?.toString(),
                fees_collected: block.fees_collected ?? rawBlockMetadata.feesCollected.toString(),
              }
            : block,
          latestBlock: latestResponse.blocks[0],
          shardchainBlocks: shardchainResponse.blocks,
          transactions: transactionsResponse.transactions,
          transactionsCursor: transactionsResponse.nextCursor,
          isLoading: false,
          areTransactionsUnavailable: transactionsResponse.unavailable,
          isLoadingMoreTransactions: false,
          hasMoreTransactions: transactionsResponse.nextCursor !== undefined,
        })
      } catch (error) {
        if (!isActive) {
          return
        }
        setState(current => ({
          ...current,
          isLoading: false,
          areTransactionsUnavailable: false,
          isLoadingMoreTransactions: false,
          hasMoreTransactions: false,
          error: error instanceof Error ? error.message : "Failed to load block",
        }))
      }
    }

    void loadBlockDetails()

    return () => {
      isActive = false
    }
  }, [client, latest, publicBlockNetwork, routeShard, routeSeqno, routeWorkchain, updateDomains])

  const loadMoreTransactions = useCallback(() => {
    const block = state.block
    const offset = state.transactions.length
    const cursor = state.transactionsCursor
    if (
      !block ||
      state.isLoading ||
      !state.hasMoreTransactions ||
      !cursor ||
      isLoadingMoreTransactionsRef.current
    ) {
      return
    }

    isLoadingMoreTransactionsRef.current = true
    setState(current => ({
      ...current,
      isLoadingMoreTransactions: true,
      loadMoreTransactionsError: undefined,
    }))

    void (async () => {
      try {
        const response = await loadBlockTransactionsPage(
          client,
          block,
          transactionsLoadMoreLimit,
          cursor,
        )
        updateDomains(response.addressBook)

        setState(current => {
          if (
            current.isLoading ||
            !current.block ||
            current.transactions.length !== offset ||
            current.transactionsCursor !== cursor ||
            !isSameBlock(current.block, block.workchain, block.shard, block.seqno)
          ) {
            return current
          }

          const transactions = [...current.transactions, ...response.transactions]
          return {
            ...current,
            transactions,
            transactionsCursor: response.nextCursor,
            isLoadingMoreTransactions: false,
            hasMoreTransactions: response.nextCursor !== undefined,
          }
        })
      } catch (error) {
        setState(current => {
          if (
            current.isLoading ||
            !current.block ||
            current.transactions.length !== offset ||
            current.transactionsCursor !== cursor ||
            !isSameBlock(current.block, block.workchain, block.shard, block.seqno)
          ) {
            return current
          }
          return {
            ...current,
            isLoadingMoreTransactions: false,
            loadMoreTransactionsError:
              error instanceof Error ? error.message : "Failed to load more transactions",
          }
        })
      } finally {
        isLoadingMoreTransactionsRef.current = false
      }
    })()
  }, [
    client,
    state.block,
    state.hasMoreTransactions,
    state.isLoading,
    state.transactions.length,
    state.transactionsCursor,
    transactionsLoadMoreLimit,
    updateDomains,
  ])

  const workchain = latest ? (state.block?.workchain ?? -1) : routeWorkchain
  const shard = latest ? (state.block?.shard ?? MASTERCHAIN_SHARD) : routeShard
  const seqno = latest ? (state.block?.seqno ?? Number.NaN) : routeSeqno

  const title = workchain === -1 ? "Masterchain block" : "Workchain block"
  const hasResolvedBlockId =
    Number.isInteger(workchain) && Number.isInteger(seqno) && Boolean(shard)
  const hasValidRoute = latest || hasResolvedBlockId
  const blockId = hasResolvedBlockId ? formatToncenterBlockId({workchain, shard, seqno}) : undefined
  const latestPath = state.latestBlock
    ? routes.blockPath(
        state.latestBlock.workchain,
        state.latestBlock.shard,
        state.latestBlock.seqno,
      )
    : undefined
  const canOpenPrev = hasResolvedBlockId && seqno > 1
  const prevPath = canOpenPrev ? routes.blockPath(workchain, shard, seqno - 1) : undefined
  const nextPath = hasResolvedBlockId ? routes.blockPath(workchain, shard, seqno + 1) : undefined
  const transactionAddresses = useMemo(
    () => state.transactions.map(transaction => transaction.account),
    [state.transactions],
  )
  const blockActions = state.block ? getBlockActions(state.block, publicBlockNetwork) : undefined
  const favoriteBlock = state.block
    ? {
        workchain: state.block.workchain,
        shard: state.block.shard,
        seqno: state.block.seqno,
        generatedAt: blockUnixTime(state.block),
      }
    : undefined
  const blockIsFavorite = favoriteBlock ? isFavoriteBlock(favoriteBlock) : false

  useEffect(() => {
    void prefetchNames(transactionAddresses)
  }, [prefetchNames, transactionAddresses])

  return (
    <div className={styles.container}>
      <ExplorerBreadcrumbs
        items={[
          {label: "Blocks", path: routes.blocksPath},
          {
            label: blockId ?? title,
            copy: blockId
              ? {
                  value: blockId,
                  label: "Copy block ID",
                  copiedLabel: "Block ID copied",
                }
              : undefined,
          },
        ]}
      />
      <section className={styles.hero}>
        <div className={styles.titleRow}>
          <h1 className={styles.title}>{title}</h1>
          {favoriteBlock ? (
            <InlineAction
              className={blockIsFavorite ? styles.favoriteActionActive : undefined}
              label={blockIsFavorite ? "Remove block from favorites" : "Add block to favorites"}
              icon={<Star className={blockIsFavorite ? styles.favoriteIconActive : undefined} />}
              aria-pressed={blockIsFavorite}
              onClick={() => toggleFavoriteBlock(favoriteBlock)}
            />
          ) : null}
        </div>
      </section>

      <section className={styles.blocksLayout}>
        {hasValidRoute ? (
          <div className={styles.blockDetailControls}>
            <div className={styles.blockDetailToolbar} aria-label="Block navigation">
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<ChevronLeft size={14} />}
                disabled={!prevPath}
                onClick={() => prevPath && void navigate(prevPath)}
              >
                Prev block
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronRight size={14} />}
                disabled={!nextPath}
                onClick={() => nextPath && void navigate(nextPath)}
              >
                Next block
              </Button>
              <BlockDateNavigation
                client={client}
                currentBlock={state.block}
                onOpenBlock={block =>
                  void navigate(routes.blockPath(block.workchain, block.shard, block.seqno))
                }
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronsRight size={14} />}
                disabled={
                  !latestPath ||
                  (state.block !== undefined &&
                    latestPath ===
                      routes.blockPath(state.block.workchain, state.block.shard, state.block.seqno))
                }
                onClick={() => latestPath && void navigate(latestPath)}
              >
                Latest
              </Button>
            </div>

            {state.block && blockActions ? (
              <div className={styles.blockHeaderActions} aria-label="Block actions">
                {publicBlockNetwork && blockActions.downloadUrl ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    leadingIcon={<Download size={14} />}
                    onClick={() =>
                      globalThis.open(blockActions.downloadUrl, "_blank", "noopener,noreferrer")
                    }
                  >
                    Download
                  </Button>
                ) : null}
                {publicBlockNetwork ? (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    leadingIcon={<FileJson size={14} />}
                    disabled={blockActions.configSeqno === undefined}
                    onClick={() =>
                      blockActions.configSeqno !== undefined &&
                      void navigate(routes.configPath(blockActions.configSeqno))
                    }
                  >
                    Config
                  </Button>
                ) : null}
                <CopyButton
                  value={blockActions.extendedBlockId}
                  label="Copy extended block ID"
                  copiedLabel="Extended block ID copied"
                  copiedChildren="Copied ID"
                  variant="outline"
                  size="sm"
                  className={styles.blockCopyButton}
                >
                  Copy block ID
                </CopyButton>
                {publicBlockNetwork ? (
                  <>
                    <span className={styles.blockActionSeparator} aria-hidden="true" />
                    <a
                      className={styles.blockExplorerLink}
                      href={blockActions.tonscanUrl}
                      target="_blank"
                      rel="noreferrer"
                    >
                      Tonscan
                      <ExternalLink size={13} aria-hidden="true" />
                    </a>
                    <a
                      className={styles.blockExplorerLink}
                      href={blockActions.toncoinUrl}
                      target="_blank"
                      rel="noreferrer"
                    >
                      toncoin.org
                      <ExternalLink size={13} aria-hidden="true" />
                    </a>
                  </>
                ) : null}
              </div>
            ) : null}
          </div>
        ) : null}

        {state.error ? (
          <TableStateBlock>{state.error}</TableStateBlock>
        ) : state.isLoading || !state.block ? (
          <BlockDetailsSkeleton
            showShardchainBlocks={workchain === -1}
            showRawBlockFields={publicBlockNetwork !== undefined}
          />
        ) : (
          <>
            <BlockSummaryTable
              block={state.block}
              onOpenBlock={(block, event) =>
                openPath(routes.blockPath(block.workchain, block.shard, block.seqno), event)
              }
            />

            {state.block.workchain === -1 ? (
              <BlockTableSection
                title="Last shard blocks"
                blocks={state.shardchainBlocks}
                blockDisplay="full"
                isLoading={false}
                emptyLabel="No shardchain blocks for this masterchain block"
                showShardFlags
                onOpenBlock={(block, event) =>
                  openPath(routes.blockPath(block.workchain, block.shard, block.seqno), event)
                }
              />
            ) : null}

            <BlockTransactionsTable
              transactions={state.transactions}
              areTransactionsUnavailable={state.areTransactionsUnavailable}
              hasMore={state.hasMoreTransactions}
              isLoadingMore={state.isLoadingMoreTransactions}
              loadMoreError={state.loadMoreTransactionsError}
              onLoadMore={loadMoreTransactions}
              onOpenAccount={(address, event) => openPath(routes.addressPath(address), event)}
              onOpenTransaction={(hash, event) =>
                openPath(routes.transactionPath(hashToHex(hash) ?? hash), event)
              }
            />
          </>
        )}
      </section>
    </div>
  )
}

const BlockDateNavigation: FC<{
  readonly client: TonClient
  readonly currentBlock?: V3Block
  readonly onOpenBlock: (block: V3Block) => void
}> = ({client, currentBlock, onOpenBlock}) => {
  const [isOpen, setIsOpen] = useState(false)
  const [dateValue, setDateValue] = useState("")
  const [minDateValue, setMinDateValue] = useState(() =>
    formatDateTimeLocalInput(MIN_BLOCK_UNIX_TIME),
  )
  const [isSearching, setIsSearching] = useState(false)
  const [error, setError] = useState<string>()

  useEffect(() => {
    let isActive = true
    setMinDateValue(formatDateTimeLocalInput(MIN_BLOCK_UNIX_TIME))

    const loadEarliestBlockTime = async () => {
      try {
        const response = await client.getBlocks({workchain: -1, limit: 1, sort: "asc"})
        const unixTime = response.blocks[0] && blockUnixTime(response.blocks[0])
        if (isActive && unixTime !== undefined) {
          setMinDateValue(formatDateTimeLocalInput(unixTime))
        }
      } catch {
        // Fall back to the minimum value supported by the block timestamp format.
      }
    }

    void loadEarliestBlockTime()

    return () => {
      isActive = false
    }
  }, [client])

  const handleOpenChange = (nextOpen: boolean) => {
    setIsOpen(nextOpen)
    setError(undefined)
    if (nextOpen && currentBlock) {
      const unixTime = blockUnixTime(currentBlock)
      if (unixTime !== undefined) {
        setDateValue(formatDateTimeLocalInput(unixTime))
      }
    }
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const timestamp = new Date(dateValue).getTime()
    if (!Number.isFinite(timestamp)) {
      setError("Select a valid date and time")
      return
    }

    setIsSearching(true)
    setError(undefined)
    try {
      const response = await client.getBlocks({
        workchain: -1,
        endUtime: Math.floor(timestamp / 1000),
        limit: 1,
        sort: "desc",
      })
      const block = response.blocks[0]
      if (!block) {
        setError("No masterchain block exists at or before this time")
        return
      }

      setIsOpen(false)
      onOpenBlock(block)
    } catch {
      setError("Failed to find a block. Try again")
    } finally {
      setIsSearching(false)
    }
  }

  return (
    <Popover
      ariaLabel="Navigate to block by date"
      content={
        <form className={styles.blockDateForm} onSubmit={event => void handleSubmit(event)}>
          <Input
            type="datetime-local"
            step={1}
            size="sm"
            label="Local date and time"
            value={dateValue}
            min={minDateValue}
            disabled={isSearching}
            onChange={event => setDateValue(event.currentTarget.value)}
          />
          <p className={styles.blockDateHint}>
            Opens the latest masterchain block at or before this time
          </p>
          {error ? (
            <p className={styles.blockDateError} role="alert">
              {error}
            </p>
          ) : null}
          <div className={styles.blockDateActions}>
            <Button type="submit" variant="primary" size="sm" disabled={!dateValue || isSearching}>
              {isSearching ? "Finding…" : "Open block"}
            </Button>
          </div>
        </form>
      }
      interaction="click"
      placement="bottom"
      open={isOpen}
      onOpenChange={handleOpenChange}
      maxWidth="min(24rem, calc(100vw - 2rem))"
      triggerAsChild
    >
      <Button
        type="button"
        variant="outline"
        size="sm"
        leadingIcon={<CalendarDays size={14} />}
        disabled={!currentBlock}
      >
        By date
      </Button>
    </Popover>
  )
}

const BlockTableSection: FC<{
  readonly title: string
  readonly blocks: readonly V3Block[]
  readonly blockDisplay?: "seqno" | "full"
  readonly isLoading: boolean
  readonly emptyLabel: string
  readonly showShardFlags?: boolean
  readonly onOpenBlock: (block: V3Block, event?: ExplorerNavigationClickEvent) => void
}> = ({
  title,
  blocks,
  blockDisplay = "seqno",
  isLoading,
  emptyLabel,
  showShardFlags = false,
  onOpenBlock,
}) => {
  const routes = useExplorerRoutePaths()

  if (isLoading) {
    return <BlockTableSkeleton title={title} rows={4} showShardFlags={showShardFlags} />
  }

  return (
    <DataTable
      title={title}
      minWidth={blockDisplay === "full" ? "54rem" : showShardFlags ? "42rem" : "32.5rem"}
      aria-label={title}
    >
      <DataTableTable aria-label={title} layout="fixed">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell
              columnWidth={blockDisplay === "full" ? "20rem" : showShardFlags ? "20%" : "14rem"}
            >
              Block
            </DataTableHeaderCell>
            <DataTableHeaderCell columnWidth={showShardFlags ? "18%" : "8rem"}>
              Transactions
            </DataTableHeaderCell>
            <DataTableHeaderCell columnWidth={showShardFlags ? "18%" : "12rem"}>
              Generated at
            </DataTableHeaderCell>
            {showShardFlags ? (
              <>
                <DataTableHeaderCell columnWidth="11%">Before split</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="11%">After split</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="11%">Want split</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="11%">Want merge</DataTableHeaderCell>
              </>
            ) : null}
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {blocks.length === 0 ? (
            <DataTableEmpty colSpan={showShardFlags ? 7 : 3}>{emptyLabel}</DataTableEmpty>
          ) : (
            blocks.map(block => (
              <DataTableRow
                key={formatToncenterBlockId(block)}
                interactive
                tabIndex={0}
                onClick={event => onOpenBlock(block, event)}
                onKeyDown={event => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    onOpenBlock(block)
                  }
                }}
              >
                <DataTableCell>
                  <BlockChip
                    workchain={block.workchain}
                    shard={block.shard}
                    seqno={block.seqno}
                    display={blockDisplay}
                    href={routes.blockPath(block.workchain, block.shard, block.seqno)}
                    onClick={event => {
                      event.stopPropagation()
                      onOpenBlock(block, event)
                    }}
                  />
                </DataTableCell>
                <DataTableCell>
                  <NumberValue value={block.tx_count} />
                </DataTableCell>
                <DataTableCell truncate>
                  <DateTime
                    display="date-time-numeric-seconds"
                    fallback="Unknown"
                    unit="seconds"
                    value={blockUnixTime(block)}
                  />
                </DataTableCell>
                {showShardFlags ? (
                  <>
                    <DataTableCell>
                      <BooleanValue display="true-false" value={block.before_split} />
                    </DataTableCell>
                    <DataTableCell>
                      <BooleanValue display="true-false" value={block.after_split} />
                    </DataTableCell>
                    <DataTableCell>
                      <BooleanValue display="true-false" value={block.want_split} />
                    </DataTableCell>
                    <DataTableCell>
                      <BooleanValue display="true-false" value={block.want_merge} />
                    </DataTableCell>
                  </>
                ) : null}
              </DataTableRow>
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

const BlockTransactionsTable: FC<{
  readonly transactions: readonly BlockTransactionListItem[]
  readonly areTransactionsUnavailable: boolean
  readonly hasMore: boolean
  readonly isLoadingMore: boolean
  readonly loadMoreError?: string
  readonly onLoadMore: () => void
  readonly onOpenAccount: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onOpenTransaction: (hash: string, event?: ExplorerNavigationClickEvent) => void
}> = ({
  transactions,
  areTransactionsUnavailable,
  hasMore,
  isLoadingMore,
  loadMoreError,
  onLoadMore,
  onOpenAccount,
  onOpenTransaction,
}) => {
  const loadMoreRef = useRef<HTMLDivElement>(null)
  const columnCount = 5

  useEffect(() => {
    const target = loadMoreRef.current
    if (
      !hasMore ||
      isLoadingMore ||
      loadMoreError ||
      !target ||
      typeof IntersectionObserver === "undefined"
    ) {
      return
    }

    let requested = false
    const observer = new IntersectionObserver(
      entries => {
        if (requested || !entries.some(entry => entry.isIntersecting)) {
          return
        }
        requested = true
        onLoadMore()
      },
      {rootMargin: "240px 0px"},
    )

    observer.observe(target)
    return () => observer.disconnect()
  }, [hasMore, isLoadingMore, loadMoreError, onLoadMore])

  return (
    <DataTable title="Transactions" minWidth="47.5rem" aria-label="Transactions">
      <DataTableTable aria-label="Transactions" layout="fixed">
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="3.375rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Account</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8.125rem">Logical time</DataTableHeaderCell>
            <DataTableHeaderCell>Hash</DataTableHeaderCell>
            <DataTableHeaderCell align="right" columnWidth="8.125rem">
              Exit code
            </DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {transactions.length === 0 ? (
            <DataTableEmpty colSpan={columnCount}>
              {areTransactionsUnavailable
                ? "Transactions are unavailable"
                : "No transactions in this block"}
            </DataTableEmpty>
          ) : (
            transactions.map((transaction, index) => {
              const hash = hashToHex(transaction.hash) ?? transaction.hash
              const exitCode = formatTransactionExitCode(transaction)
              const hasNonZeroExitCode = exitCode !== "0" && exitCode !== "Unknown"
              return (
                <DataTableRow
                  key={`${transaction.hash}:${transaction.lt}`}
                  interactive
                  tabIndex={0}
                  onClick={event => onOpenTransaction(transaction.hash, event)}
                  onKeyDown={event => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault()
                      onOpenTransaction(transaction.hash)
                    }
                  }}
                >
                  <DataTableCell tone="muted">{index + 1}</DataTableCell>
                  <DataTableCell>
                    <ExplorerAddressChip
                      address={transaction.account}
                      fallback="Account"
                      onAddressClick={onOpenAccount}
                    />
                  </DataTableCell>
                  <DataTableCell mono>{transaction.lt}</DataTableCell>
                  <DataTableCell>
                    <span className={styles.blocksHashCell}>
                      <span className={styles.blocksHashText} title={hash}>
                        {shortenMiddle(hash, {maxLength: 19})}
                      </span>
                      <CopyInlineAction
                        value={hash}
                        size="compact"
                        label="Copy transaction hash"
                        copiedLabel="Transaction hash copied"
                      />
                    </span>
                  </DataTableCell>
                  <DataTableCell
                    align="right"
                    mono
                    className={hasNonZeroExitCode ? styles.blocksExitCodeFailure : undefined}
                  >
                    {exitCode}
                  </DataTableCell>
                </DataTableRow>
              )
            })
          )}
        </DataTableBody>
        {hasMore && transactions.length > 0 ? (
          <DataTableFooter>
            <DataTableRow>
              <DataTableCell colSpan={columnCount} className={styles.blockTransactionsLoadMoreCell}>
                <div ref={loadMoreRef} className={styles.blockTransactionsLoadMore}>
                  {loadMoreError ? (
                    <span className={styles.blockTransactionsLoadMoreError} role="alert">
                      {loadMoreError}
                    </span>
                  ) : null}
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={onLoadMore}
                    disabled={isLoadingMore}
                  >
                    {isLoadingMore ? "Loading..." : loadMoreError ? "Retry" : "Load more"}
                  </Button>
                </div>
              </DataTableCell>
            </DataTableRow>
          </DataTableFooter>
        ) : null}
      </DataTableTable>
    </DataTable>
  )
}

const BlockTableSkeleton: FC<{
  readonly title: string
  readonly rows: number
  readonly showShardFlags?: boolean
}> = ({title, rows, showShardFlags = false}) => (
  <DataTable
    title={title}
    minWidth={showShardFlags ? "42rem" : "32.5rem"}
    aria-label={`Loading ${title}`}
  >
    <DataTableTable aria-busy="true" aria-label={title} layout="fixed">
      <DataTableHead>
        <DataTableRow>
          <DataTableHeaderCell columnWidth={showShardFlags ? "20%" : "14rem"}>
            Block
          </DataTableHeaderCell>
          <DataTableHeaderCell columnWidth={showShardFlags ? "18%" : "8rem"}>
            Transactions
          </DataTableHeaderCell>
          <DataTableHeaderCell columnWidth={showShardFlags ? "18%" : "12rem"}>
            Generated at
          </DataTableHeaderCell>
          {showShardFlags ? (
            <>
              <DataTableHeaderCell columnWidth="11%">Before split</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="11%">After split</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="11%">Want split</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="11%">Want merge</DataTableHeaderCell>
            </>
          ) : null}
        </DataTableRow>
      </DataTableHead>
      <DataTableBody>
        <DataTableSkeletonRows
          columns={showShardFlags ? 7 : 3}
          rows={rows}
          alignments={
            showShardFlags ? ["left", "left", "left", "left", "left", "left", "left"] : undefined
          }
          widths={
            showShardFlags
              ? ["8rem", "5rem", "8rem", "2.5rem", "2.5rem", "2.5rem", "2.5rem"]
              : ["8rem", "5rem", "10rem"]
          }
        />
      </DataTableBody>
    </DataTableTable>
  </DataTable>
)

const BlockTransactionsTableSkeleton: FC<{readonly rows: number}> = ({rows}) => (
  <DataTable title="Transactions" minWidth="47.5rem" aria-label="Loading transactions">
    <DataTableTable aria-busy="true" aria-label="Transactions" layout="fixed">
      <DataTableHead>
        <DataTableRow>
          <DataTableHeaderCell columnWidth="3.375rem">#</DataTableHeaderCell>
          <DataTableHeaderCell>Account</DataTableHeaderCell>
          <DataTableHeaderCell columnWidth="8.125rem">Logical time</DataTableHeaderCell>
          <DataTableHeaderCell>Hash</DataTableHeaderCell>
          <DataTableHeaderCell align="right" columnWidth="8.125rem">
            Exit code
          </DataTableHeaderCell>
        </DataTableRow>
      </DataTableHead>
      <DataTableBody>
        <DataTableSkeletonRows
          columns={5}
          rows={rows}
          alignments={["left", "left", "left", "left", "right"]}
          widths={["2rem", "12rem", "7rem", "15rem", "3rem"]}
        />
      </DataTableBody>
    </DataTableTable>
  </DataTable>
)

const BlockSummaryTable: FC<{
  readonly block: V3Block
  readonly onOpenBlock: (block: V3BlockId, event: ExplorerNavigationClickEvent) => void
}> = ({block, onOpenBlock}) => {
  const routes = useExplorerRoutePaths()

  const rootHash = formatBlockHash(block.root_hash)
  const fileHash = formatBlockHash(block.file_hash)
  const createdBy = formatBlockHash(block.created_by)
  const randSeed = formatBlockHash(block.rand_seed)
  const masterchainBlockRef = block.masterchain_block_ref
  const masterchainShard = masterchainBlockRef?.shard ?? MASTERCHAIN_SHARD
  const prevKeyBlockSeqno = block.prev_key_block_seqno
  const minRefMcSeqno = block.min_ref_mc_seqno
  const genUtime = blockUnixTime(block)
  const workchainName =
    block.workchain === -1 ? "Masterchain" : block.workchain === 0 ? "Basechain" : undefined
  const shardName = block.shard === MASTERCHAIN_SHARD ? "root shard" : undefined
  const globalIdName =
    block.global_id === -239 ? "Mainnet" : block.global_id === -3 ? "Testnet" : undefined
  const hasGenSoftware =
    block.gen_software_version !== undefined || block.gen_software_capabilities !== undefined

  return (
    <section className={styles.blockDetailsPanel} aria-label="Block details">
      <BlockDetailSection label="Identity" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailItem
          label="Workchain"
          value={
            <>
              {block.workchain}
              {workchainName ? (
                <>
                  {" "}
                  <span className={styles.blockDetailSecondaryValue}>({workchainName})</span>
                </>
              ) : null}
            </>
          }
        />
        <BlockDetailItem
          label="Shard"
          value={
            <>
              {block.shard}
              {shardName ? (
                <>
                  {" "}
                  <span className={styles.blockDetailSecondaryValue}>({shardName})</span>
                </>
              ) : null}
            </>
          }
          mono
        />
        <BlockDetailItem label="Seqno" value={block.seqno.toString()} mono />
        <BlockDetailItem
          label="Global ID"
          value={
            <>
              {formatOptionalNumber(block.global_id)}
              {globalIdName ? (
                <>
                  {" "}
                  <span className={styles.blockDetailSecondaryValue}>({globalIdName})</span>
                </>
              ) : null}
            </>
          }
          mono
        />
      </BlockDetailSection>

      <BlockDetailSection label="Hashes" contentClassName={styles.blockHashesGrid}>
        <BlockDetailItem label="Root hash" value={rootHash} copyValue={rootHash} mono />
        <BlockDetailItem label="File hash" value={fileHash} copyValue={fileHash} mono />
        <BlockDetailItem label="Created by" value={createdBy} copyValue={createdBy} mono />
        <BlockDetailItem label="Rand seed" value={randSeed} copyValue={randSeed} mono />
      </BlockDetailSection>

      <BlockDetailSection
        label="Generation"
        contentClassName={hasGenSoftware ? undefined : styles.blockFourColumnGrid}
      >
        <BlockDetailItem
          label="Gen utime"
          value={
            genUtime === undefined ? (
              "Unknown"
            ) : (
              <>
                <DateTime display="date-time-numeric-seconds" unit="seconds" value={genUtime} />{" "}
                <span className={styles.blockDetailSecondaryValue}>
                  (<RelativeTime mode="relative" tooltip={false} unit="seconds" value={genUtime} />)
                </span>
              </>
            )
          }
          visualPlaceholder="<time>"
        />
        <BlockDetailItem label="Version" value={formatOptionalNumber(block.version)} mono />
        <BlockDetailItem label="Vert seqno" value={formatOptionalNumber(block.vert_seqno)} mono />
        <BlockDetailItem
          label="Gen catchain seqno"
          value={formatOptionalNumber(block.gen_catchain_seqno)}
          mono
        />
        {block.gen_software_version === undefined ? null : (
          <BlockDetailItem
            label="Gen software version"
            value={
              <a
                className={styles.blockDetailDocLink}
                href={`https://github.com/ton-blockchain/ton/blob/master/doc/GlobalVersions.md#version-${block.gen_software_version}`}
                target="_blank"
                rel="noreferrer"
              >
                {block.gen_software_version}
                <ExternalLink size={11} aria-hidden="true" />
              </a>
            }
            mono
          />
        )}
        {block.gen_software_capabilities === undefined ? null : (
          <BlockDetailItem
            label="Capabilities"
            value={<GlobalCapabilities value={block.gen_software_capabilities} />}
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="References">
        {block.workchain !== -1 && masterchainBlockRef ? (
          <BlockDetailItem
            label="Masterchain block"
            value={
              <BlockChip
                workchain={masterchainBlockRef.workchain}
                shard={masterchainBlockRef.shard}
                seqno={masterchainBlockRef.seqno}
                href={routes.blockPath(
                  masterchainBlockRef.workchain,
                  masterchainBlockRef.shard,
                  masterchainBlockRef.seqno,
                )}
                onClick={event => onOpenBlock(masterchainBlockRef, event)}
              />
            }
          />
        ) : null}
        <BlockDetailItem
          label="Prev refs"
          value={
            block.prev_blocks && block.prev_blocks.length > 0 ? (
              <span className={styles.blockReferenceList}>
                {block.prev_blocks.map(ref => (
                  <BlockChip
                    key={formatToncenterBlockId(ref)}
                    workchain={ref.workchain}
                    shard={ref.shard}
                    seqno={ref.seqno}
                    display="full"
                    href={routes.blockPath(ref.workchain, ref.shard, ref.seqno)}
                    onClick={event => onOpenBlock(ref, event)}
                  />
                ))}
              </span>
            ) : (
              "None"
            )
          }
        />
        <BlockDetailItem
          label="Prev key block seqno"
          value={
            prevKeyBlockSeqno && prevKeyBlockSeqno > 0 ? (
              <BlockChip
                workchain={-1}
                shard={masterchainShard}
                seqno={prevKeyBlockSeqno}
                href={routes.blockPath(-1, masterchainShard, prevKeyBlockSeqno)}
                onClick={event =>
                  onOpenBlock(
                    {
                      workchain: -1,
                      shard: masterchainShard,
                      seqno: prevKeyBlockSeqno,
                    },
                    event,
                  )
                }
              />
            ) : (
              formatOptionalNumber(prevKeyBlockSeqno)
            )
          }
        />
        <BlockDetailItem
          label="Min ref mc seqno"
          value={
            minRefMcSeqno && minRefMcSeqno > 0 ? (
              <BlockChip
                workchain={-1}
                shard={masterchainShard}
                seqno={minRefMcSeqno}
                href={routes.blockPath(-1, masterchainShard, minRefMcSeqno)}
                onClick={event =>
                  onOpenBlock(
                    {
                      workchain: -1,
                      shard: masterchainShard,
                      seqno: minRefMcSeqno,
                    },
                    event,
                  )
                }
              />
            ) : (
              formatOptionalNumber(minRefMcSeqno)
            )
          }
        />
      </BlockDetailSection>

      <BlockDetailSection label="Activity">
        <BlockDetailItem label="Tx quantity" value={<NumberValue value={block.tx_count} />} mono />
        {block.fees_collected === undefined ? null : (
          <BlockDetailItem
            label="Fees collected"
            value={<GramAmount value={block.fees_collected} useGrouping />}
          />
        )}
        {block.in_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="In msg descr length"
            value={<NumberValue value={block.in_msg_descr_length} />}
            mono
          />
        )}
        {block.out_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="Out msg descr length"
            value={<NumberValue value={block.out_msg_descr_length} />}
            mono
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="Flags" contentClassName={styles.blockSixColumnGrid}>
        <BlockDetailItem
          label="Key block"
          value={<BooleanValue display="true-false" value={block.key_block} />}
        />
        <BlockDetailItem
          label="After merge"
          value={<BooleanValue display="true-false" value={block.after_merge} />}
        />
        <BlockDetailItem
          label="After split"
          value={<BooleanValue display="true-false" value={block.after_split} />}
        />
        <BlockDetailItem
          label="Before split"
          value={<BooleanValue display="true-false" value={block.before_split} />}
        />
        <BlockDetailItem
          label="Want merge"
          value={<BooleanValue display="true-false" value={block.want_merge} />}
        />
        <BlockDetailItem
          label="Want split"
          value={<BooleanValue display="true-false" value={block.want_split} />}
        />
      </BlockDetailSection>

      <BlockDetailSection label="Logical time">
        <BlockDetailItem label="Start LT / End LT" value={`${block.start_lt} – ${block.end_lt}`} />
      </BlockDetailSection>
    </section>
  )
}

const BlockDetailSection: FC<{
  readonly label: string
  readonly children: ReactNode
  readonly contentClassName?: string
}> = ({label, children, contentClassName}) => (
  <div className={styles.blockDetailRow}>
    <div className={styles.blockDetailLabel}>{label}</div>
    <div className={`${styles.blockDetailGrid} ${contentClassName ?? ""}`}>{children}</div>
  </div>
)

interface BlockDetailItemProps {
  readonly label: string
  readonly value: ReactNode
  readonly title?: string
  readonly copyValue?: string
  readonly mono?: boolean
  readonly visualPlaceholder?: string
}

const BlockDetailItem: FC<BlockDetailItemProps> = ({
  label,
  value,
  title,
  copyValue,
  mono = false,
  visualPlaceholder,
}) => (
  <div className={styles.blockDetailItem}>
    <span className={styles.blockDetailItemLabel}>{label}</span>
    <span
      className={`${styles.blockDetailValue} ${mono ? styles.blocksMonoCell : ""}`}
      title={title ?? (typeof value === "string" ? value : undefined)}
      data-visual-dynamic={visualPlaceholder ? "time" : undefined}
      data-visual-placeholder={visualPlaceholder}
    >
      {copyValue ? (
        <InlineActions
          className={styles.blockDetailInlineActions}
          visibility="hover"
          actions={
            <CopyInlineAction
              value={copyValue}
              size="compact"
              label={`Copy ${label.toLowerCase()}`}
              copiedLabel={`${label} copied`}
            />
          }
        >
          <span className={styles.blockDetailValueText}>{value}</span>
        </InlineActions>
      ) : (
        value
      )}
    </span>
  </div>
)

const BlockDetailSkeletonItem: FC<{
  readonly label: string
  readonly valueClassName?: string
  readonly wide?: boolean
}> = ({label, valueClassName, wide = false}) => (
  <div className={styles.blockDetailItem}>
    <span className={styles.blockDetailItemLabel}>{label}</span>
    <span
      className={`${styles.skeletonLine} ${wide ? styles.blockDetailSkeletonValueWide : styles.blockDetailSkeletonValue} ${valueClassName ?? ""}`}
    />
  </div>
)

const BlockDetailsSkeleton: FC<{
  readonly showShardchainBlocks: boolean
  readonly showRawBlockFields: boolean
}> = ({showShardchainBlocks, showRawBlockFields}) => (
  <>
    <section className={styles.blockDetailsPanel} aria-label="Loading block details">
      <BlockDetailSection label="Identity" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailSkeletonItem label="Workchain" />
        <BlockDetailSkeletonItem label="Shard" />
        <BlockDetailSkeletonItem label="Seqno" />
        <BlockDetailSkeletonItem label="Global ID" />
      </BlockDetailSection>

      <BlockDetailSection label="Hashes" contentClassName={styles.blockHashesGrid}>
        <BlockDetailSkeletonItem
          label="Root hash"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="File hash"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Created by"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Rand seed"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
      </BlockDetailSection>

      <BlockDetailSection
        label="Generation"
        contentClassName={showRawBlockFields ? undefined : styles.blockFourColumnGrid}
      >
        <BlockDetailSkeletonItem label="Gen utime" />
        <BlockDetailSkeletonItem label="Version" />
        <BlockDetailSkeletonItem label="Vert seqno" />
        <BlockDetailSkeletonItem label="Gen catchain seqno" />
        {showRawBlockFields ? (
          <>
            <BlockDetailSkeletonItem label="Gen software version" />
            <BlockDetailSkeletonItem label="Capabilities" />
          </>
        ) : null}
      </BlockDetailSection>

      <BlockDetailSection label="References">
        <BlockDetailSkeletonItem
          label="Prev refs"
          valueClassName={styles.blockDetailSkeletonChipValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Prev key block seqno"
          valueClassName={styles.blockDetailSkeletonChipValue}
        />
        <BlockDetailSkeletonItem
          label="Min ref mc seqno"
          valueClassName={styles.blockDetailSkeletonChipValue}
        />
      </BlockDetailSection>

      <BlockDetailSection label="Activity">
        <BlockDetailSkeletonItem label="Tx quantity" />
        {showRawBlockFields ? <BlockDetailSkeletonItem label="Fees collected" /> : null}
      </BlockDetailSection>

      <BlockDetailSection label="Flags" contentClassName={styles.blockSixColumnGrid}>
        <BlockDetailSkeletonItem label="Key block" />
        <BlockDetailSkeletonItem label="After merge" />
        <BlockDetailSkeletonItem label="After split" />
        <BlockDetailSkeletonItem label="Before split" />
        <BlockDetailSkeletonItem label="Want merge" />
        <BlockDetailSkeletonItem label="Want split" />
      </BlockDetailSection>

      <BlockDetailSection label="Logical time">
        <BlockDetailSkeletonItem label="Start LT / End LT" wide />
      </BlockDetailSection>
    </section>
    {showShardchainBlocks ? (
      <BlockTableSkeleton title="Last shard blocks" rows={1} showShardFlags />
    ) : null}
    <BlockTransactionsTableSkeleton rows={4} />
  </>
)

const TableStateBlock: FC<{
  readonly title?: string
  readonly children: ReactNode
}> = ({title, children}) => (
  <section className={styles.blocksTableFrame}>
    {title ? <header className={styles.blocksTableTitle}>{title}</header> : null}
    <div className={styles.blocksTableState}>{children}</div>
  </section>
)

function isSameBlock(block: V3Block, workchain: number, shard: string, seqno: number): boolean {
  return block.workchain === workchain && block.shard === shard && block.seqno === seqno
}

function formatBlockHash(value: string): string {
  return hashToHex(value) ?? value
}

function formatOptionalNumber(value: string | number | undefined): string {
  return value === undefined ? "—" : value.toString()
}

function getExtendedBlockId(block: V3Block): string {
  const rootHash = formatBlockHash(block.root_hash)
  const fileHash = formatBlockHash(block.file_hash)
  return `(${block.workchain},${block.shard},${block.seqno},${rootHash},${fileHash})`
}

function getBlockActions(
  block: V3Block,
  rawBlockNetwork: RawBlockNetwork | undefined,
): {
  readonly downloadUrl?: string
  readonly configSeqno?: number
  readonly tonscanUrl: string
  readonly toncoinUrl: string
  readonly extendedBlockId: string
} {
  const blockId = formatToncenterBlockId(block)
  const tonapiOrigin =
    rawBlockNetwork === "testnet" ? "https://testnet.tonapi.io" : "https://tonapi.io"
  const tonscanOrigin =
    rawBlockNetwork === "testnet" ? "https://testnet.tonscan.org" : "https://tonscan.org"
  const toncoinOrigin =
    rawBlockNetwork === "testnet"
      ? "https://test-explorer.toncoin.org"
      : "https://explorer.toncoin.org"
  return {
    downloadUrl: rawBlockNetwork
      ? `${tonapiOrigin}/v2/blockchain/blocks/${encodeURIComponent(blockId)}/boc`
      : undefined,
    configSeqno: getConfigSeqno(block),
    tonscanUrl: `${tonscanOrigin}/block/${block.workchain}:${block.shard}:${block.seqno}`,
    toncoinUrl: `${toncoinOrigin}/search?workchain=${block.workchain}&shard=${encodeURIComponent(block.shard)}&seqno=${block.seqno}`,
    extendedBlockId: getExtendedBlockId(block),
  }
}

function getConfigSeqno(block: V3Block): number | undefined {
  if (block.workchain === -1) return block.seqno
  return block.master_ref_seqno ?? block.masterchain_block_ref?.seqno
}

function blockUnixTime(block: V3Block): number | undefined {
  const value = Number(block.gen_utime)
  return Number.isFinite(value) && value > 0 ? value : undefined
}

function formatTransactionExitCode(transaction: BlockTransactionListItem): string {
  if (!("description" in transaction)) {
    return "Unknown"
  }

  const computeExitCode = transaction.description.compute_ph?.exit_code
  if (typeof computeExitCode === "number") {
    return computeExitCode.toString()
  }
  const resultCode = transaction.description.action?.result_code
  return typeof resultCode === "number" ? resultCode.toString() : "Unknown"
}
