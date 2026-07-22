import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  ChevronsRight,
  Download,
  ExternalLink,
  FileJson,
} from "lucide-react"
import {useNavigate, useParams} from "react-router-dom"
import {
  BlockChip,
  Button,
  CopyButton,
  CopyInlineAction,
  InlineActions,
  Input,
  ModeViewer,
  Popover,
  formatToncenterBlockId,
  type ModeInfo,
  type ModeParser,
} from "@acton/ui"
import {useEffect, useMemo, useState} from "react"
import type {FC, FormEvent, ReactNode} from "react"

import type {RawBlockNetwork, TonClient} from "../api/client"
import type {V3Block, V3BlockId, V3TransactionListItem} from "../api/types"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../components/DeveloperTransactionList"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {formatNano, formatRelativeTime, hashToHex} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../hooks/useTransactionMessageNames"

import styles from "./BlocksPage.module.css"

const BLOCKS_PAGE_LIMIT = 8
const LAST_TRANSACTION_MESSAGES_LIMIT = 5
const LAST_TRANSACTIONS_FETCH_LIMIT = 12
const BLOCK_TRANSACTIONS_LIMIT = 100
const BLOCKS_REFRESH_MS = 2000
const MASTERCHAIN_SHARD = "8000000000000000"
const MIN_BLOCK_UNIX_TIME = 0
const GLOBAL_CAPABILITIES = [
  {
    value: 1,
    name: "capIhrEnabled",
    description: "Enables Instant Hypercube Routing.",
  },
  {
    value: 2,
    name: "capCreateStatsEnabled",
    description: "Enables creation statistics in the masterchain state.",
  },
  {
    value: 4,
    name: "capBounceMsgBody",
    description: "Allows bounced messages to retain part of the original message body.",
  },
  {
    value: 8,
    name: "capReportVersion",
    description: "Makes collators report their supported version and capabilities in blocks.",
  },
  {
    value: 16,
    name: "capSplitMergeTransactions",
    description: "Enables shard split and merge transactions.",
  },
  {
    value: 32,
    name: "capShortDequeue",
    description: "Enables short dequeue records in block message queues.",
  },
  {
    value: 64,
    name: "capStoreOutMsgQueueSize",
    description: "Stores the outgoing message queue size in the shard state.",
  },
  {
    value: 128,
    name: "capMsgMetadata",
    description: "Enables transaction-chain metadata in message envelopes.",
  },
  {
    value: 256,
    name: "capDeferMessages",
    description: "Enables deferred message processing through dispatch queues.",
  },
  {
    value: 512,
    name: "capFullCollatedData",
    description: "Enables full collated data for block validation.",
  },
] as const

const parseGlobalCapabilities: ModeParser = mode => {
  const flags: ModeInfo[] = GLOBAL_CAPABILITIES.filter(
    capability => Math.floor(mode / capability.value) % 2 === 1,
  ).map(capability => ({...capability}))
  const knownValue = flags.reduce((sum, flag) => sum + flag.value, 0)
  const unknownValue = mode - knownValue

  if (unknownValue > 0) {
    flags.push({
      value: unknownValue,
      name: "Unknown capabilities",
      description: "Capability bits that are not known to this explorer version.",
    })
  } else if (mode === 0) {
    flags.push({
      value: 0,
      name: "No capabilities",
      description: "This block does not report any enabled global capabilities.",
    })
  }

  return flags
}

interface BlocksPageProps {
  readonly client: TonClient
}

interface BlockDetailsPageProps extends BlocksPageProps {
  readonly latest?: boolean
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
  readonly transactions: readonly V3TransactionListItem[]
  readonly isLoading: boolean
  readonly error?: string
}

export const BlocksPage: FC<BlocksPageProps> = ({client}) => {
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
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
          <BlockTableSection
            title="Last workchain blocks"
            blocks={state.workchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No workchain blocks yet"
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
        </div>
      </section>
    </div>
  )
}

