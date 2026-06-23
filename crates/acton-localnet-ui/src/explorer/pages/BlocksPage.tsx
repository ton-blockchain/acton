import {Check, ChevronLeft, ChevronRight, ChevronsRight, Copy} from "lucide-react"
import {Link, useNavigate, useParams} from "react-router-dom"
import {Button} from "@acton/shared-ui"
import {useEffect, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"

import type {TonClient} from "../api/client"
import type {V3Block, V3TransactionListItem} from "../api/types"
import {AddressLabel} from "../components/AddressLabel"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../components/DeveloperTransactionList"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useOpenExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../hooks/useTransactionMessageNames"

import styles from "./BlocksPage.module.css"

const BLOCKS_PAGE_LIMIT = 8
const LAST_TRANSACTION_MESSAGES_LIMIT = 5
const LAST_TRANSACTIONS_FETCH_LIMIT = 12
const BLOCK_TRANSACTIONS_LIMIT = 100
const BLOCKS_REFRESH_MS = 2000
const BLOCK_SUMMARY_LABELS = [
  "Workchain",
  "Shard",
  "Seqno",
  "LT",
  "Generated at",
  "Root hash",
  "File hash",
  "Tx quantity",
  "Prev refs",
] as const

interface BlocksPageProps {
  readonly client: TonClient
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
  const {prefetchNames} = useAddressBook()
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
        setState(current => ({...current, isLoading: true, error: undefined}))
      }
      try {
        const [transactions, masterchainBlocks, workchainBlocks] = await Promise.all([
          client.getRecentTransactions(LAST_TRANSACTIONS_FETCH_LIMIT),
          client.getBlocks({workchain: -1, limit: BLOCKS_PAGE_LIMIT, sort: "desc"}),
          client.getBlocks({workchain: 0, limit: BLOCKS_PAGE_LIMIT, sort: "desc"}),
        ])

        if (!isActive) {
          return
        }

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
  }, [client])

  return (
    <div className={styles.container}>
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
            emptyLabel="No masterchain blocks yet."
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
          <BlockTableSection
            title="Last workchain blocks"
            blocks={state.workchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No workchain blocks yet."
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
        </div>
      </section>
    </div>
  )
}

