import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  InlineAction,
} from "@acton/ui"
import {Box, ExternalLink, Pencil, Plus, Waypoints} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import {useNavigate} from "react-router"

import {TablePage} from "../../../components/TablePage"
import type {TonClient} from "../../explorer/api/client"
import type {LocalnetContract} from "../../explorer/api/types"
import {ExplorerAddressChip} from "../../explorer/components/ExplorerAddressChip"
import {formatAddress, formatRelativeTime} from "../../explorer/components/utils"
import {useExplorerRoutePaths} from "../../explorer/hooks/useExplorerRoutePaths"
import {useAddressFormat} from "../../explorer/hooks/useNetworkInfo"
import {localnetContractPath, useLocalnetRoutes} from "../../routes"
import {AddContractDialog} from "../components/AddContractDialog"
import {EditContractNameDialog} from "../components/EditContractNameDialog"
import {getContractIdentity} from "../contracts/contractPresentation"
import {ContractStatus} from "../contracts/ContractStatus"

import styles from "./ContractsPage.module.css"

interface ContractsPageProps {
  readonly addOpen: boolean
  readonly client: TonClient
  readonly onAddOpenChange: (open: boolean) => void
}

export function ContractsPage({addOpen, client, onAddOpenChange}: ContractsPageProps) {
  const navigate = useNavigate()
  const routes = useExplorerRoutePaths()
  const localnetRoutes = useLocalnetRoutes()
  const addressFormat = useAddressFormat()
  const [contracts, setContracts] = useState<readonly LocalnetContract[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string>()
  const [contractBeingRenamed, setContractBeingRenamed] = useState<LocalnetContract>()
  const latestLoadId = useRef(0)

  const loadContracts = useCallback(
    async (showLoading = true) => {
      const loadId = ++latestLoadId.current
      if (showLoading) {
        setLoading(true)
        setLoadError(undefined)
      }
      try {
        const nextContracts = await client.listContracts()
        if (loadId === latestLoadId.current) {
          setContracts(nextContracts)
          setLoadError(undefined)
        }
      } catch (error) {
        if (loadId === latestLoadId.current && showLoading) {
          setLoadError(error instanceof Error ? error.message : "Failed to load contracts")
        }
      } finally {
        if (loadId === latestLoadId.current && showLoading) {
          setLoading(false)
        }
      }
    },
    [client],
  )

  useEffect(() => {
    void loadContracts()
  }, [loadContracts])

  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible") {
        void loadContracts(false)
      }
    }
    const interval = globalThis.setInterval(refresh, 3000)
    globalThis.addEventListener("focus", refresh)
    document.addEventListener("visibilitychange", refresh)

    return () => {
      globalThis.clearInterval(interval)
      globalThis.removeEventListener("focus", refresh)
      document.removeEventListener("visibilitychange", refresh)
    }
  }, [loadContracts])

  const openContractDetails = (address: string) => {
    void navigate(
      localnetContractPath(localnetRoutes.basePath, formatAddress(address, false, addressFormat)),
    )
  }

  const openExplorer = (address: string) => {
    void navigate(routes.addressPath(formatAddress(address, false, addressFormat)))
  }

  const openSimulator = (address: string) => {
    const search = new URLSearchParams({
      address: formatAddress(address, false, addressFormat),
    })
    void navigate(`${routes.emulatePath}?${search}`)
  }

  return (
    <>
      <TablePage
        error={loadError}
        errorTitle="Unable to load contracts"
        hasContent={contracts.length > 0}
        onRetry={loadContracts}
      >
        <DataTable minWidth="64rem">
          <DataTableTable aria-label="Contracts">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="24%">Name</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="22%">Address</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="11%">Status</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="18%">Origin and source</DataTableHeaderCell>
                <DataTableHeaderCell>Last activity</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="6.5rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {loading && contracts.length === 0 ? (
                <DataTableSkeletonRows
                  columns={6}
                  rows={4}
                  widths={["64%", "72%", "44%", "62%", "50%", "72%"]}
                  alignments={["left", "left", "left", "left", "left", "right"]}
                />
              ) : contracts.length === 0 ? (
                <DataTableEmpty colSpan={6}>
                  <div className={styles.emptyState}>
                    <span className={styles.emptyIcon}>
                      <Box size={20} aria-hidden="true" />
                    </span>
                    <strong>No contracts found</strong>
                    <span>
                      Add a deployed address or interact with a contract in this environment
                    </span>
                    <Button
                      size="sm"
                      variant="primary"
                      leadingIcon={<Plus size={15} aria-hidden="true" />}
                      onClick={() => onAddOpenChange(true)}
                    >
                      Add contract
                    </Button>
                  </div>
                </DataTableEmpty>
              ) : (
                contracts.map(contract => (
                  <DataTableRow
                    key={contract.address}
                    hover
                    interactive
                    onClick={event => {
                      const target = event.target
                      if (
                        target instanceof Element &&
                        target.closest("button, a, input, select, textarea")
                      ) {
                        return
                      }
                      openContractDetails(contract.address)
                    }}
                  >
                    <DataTableCell>
                      <ContractIdentity contract={contract} />
                    </DataTableCell>
                    <DataTableCell>
                      <ExplorerAddressChip
                        address={contract.address}
                        resolveName={false}
                        onAddressClick={openExplorer}
                      />
                    </DataTableCell>
                    <DataTableCell>
                      <ContractStatus status={contract.status} />
                    </DataTableCell>
                    <DataTableCell>
                      <ContractSource contract={contract} />
                    </DataTableCell>
                    <DataTableCell>
                      <ContractLastActivity contract={contract} />
                    </DataTableCell>
                    <DataTableCell align="right">
                      <span className={styles.actions}>
                        <InlineAction
                          label="Edit contract name"
                          icon={<Pencil />}
                          onClick={() => setContractBeingRenamed(contract)}
                        />
                        <InlineAction
                          label="Open in Explorer"
                          icon={<ExternalLink />}
                          onClick={() => openExplorer(contract.address)}
                        />
                        <InlineAction
                          label="Open in Simulator"
                          icon={<Waypoints />}
                          onClick={() => openSimulator(contract.address)}
                        />
                      </span>
                    </DataTableCell>
                  </DataTableRow>
                ))
              )}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      </TablePage>

      <AddContractDialog
        client={client}
        open={addOpen}
        onAdded={loadContracts}
        onOpenChange={onAddOpenChange}
      />
      <EditContractNameDialog
        client={client}
        contract={contractBeingRenamed}
        onSaved={loadContracts}
        onOpenChange={open => {
          if (!open) setContractBeingRenamed(undefined)
        }}
      />
    </>
  )
}

function ContractIdentity({contract}: {readonly contract: LocalnetContract}) {
  const {title} = getContractIdentity(contract)

  return (
    <span className={styles.identity}>
      <span className={styles.identityName}>{title}</span>
    </span>
  )
}

function ContractSource({contract}: {readonly contract: LocalnetContract}) {
  return (
    <span className={styles.source}>
      <span>{contract.sourceKind === "fork" ? "Fork" : "Local"}</span>
      <span>{contract.artifact ? "Acton source" : "Source unavailable"}</span>
    </span>
  )
}

function ContractLastActivity({contract}: {readonly contract: LocalnetContract}) {
  if (contract.lastActivityAt) {
    const date = new Date(contract.lastActivityAt * 1000)
    return (
      <time
        className={styles.lastActivity}
        dateTime={date.toISOString()}
        title={date.toLocaleString()}
      >
        {formatRelativeTime(contract.lastActivityAt)}
      </time>
    )
  }

  return (
    <span
      className={styles.muted}
      title={contract.lastTransactionLt ? `Logical time ${contract.lastTransactionLt}` : undefined}
    >
      {contract.lastTransactionLt ? "Date unavailable" : "No activity"}
    </span>
  )
}