export const BlockDetailsPage: FC<BlockDetailsPageProps> = ({client, latest = false}) => {
  const params = useParams<{
    workchain: string
    shard: string
    seqno: string
  }>()
  const navigate = useNavigate()
  const {network} = useNetworkInfo()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames, updateDomains} = useAddressBook()
  const rawBlockNetwork: RawBlockNetwork | undefined =
    network.id === "mainnet" || network.id === "testnet" ? network.id : undefined
  const routeWorkchain = Number(params.workchain)
  const routeShard = params.shard ?? ""
  const routeSeqno = Number(params.seqno)
  const [state, setState] = useState<BlockDetailsState>({
    shardchainBlocks: [],
    transactions: [],
    isLoading: true,
  })

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
          error: "Invalid block route.",
        })
        return
      }

      setState(current => ({
        ...current,
        isLoading: true,
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
              error: "Block not found.",
            })
          }
          return
        }

        const rawBlockMetadataPromise =
          rawBlockNetwork &&
          (block.gen_software_version === undefined ||
            block.gen_software_capabilities === undefined ||
            block.fees_collected === undefined)
            ? client
                .getRawBlockBoc(getExtendedBlockId(block), rawBlockNetwork)
                .then(async cell => {
                  const {parseBlockMetadata} = await import("../cell-inspector/blockParser")
                  return parseBlockMetadata(cell)
                })
                .catch(() => undefined)
            : Promise.resolve(undefined)
        const [transactionsResponse, shardchainResponse, rawBlockMetadata] = await Promise.all([
          client.getBlockTransactions({
            workchain: block.workchain,
            shard: block.shard,
            seqno: block.seqno,
            limit: BLOCK_TRANSACTIONS_LIMIT,
          }),
          block.workchain === -1
            ? client.getMasterchainBlockShards(block.seqno)
            : Promise.resolve({blocks: []}),
          rawBlockMetadataPromise,
        ])

        if (!isActive) {
          return
        }

        updateDomains(transactionsResponse.address_book)
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
          isLoading: false,
        })
      } catch (error) {
        if (!isActive) {
          return
        }
        setState(current => ({
          ...current,
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load block",
        }))
      }
    }

    void loadBlockDetails()

    return () => {
      isActive = false
    }
  }, [client, latest, rawBlockNetwork, routeShard, routeSeqno, routeWorkchain, updateDomains])

  const workchain = latest ? (state.block?.workchain ?? -1) : routeWorkchain
  const shard = latest ? (state.block?.shard ?? MASTERCHAIN_SHARD) : routeShard
  const seqno = latest ? (state.block?.seqno ?? Number.NaN) : routeSeqno

  const title = workchain === -1 ? "Masterchain block" : "Workchain block"
  const hasResolvedBlockId =
    Number.isInteger(workchain) && Number.isInteger(seqno) && Boolean(shard)
  const hasValidRoute = latest || hasResolvedBlockId
  const blockId = hasResolvedBlockId ? formatToncenterBlockId({workchain, shard, seqno}) : undefined
  const latestPath = state.latestBlock ? blockPath(state.latestBlock) : undefined
  const canOpenPrev = hasResolvedBlockId && seqno > 1
  const prevPath = canOpenPrev ? blockPath({workchain, shard, seqno: seqno - 1}) : undefined
  const nextPath = hasResolvedBlockId ? blockPath({workchain, shard, seqno: seqno + 1}) : undefined
  const transactionAddresses = useMemo(
    () => state.transactions.map(transaction => transaction.account),
    [state.transactions],
  )
  const blockActions = state.block ? getBlockActions(state.block, rawBlockNetwork) : undefined

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
        <div>
          <h1 className={styles.title}>{title}</h1>
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
                onOpenBlock={block => void navigate(blockPath(block))}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronsRight size={14} />}
                disabled={
                  !latestPath ||
                  (state.block !== undefined && latestPath === blockPath(state.block))
                }
                onClick={() => latestPath && void navigate(latestPath)}
              >
                Latest
              </Button>
            </div>

            {state.block && blockActions ? (
              <div className={styles.blockHeaderActions} aria-label="Block actions">
                {blockActions.downloadUrl ? (
                  <a
                    className={styles.blockActionLink}
                    href={blockActions.downloadUrl}
                    target="_blank"
                    rel="noreferrer"
                  >
                    <Download size={15} aria-hidden="true" />
                    Download
                  </a>
                ) : null}
                {rawBlockNetwork ? (
                  blockActions.configUrl ? (
                    <a
                      className={styles.blockActionLink}
                      href={blockActions.configUrl}
                      target="_blank"
                      rel="noreferrer"
                    >
                      <FileJson size={15} aria-hidden="true" />
                      Config
                    </a>
                  ) : (
                    <span className={`${styles.blockActionLink} ${styles.blockActionLinkDisabled}`}>
                      <FileJson size={15} aria-hidden="true" />
                      Config
                    </span>
                  )
                ) : null}
                <CopyButton
                  value={blockActions.extendedBlockId}
                  label="Copy extended block ID"
                  copiedLabel="Extended block ID copied"
                  copiedChildren="Copied ID"
                  variant="outline"
                  size="sm"
                >
                  Copy block ID
                </CopyButton>
                {rawBlockNetwork ? (
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
            showRawBlockFields={rawBlockNetwork !== undefined}
          />
        ) : (
          <>
            <BlockSummaryTable
              block={state.block}
              onOpenBlock={(block, event) => openPath(blockPath(block), event)}
            />

            {state.block.workchain === -1 ? (
              <BlockTableSection
                title="Last shard blocks"
                blocks={state.shardchainBlocks}
                isLoading={false}
                emptyLabel="No shardchain blocks for this masterchain block"
                showShardFlags
                onOpenBlock={(block, event) => openPath(blockPath(block), event)}
              />
            ) : null}

            <BlockTransactionsTable
              transactions={state.transactions}
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
  readonly isLoading: boolean
  readonly emptyLabel: string
  readonly showShardFlags?: boolean
  readonly onOpenBlock: (block: V3Block, event?: ExplorerNavigationClickEvent) => void
}> = ({title, blocks, isLoading, emptyLabel, showShardFlags = false, onOpenBlock}) => {
  if (isLoading) {
    return <BlockTableSkeleton title={title} rows={4} showShardFlags={showShardFlags} />
  }

  if (blocks.length === 0) {
    return <TableStateBlock title={title}>{emptyLabel}</TableStateBlock>
  }

  return (
    <section className={styles.blocksTableFrame} aria-label={title}>
      <header className={styles.blocksTableTitle}>{title}</header>
      <div className={styles.blocksTableScroller}>
        <table className={`${styles.blocksTable} ${showShardFlags ? styles.shardBlocksTable : ""}`}>
          <thead>
            <tr>
              <th>Block</th>
              <th>Transactions</th>
              <th>Generated at</th>
              {showShardFlags ? (
                <>
                  <th>Before split</th>
                  <th>After split</th>
                  <th>Want split</th>
                  <th>Want merge</th>
                </>
              ) : null}
            </tr>
          </thead>
          <tbody>
            {blocks.map(block => (
              <tr
                key={formatToncenterBlockId(block)}
                className={styles.blocksTableRow}
                tabIndex={0}
                onClick={event => onOpenBlock(block, event)}
                onKeyDown={event => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    onOpenBlock(block)
                  }
                }}
              >
                <td className={styles.blocksPrimaryCell}>
                  <BlockChip
                    workchain={block.workchain}
                    shard={block.shard}
                    seqno={block.seqno}
                    href={blockPath(block)}
                    onClick={event => {
                      event.stopPropagation()
                      onOpenBlock(block, event)
                    }}
                  />
                </td>
                <td>{block.tx_count.toLocaleString()}</td>
                <td
                  title={formatAbsoluteBlockTime(block)}
                  data-visual-dynamic="time"
                  data-visual-placeholder="<time>"
                >
                  {formatAbsoluteBlockTime(block)}
                </td>
                {showShardFlags ? (
                  <>
                    <td>
                      <BooleanValue value={block.before_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.after_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.want_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.want_merge} />
                    </td>
                  </>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

const BlockTransactionsTable: FC<{
  readonly transactions: readonly V3TransactionListItem[]
  readonly onOpenAccount: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onOpenTransaction: (hash: string, event?: ExplorerNavigationClickEvent) => void
}> = ({transactions, onOpenAccount, onOpenTransaction}) => {
  if (transactions.length === 0) {
    return <TableStateBlock title="Transactions">No transactions in this block</TableStateBlock>
  }

  return (
    <section className={styles.blocksTableFrame} aria-label="Transactions">
      <header className={styles.blocksTableTitle}>Transactions</header>
      <div className={styles.blocksTableScroller}>
        <table className={`${styles.blocksTable} ${styles.blockTransactionsTable}`}>
          <thead>
            <tr>
              <th>#</th>
              <th>Account</th>
              <th>Logical time</th>
              <th>Hash</th>
              <th>Exit code</th>
            </tr>
          </thead>
          <tbody>
            {transactions.map((transaction, index) => {
              const hash = hashToHex(transaction.hash) ?? transaction.hash
              const exitCode = formatTransactionExitCode(transaction)
              const hasNonZeroExitCode = exitCode !== "0" && exitCode !== "Unknown"
              return (
                <tr
                  key={`${transaction.hash}:${transaction.lt}`}
                  className={styles.blocksTableRow}
                  tabIndex={0}
                  onClick={event => onOpenTransaction(transaction.hash, event)}
                  onKeyDown={event => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault()
                      onOpenTransaction(transaction.hash)
                    }
                  }}
                >
                  <td>{index + 1}</td>
                  <td>
                    <ExplorerAddressChip
                      address={transaction.account}
                      fallback="Account"
                      onAddressClick={onOpenAccount}
                    />
                  </td>
                  <td>{transaction.lt}</td>
                  <td>
                    <span className={styles.blocksHashCell}>
                      <span className={styles.blocksHashText} title={hash}>
                        {compactMiddle(hash, 18)}
                      </span>
                      <CopyInlineAction
                        value={hash}
                        size="compact"
                        label="Copy transaction hash"
                        copiedLabel="Transaction hash copied"
                      />
                    </span>
                  </td>
                  <td
                    className={`${styles.blocksExitCodeCell} ${
                      hasNonZeroExitCode ? styles.blocksExitCodeFailure : ""
                    }`}
                  >
                    {exitCode}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </section>
  )
}

const BlockTableSkeleton: FC<{
  readonly title: string
  readonly rows: number
  readonly showShardFlags?: boolean
}> = ({title, rows, showShardFlags = false}) => (
  <section className={styles.blocksTableFrame} aria-label={`Loading ${title}`}>
    <header className={styles.blocksTableTitle}>{title}</header>
    <div className={styles.blocksTableScroller}>
      <table className={`${styles.blocksTable} ${showShardFlags ? styles.shardBlocksTable : ""}`}>
        <thead>
          <tr>
            <th>Block</th>
            <th>Transactions</th>
            <th>Generated at</th>
            {showShardFlags ? (
              <>
                <th>Before split</th>
                <th>After split</th>
                <th>Want split</th>
                <th>Want merge</th>
              </>
            ) : null}
          </tr>
        </thead>
        <tbody>
          {Array.from({length: rows}, (_, index) => (
            <tr key={`block-table-skeleton-${index}`}>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonBlock}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonCount}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonTime}`} />
              </td>
              {showShardFlags
                ? Array.from({length: 4}, (_, flagIndex) => (
                    <td key={`block-table-skeleton-${index}-flag-${flagIndex}`}>
                      <span className={`${styles.skeletonLine} ${styles.blocksSkeletonFlag}`} />
                    </td>
                  ))
                : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  </section>
)

const BlockTransactionsTableSkeleton: FC<{readonly rows: number}> = ({rows}) => (
  <section className={styles.blocksTableFrame} aria-label="Loading transactions">
    <header className={styles.blocksTableTitle}>Transactions</header>
    <div className={styles.blocksTableScroller}>
      <table className={`${styles.blocksTable} ${styles.blockTransactionsTable}`}>
        <thead>
          <tr>
            <th>#</th>
            <th>Account</th>
            <th>Logical time</th>
            <th>Hash</th>
            <th>Exit code</th>
          </tr>
        </thead>
        <tbody>
          {Array.from({length: rows}, (_, index) => (
            <tr key={`block-transaction-skeleton-${index}`}>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonIndex}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonAccount}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonLt}`} />
              </td>
              <td>
                <span className={styles.blocksSkeletonHashCell}>
                  <span className={`${styles.skeletonLine} ${styles.blocksSkeletonHash}`} />
                  <span className={`${styles.skeletonLine} ${styles.blocksSkeletonCopy}`} />
                </span>
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonExitCode}`} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  </section>
)

const BlockSummaryTable: FC<{
  readonly block: V3Block
  readonly onOpenBlock: (block: V3BlockId, event: ExplorerNavigationClickEvent) => void
}> = ({block, onOpenBlock}) => {
  const rootHash = formatBlockHash(block.root_hash)
  const fileHash = formatBlockHash(block.file_hash)
  const createdBy = formatBlockHash(block.created_by)
  const randSeed = formatBlockHash(block.rand_seed)
  const masterchainShard = block.masterchain_block_ref?.shard ?? MASTERCHAIN_SHARD
  const prevKeyBlockSeqno = block.prev_key_block_seqno
  const minRefMcSeqno = block.min_ref_mc_seqno
  const genUtime = blockUnixTime(block)
  const absoluteGenTime = formatAbsoluteBlockTime(block)
  const hasGenSoftware =
    block.gen_software_version !== undefined || block.gen_software_capabilities !== undefined

  return (
    <section className={styles.blockDetailsPanel} aria-label="Block details">
      <BlockDetailSection label="Identity" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailItem label="Workchain" value={block.workchain.toString()} />
        <BlockDetailItem label="Shard" value={block.shard} mono />
        <BlockDetailItem label="Seqno" value={block.seqno.toString()} mono />
        <BlockDetailItem label="Global ID" value={formatOptionalNumber(block.global_id)} mono />
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
              absoluteGenTime
            ) : (
              <>
                {absoluteGenTime}{" "}
                <span className={styles.blockDetailRelativeTime}>
                  ({formatRelativeTime(genUtime)})
                </span>
              </>
            )
          }
          title={absoluteGenTime}
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
            value={<BlockCapabilities value={block.gen_software_capabilities} />}
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="References">
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
                    href={blockPath(ref)}
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
                href={blockPath({
                  workchain: -1,
                  shard: masterchainShard,
                  seqno: prevKeyBlockSeqno,
                })}
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
                href={blockPath({
                  workchain: -1,
                  shard: masterchainShard,
                  seqno: minRefMcSeqno,
                })}
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
        <BlockDetailItem label="Tx quantity" value={block.tx_count.toLocaleString()} mono />
        {block.fees_collected === undefined ? null : (
          <BlockDetailItem
            label="Fees collected"
            value={`${formatNano(block.fees_collected)} GRAM`}
          />
        )}
        {block.in_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="In msg descr length"
            value={block.in_msg_descr_length.toLocaleString()}
            mono
          />
        )}
        {block.out_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="Out msg descr length"
            value={block.out_msg_descr_length.toLocaleString()}
            mono
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="Flags" contentClassName={styles.blockSixColumnGrid}>
        <BlockDetailItem label="Key block" value={<BooleanValue value={block.key_block} />} />
        <BlockDetailItem label="After merge" value={<BooleanValue value={block.after_merge} />} />
        <BlockDetailItem label="After split" value={<BooleanValue value={block.after_split} />} />
        <BlockDetailItem label="Before split" value={<BooleanValue value={block.before_split} />} />
        <BlockDetailItem label="Want merge" value={<BooleanValue value={block.want_merge} />} />
        <BlockDetailItem label="Want split" value={<BooleanValue value={block.want_split} />} />
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

const BooleanValue: FC<{readonly value: boolean | undefined}> = ({value}) => {
  if (value === undefined) {
    return <>—</>
  }
  return (
    <span className={value ? styles.blockBooleanTrue : styles.blockBooleanFalse}>
      {value ? "true" : "false"}
    </span>
  )
}

const BlockCapabilities: FC<{readonly value: string | number}> = ({value}) => {
  const mode = Number(value)
  if (!Number.isSafeInteger(mode) || mode < 0) {
    return <>{value}</>
  }

  return (
    <Popover
      ariaLabel={`Explain global capabilities ${value}`}
      content={
        <span className={styles.blockCapabilitiesPopover}>
          <span className={styles.blockCapabilitiesPopoverTitle}>Enabled capabilities</span>
          <ModeViewer mode={mode} parseMode={parseGlobalCapabilities} />
        </span>
      }
      interaction="click"
      placement="top"
      maxWidth="min(42rem, calc(100vw - 2rem))"
    >
      <button type="button" className={styles.blockCapabilitiesTrigger}>
        {value}
      </button>
    </Popover>
  )
}

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

function blockPath(block: Pick<V3Block, "workchain" | "shard" | "seqno">): string {
  return `/block/${block.workchain}/${encodeURIComponent(block.shard)}/${block.seqno}`
}

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
  readonly configUrl?: string
  readonly tonscanUrl: string
  readonly toncoinUrl: string
  readonly extendedBlockId: string
} {
  const blockId = formatToncenterBlockId(block)
  const tonapiOrigin =
    rawBlockNetwork === "testnet" ? "https://testnet.tonapi.io" : "https://tonapi.io"
  const tonviewerOrigin =
    rawBlockNetwork === "testnet" ? "https://testnet.tonviewer.com" : "https://tonviewer.com"
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
    configUrl:
      rawBlockNetwork && block.prev_key_block_seqno && block.prev_key_block_seqno > 0
        ? `${tonviewerOrigin}/config/${block.prev_key_block_seqno}`
        : undefined,
    tonscanUrl: `${tonscanOrigin}/block/${block.workchain}:${block.shard}:${block.seqno}`,
    toncoinUrl: `${toncoinOrigin}/search?workchain=${block.workchain}&shard=${encodeURIComponent(block.shard)}&seqno=${block.seqno}`,
    extendedBlockId: getExtendedBlockId(block),
  }
}

function blockUnixTime(block: V3Block): number | undefined {
  const value = Number(block.gen_utime)
  return Number.isFinite(value) && value > 0 ? value : undefined
}

function formatDateTimeLocalInput(unixTime: number): string {
  const date = new Date(unixTime * 1000)
  const localDate = new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
  return localDate.toISOString().slice(0, 19)
}

function formatAbsoluteBlockTime(block: V3Block): string {
  const unixTime = blockUnixTime(block)
  if (unixTime === undefined) {
    return "Unknown"
  }

  const date = new Date(unixTime * 1000)
  const day = date.getDate().toString().padStart(2, "0")
  const month = (date.getMonth() + 1).toString().padStart(2, "0")
  const hours = date.getHours().toString().padStart(2, "0")
  const minutes = date.getMinutes().toString().padStart(2, "0")
  const seconds = date.getSeconds().toString().padStart(2, "0")
  return `${day}.${month}.${date.getFullYear()}, ${hours}:${minutes}:${seconds}`
}

function formatTransactionExitCode(transaction: V3TransactionListItem): string {
  const computeExitCode = transaction.description.compute_ph?.exit_code
  if (typeof computeExitCode === "number") {
    return computeExitCode.toString()
  }
  const resultCode = transaction.description.action?.result_code
  return typeof resultCode === "number" ? resultCode.toString() : "Unknown"
}

function compactMiddle(value: string, visibleChars: number): string {
  if (value.length <= visibleChars + 3) {
    return value
  }

  const side = Math.max(4, Math.floor(visibleChars / 2))
  return `${value.slice(0, side)}…${value.slice(-side)}`
}