export const BlockDetailsPage: FC<BlocksPageProps> = ({client}) => {
  const params = useParams<{workchain: string; shard: string; seqno: string}>()
  const navigate = useNavigate()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames} = useAddressBook()
  const workchain = Number(params.workchain)
  const shard = params.shard ?? ""
  const seqno = Number(params.seqno)
  const [state, setState] = useState<BlockDetailsState>({
    shardchainBlocks: [],
    transactions: [],
    isLoading: true,
  })

  useEffect(() => {
    let isActive = true

    const loadBlockDetails = async () => {
      if (!Number.isInteger(workchain) || !Number.isInteger(seqno) || !shard) {
        setState({
          shardchainBlocks: [],
          transactions: [],
          isLoading: false,
          error: "Invalid block route.",
        })
        return
      }

      setState(current => ({...current, isLoading: true, error: undefined}))
      try {
        const [blockResponse, latestResponse] = await Promise.all([
          client.getBlocks({
            workchain,
            shard,
            seqno,
            limit: 1,
          }),
          client.getBlocks({workchain, shard, limit: 1, sort: "desc"}),
        ])
        const block = blockResponse.blocks.find(candidate =>
          isSameBlock(candidate, workchain, shard, seqno),
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

        const [transactionsResponse, shardchainResponse] = await Promise.all([
          client.getBlockTransactions({
            workchain,
            shard,
            seqno,
            limit: BLOCK_TRANSACTIONS_LIMIT,
          }),
          workchain === -1
            ? client.getBlocks({workchain: 0, mcSeqno: seqno, limit: 100, sort: "desc"})
            : Promise.resolve({blocks: []}),
        ])

        if (!isActive) {
          return
        }

        setState({
          block,
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
  }, [client, shard, seqno, workchain])

  const title = workchain === -1 ? "Masterchain block" : "Workchain block"
  const hasValidRoute = Number.isInteger(workchain) && Number.isInteger(seqno) && Boolean(shard)
  const latestPath = state.latestBlock ? blockPath(state.latestBlock) : undefined
  const canOpenPrev = hasValidRoute && seqno > 1
  const prevPath = canOpenPrev ? blockPath({workchain, shard, seqno: seqno - 1}) : undefined
  const nextPath = hasValidRoute ? blockPath({workchain, shard, seqno: seqno + 1}) : undefined
  const transactionAddresses = useMemo(
    () => state.transactions.map(transaction => transaction.account),
    [state.transactions],
  )

  useEffect(() => {
    void prefetchNames(transactionAddresses)
  }, [prefetchNames, transactionAddresses])

  return (
    <div className={styles.container}>
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>{title}</h1>
        </div>
      </section>

      <section className={styles.blocksLayout}>
        {hasValidRoute ? (
          <div className={styles.blockDetailToolbar}>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={!prevPath}
              onClick={() => prevPath && void navigate(prevPath)}
            >
              <ChevronLeft size={14} />
              Prev block
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={!nextPath}
              onClick={() => nextPath && void navigate(nextPath)}
            >
              Next block
              <ChevronRight size={14} />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={
                !latestPath || (state.block !== undefined && latestPath === blockPath(state.block))
              }
              onClick={() => latestPath && void navigate(latestPath)}
            >
              Latest
              <ChevronsRight size={14} />
            </Button>
          </div>
        ) : null}

        {state.error ? (
          <TableStateBlock>{state.error}</TableStateBlock>
        ) : state.isLoading || !state.block ? (
          <BlockDetailsSkeleton showShardchainBlocks={workchain === -1} />
        ) : (
          <>
            <BlockSummaryTable block={state.block} />

            {state.block.workchain === -1 ? (
              <BlockTableSection
                title="Shardchain blocks"
                blocks={state.shardchainBlocks}
                isLoading={false}
                emptyLabel="No shardchain blocks for this masterchain block."
                onOpenBlock={(block, event) => openPath(blockPath(block), event)}
              />
            ) : null}

            <BlockTransactionsTable
              transactions={state.transactions}
              onOpenAccount={(address, event) => openPath(routes.addressPath(address), event)}
              onOpenTransaction={(hash, event) => openPath(routes.transactionPath(hash), event)}
            />
          </>
        )}
      </section>
    </div>
  )
}

const BlockTableSection: FC<{
  readonly title: string
  readonly blocks: readonly V3Block[]
  readonly isLoading: boolean
  readonly emptyLabel: string
  readonly onOpenBlock: (block: V3Block, event?: ExplorerNavigationClickEvent) => void
}> = ({title, blocks, isLoading, emptyLabel, onOpenBlock}) => {
  if (isLoading) {
    return <BlockTableSkeleton title={title} rows={4} />
  }

  if (blocks.length === 0) {
    return <TableStateBlock title={title}>{emptyLabel}</TableStateBlock>
  }

  return (
    <section className={styles.blocksTableFrame} aria-label={title}>
      <header className={styles.blocksTableTitle}>{title}</header>
      <div className={styles.blocksTableScroller}>
        <table className={styles.blocksTable}>
          <thead>
            <tr>
              <th>Block</th>
              <th>Transactions</th>
              <th>Time</th>
            </tr>
          </thead>
          <tbody>
            {blocks.map(block => (
              <tr
                key={blockKey(block)}
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
                  <Link
                    to={blockPath(block)}
                    className={styles.blocksLink}
                    onClick={event => event.stopPropagation()}
                  >
                    {block.seqno}
                  </Link>
                </td>
                <td>{block.tx_count.toLocaleString()}</td>
                <td title={formatAbsoluteBlockTime(block)}>{formatCompactBlockTime(block)}</td>
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
    return <TableStateBlock title="Transactions">No transactions in this block.</TableStateBlock>
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
            {transactions.map((transaction, index) => (
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
                  <button
                    type="button"
                    className={styles.blocksCellButton}
                    onClick={event => {
                      event.stopPropagation()
                      onOpenAccount(transaction.account, event)
                    }}
                  >
                    <AddressLabel address={transaction.account} fallback="Account" />
                  </button>
                </td>
                <td>{transaction.lt}</td>
                <td>
                  <span className={styles.blocksHashCell}>
                    <span className={styles.blocksHashText} title={transaction.hash}>
                      {compactMiddle(transaction.hash, 18)}
                    </span>
                    <CopyTextButton value={transaction.hash} title="Copy hash" />
                  </span>
                </td>
                <td className={styles.blocksExitCodeCell}>
                  {formatTransactionExitCode(transaction)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

const BlockTableSkeleton: FC<{readonly title: string; readonly rows: number}> = ({title, rows}) => (
  <section className={styles.blocksTableFrame} aria-label={`Loading ${title}`}>
    <header className={styles.blocksTableTitle}>{title}</header>
    <div className={styles.blocksTableScroller}>
      <table className={styles.blocksTable}>
        <thead>
          <tr>
            <th>Block</th>
            <th>Transactions</th>
            <th>Time</th>
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

const CopyTextButton: FC<{readonly value: string; readonly title: string}> = ({value, title}) => {
  const [isCopied, setIsCopied] = useState(false)

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = globalThis.setTimeout(() => setIsCopied(false), 1600)
    return () => globalThis.clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={`${styles.blocksCopyButton} ${isCopied ? styles.blocksCopyButtonCopied : ""}`}
      onClick={event => {
        event.stopPropagation()
        void navigator.clipboard.writeText(value)
        setIsCopied(true)
      }}
      aria-label={isCopied ? "Copied" : title}
      title={isCopied ? "Copied" : title}
    >
      {isCopied ? <Check size={14} /> : <Copy size={14} />}
    </button>
  )
}

const BlockSummaryTable: FC<{readonly block: V3Block}> = ({block}) => {
  const prevRefs = formatPrevRefs(block)

  return (
    <section className={styles.blockSummaryPanel} aria-label="Block summary">
      <SummaryItem label="Workchain" value={block.workchain.toString()} />
      <SummaryItem label="Shard" value={block.shard} mono />
      <SummaryItem label="Seqno" value={block.seqno.toString()} />
      <SummaryItem label="LT" value={formatLtRange(block)} mono />
      <SummaryItem
        label="Generated at"
        value={formatAbsoluteBlockTime(block)}
        title={formatAbsoluteBlockTime(block)}
      />
      <SummaryItem label="Root hash" value={block.root_hash} copyValue={block.root_hash} mono />
      <SummaryItem label="File hash" value={block.file_hash} copyValue={block.file_hash} mono />
      <SummaryItem label="Tx quantity" value={block.tx_count.toLocaleString()} />
      <SummaryItem label="Prev refs" value={prevRefs.value} title={prevRefs.title} />
    </section>
  )
}

interface SummaryItemProps {
  readonly label: string
  readonly value: string
  readonly title?: string
  readonly copyValue?: string
  readonly mono?: boolean
}

const SummaryItem: FC<SummaryItemProps> = ({label, value, title, copyValue, mono = false}) => (
  <div className={styles.blockSummaryRow}>
    <span className={styles.blockSummaryLabel}>{label}</span>
    <span
      className={`${styles.blockSummaryValue} ${copyValue ? styles.blockSummaryValueWithCopy : ""} ${mono ? styles.blocksMonoCell : ""}`}
      title={title ?? value}
    >
      {copyValue ? (
        <>
          <span className={styles.blockSummaryValueText}>{value}</span>
          <CopyTextButton value={copyValue} title={`Copy ${label.toLowerCase()}`} />
        </>
      ) : (
        value
      )}
    </span>
  </div>
)

const BlockDetailsSkeleton: FC<{readonly showShardchainBlocks: boolean}> = ({
  showShardchainBlocks,
}) => (
  <>
    <section className={styles.blockSummaryPanel} aria-label="Loading block summary">
      {BLOCK_SUMMARY_LABELS.map(label => (
        <div key={label} className={styles.blockSummaryRow}>
          <span className={styles.blockSummaryLabel}>{label}</span>
          <span className={`${styles.skeletonLine} ${styles.blocksSkeletonValue}`} />
        </div>
      ))}
    </section>
    {showShardchainBlocks ? <BlockTableSkeleton title="Shardchain blocks" rows={1} /> : null}
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

function blockKey(block: Pick<V3Block, "workchain" | "shard" | "seqno">): string {
  return `${block.workchain}:${block.shard}:${block.seqno}`
}

function isSameBlock(block: V3Block, workchain: number, shard: string, seqno: number): boolean {
  return block.workchain === workchain && block.shard === shard && block.seqno === seqno
}

function formatLtRange(block: V3Block): string {
  return block.start_lt === block.end_lt ? block.start_lt : `${block.start_lt} - ${block.end_lt}`
}

function formatPrevRefs(block: V3Block): {value: string; title: string} {
  const refs = block.prev_blocks ?? []
  if (refs.length === 0) {
    return {value: "None", title: "None"}
  }

  const fullRefs = refs.map(ref => `${ref.workchain}:${ref.shard}:${ref.seqno}`)
  const compactRefs = refs.map(
    ref => `${ref.workchain}:${compactMiddle(ref.shard, 8)}:${ref.seqno}`,
  )
  return {value: compactRefs.join(", "), title: fullRefs.join(", ")}
}

function blockUnixTime(block: V3Block): number | undefined {
  const value = Number(block.gen_utime)
  return Number.isFinite(value) && value > 0 ? value : undefined
}

function formatCompactBlockTime(block: V3Block): string {
  const unixTime = blockUnixTime(block)
  if (unixTime === undefined) {
    return "Unknown"
  }

  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(unixTime * 1000))
}

function formatAbsoluteBlockTime(block: V3Block): string {
  const unixTime = blockUnixTime(block)
  if (unixTime === undefined) {
    return "Unknown"
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(unixTime * 1000))
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
  return `${value.slice(0, side)}...${value.slice(-side)}`
}
