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
  Dialog,
  InlineAction,
  useToast,
} from "@acton/ui"
import {Box, ExternalLink, Pencil, Plus, Trash2, Waypoints} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import {useNavigate} from "react-router"

import {TablePage} from "../../../components/TablePage"
import {supports} from "../../../environmentCapabilities"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import type {TonClient} from "@acton/explorer-core/api/client"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {ExplorerAddressChip} from "@acton/explorer-core/components/ExplorerAddressChip"
import {formatAddress} from "@acton/explorer-core/components/utils"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import {localnetContractPath, useLocalnetRoutes} from "../../routes"
import {AddContractDialog} from "../components/AddContractDialog"
import {EditContractNameDialog} from "../components/EditContractNameDialog"
import {contractOriginLabels, getContractIdentity} from "../contracts/contractPresentation"
import {ContractStatus} from "../contracts/ContractStatus"

import styles from "./ContractsPage.module.css"

interface ContractsPageProps {
  readonly addOpen: boolean
  readonly client: TonClient
  readonly onAddOpenChange: (open: boolean) => void
}

export function ContractsPage({addOpen, client, onAddOpenChange}: ContractsPageProps) {
  const {showToast} = useToast()
  const navigate = useNavigate()
  const routes = useExplorerRoutePaths()
  const localnetRoutes = useLocalnetRoutes()
  const addressFormat = useAddressFormat()
  const {environment} = useLocalnetRuntime()
  const simulatorEnabled = supports(environment, "simulator")
  const [contracts, setContracts] = useState<readonly LocalnetContract[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string>()
  const [contractBeingRenamed, setContractBeingRenamed] = useState<LocalnetContract>()
  const [contractBeingDeleted, setContractBeingDeleted] = useState<LocalnetContract>()
  const [deleting, setDeleting] = useState(false)
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

  const deleteContract = async () => {
    if (!contractBeingDeleted) return

    const contract = contractBeingDeleted
    setDeleting(true)
    try {
      await client.deleteContract(contract.address)
      setContracts(current =>
        current.filter(currentContract => currentContract.address !== contract.address),
      )
      setContractBeingDeleted(undefined)
      showToast({
        title: "Contract removed from Studio",
        description: `${getContractIdentity(contract).title} was removed from this environment's registry`,
        variant: "success",
      })
    } catch (error) {
      showToast({
        title: "Contract not removed",
        description:
          error instanceof Error ? error.message : "Failed to remove contract from Studio",
        variant: "error",
      })
    } finally {
      setDeleting(false)
    }
  }

  return (
    <>
      <TablePage
        error={loadError}
        errorTitle="Unable to load contracts"
        hasContent={contracts.length > 0}
        onRetry={loadContracts}
      >
        <DataTable minWidth="56rem">
          <DataTableTable aria-label="Contracts">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="26%">Name</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="24%">Address</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="12%">Status</DataTableHeaderCell>
                <DataTableHeaderCell>Origin and source</DataTableHeaderCell>
                <DataTableHeaderCell align="right" columnWidth="6.5rem">
                  Actions
                </DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {loading && contracts.length === 0 ? (
                <DataTableSkeletonRows
                  columns={5}
                  rows={4}
                  widths={["64%", "72%", "44%", "62%", "72%"]}
                  alignments={["left", "left", "left", "left", "right"]}
                />
              ) : contracts.length === 0 ? (
                <DataTableEmpty colSpan={5}>
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
                        {simulatorEnabled ? (
                          <InlineAction
                            label="Open in Simulator"
                            icon={<Waypoints />}
                            onClick={() => openSimulator(contract.address)}
                          />
                        ) : null}
                        <InlineAction
                          label="Remove contract from Studio"
                          icon={<Trash2 />}
                          onClick={() => setContractBeingDeleted(contract)}
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
      <Dialog
        open={contractBeingDeleted !== undefined}
        onOpenChange={open => {
          if (!open && !deleting) setContractBeingDeleted(undefined)
        }}
        title={
          contractBeingDeleted
            ? `Remove ${getContractIdentity(contractBeingDeleted).title} from Studio?`
            : "Remove contract from Studio?"
        }
        description="Only the Studio registry entry will be removed. The on-chain contract, source files, and ABI will not be changed."
        dismissible={!deleting}
        maxWidth="30rem"
      >
        <div className={styles.dialogActions}>
          <Button
            type="button"
            variant="secondary"
            disabled={deleting}
            onClick={() => setContractBeingDeleted(undefined)}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="danger"
            loading={deleting}
            leadingIcon={<Trash2 size={15} aria-hidden="true" />}
            onClick={() => void deleteContract()}
          >
            Remove from Studio
          </Button>
        </div>
      </Dialog>
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
      <span>{contractOriginLabels[contract.sourceKind].short}</span>
      <span>{contract.artifact ? "Acton source" : "Source unavailable"}</span>
    </span>
  )
}
