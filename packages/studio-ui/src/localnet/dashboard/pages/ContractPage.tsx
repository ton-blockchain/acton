import {Button, CodeViewer, ContentTabs, CopyInlineAction, InlineAction, Skeleton} from "@acton/ui"
import {AbiPanel, type AbiTab} from "@acton/transaction-ui/abi"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {CheckCircle2, CircleAlert, FileCode2, Pencil, Search, Waypoints} from "lucide-react"
import {useState, type ReactNode} from "react"
import {useNavigate, useParams} from "react-router"

import {TablePage} from "../../../components/TablePage"
import {supports} from "../../../environmentCapabilities"
import {useLocalnetRuntime} from "../../LocalnetRuntimeProvider"
import type {TonClient} from "@acton/explorer-core/api/client"
import type {LocalnetContract} from "@acton/explorer-core/api/types"
import {ExplorerAddressChip} from "@acton/explorer-core/components/ExplorerAddressChip"
import {formatAddress} from "@acton/explorer-core/components/utils"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"
import type {RegisteredSource} from "@acton/explorer-core/metadata/types"
import {localnetContractPath, useLocalnetRoutes} from "../../routes"
import {EditContractNameDialog} from "../components/EditContractNameDialog"
import {
  contractOriginLabels,
  formatContractCompiler,
  getContractIdentity,
  shortenTechnicalValue,
} from "../contracts/contractPresentation"
import {ContractStatus} from "../contracts/ContractStatus"
import {type ContractDetails, useContractDetails} from "../contracts/useContractDetails"

import styles from "./ContractPage.module.css"

export type ContractSection = "source" | "abi" | "raw-abi"

interface ContractPageProps {
  readonly client: TonClient
  readonly section: ContractSection
}

const contractTabs = [
  {label: "Source", value: "source"},
  {label: "ABI", value: "abi"},
  {label: "Raw ABI", value: "raw-abi"},
] as const

export function ContractPage({client, section}: ContractPageProps) {
  const {address = ""} = useParams<{address: string}>()
  const navigate = useNavigate()
  const explorerRoutes = useExplorerRoutePaths()
  const localnetRoutes = useLocalnetRoutes()
  const addressFormat = useAddressFormat()
  const {environment} = useLocalnetRuntime()
  const simulatorEnabled = supports(environment, "simulator")
  const {details, error, loading, reload} = useContractDetails(client, address)
  const [renaming, setRenaming] = useState(false)

  if (loading && !details) {
    return <ContractPageSkeleton />
  }

  if (error || !details) {
    return (
      <div className={styles.page}>
        <TablePage
          error={error ?? "Contract details are unavailable"}
          errorTitle="Unable to load contract"
          hasContent={false}
          onRetry={reload}
        >
          <span />
        </TablePage>
      </div>
    )
  }

  const {contract} = details
  const identity = getContractIdentity(contract)
  const userFriendlyAddress = formatAddress(contract.address, false, addressFormat)
  const openExplorer = () => void navigate(explorerRoutes.addressPath(userFriendlyAddress))
  const openSimulator = () => {
    const search = new URLSearchParams({address: userFriendlyAddress})
    void navigate(`${explorerRoutes.emulatePath}?${search}`)
  }
  const openSection = (nextSection: ContractSection) => {
    void navigate(
      localnetContractPath(
        localnetRoutes.basePath,
        userFriendlyAddress,
        nextSection === "source" ? undefined : nextSection,
      ),
    )
  }

  return (
    <>
      <div className={styles.page}>
        <section className={styles.identityPanel}>
          <div className={styles.identityHeader}>
            <div className={styles.identity}>
              <div className={styles.identityBody}>
                <div className={styles.identityTitleRow}>
                  <h1>{identity.title}</h1>
                  <InlineAction
                    className={styles.renameAction}
                    icon={<Pencil />}
                    label="Edit contract name"
                    size="compact"
                    onClick={() => setRenaming(true)}
                  />
                  <ContractStatus status={contract.status} />
                </div>
                <ExplorerAddressChip
                  address={userFriendlyAddress}
                  className={styles.identityAddress}
                  resolveName={false}
                  shorten={false}
                  variant="plain"
                  onAddressClick={openExplorer}
                />
              </div>
            </div>
            <div className={styles.identityActions}>
              <Button size="sm" variant="outline" leadingIcon={<Search />} onClick={openExplorer}>
                Open in Explorer
              </Button>
              {simulatorEnabled ? (
                <Button
                  size="sm"
                  variant="primary"
                  leadingIcon={<Waypoints />}
                  onClick={openSimulator}
                >
                  Simulate
                </Button>
              ) : null}
            </div>
          </div>
          <ContractSummary details={details} />
        </section>

        <ArtifactComparison
          contract={contract}
          currentSource={details.currentSource}
          deployedSource={details.deployedSource}
          error={details.sourceError}
        />

        <ContentTabs<ContractSection>
          className={styles.workspace}
          panelClassName={styles.workspacePanel}
          tabs={contractTabs}
          value={section}
          onValueChange={openSection}
        >
          {section === "source" ? (
            <ContractSource
              contract={contract}
              deployedSource={details.deployedSource}
              currentSource={details.currentSource}
              error={details.sourceError}
              onOpenSources={() => void navigate(explorerRoutes.sourcesPath)}
            />
          ) : section === "abi" || section === "raw-abi" ? (
            <ContractAbi
              activeTab={section === "raw-abi" ? "raw" : "view"}
              abi={details.abi?.compiler_abi}
              error={details.abiError}
              onOpenAbi={() => void navigate(explorerRoutes.abiPath)}
              onTabChange={tab => openSection(tab === "raw" ? "raw-abi" : "abi")}
            />
          ) : null}
        </ContentTabs>
      </div>

      <EditContractNameDialog
        client={client}
        contract={renaming ? contract : undefined}
        onSaved={() => reload(false)}
        onOpenChange={setRenaming}
      />
    </>
  )
}

function ContractSummary({details}: {readonly details: ContractDetails}) {
  const {contract, deployedSource} = details
  const artifactId = deployedArtifactId(contract, deployedSource)

  return (
    <dl className={`${styles.detailList} ${styles.summaryList}`}>
      <Detail label="Origin">{contractOriginLabels[contract.sourceKind].detail}</Detail>
      <Detail label="Code hash">
        <TechnicalValue value={contract.codeHash} label="code hash" />
      </Detail>
      <Detail label="Artifact ID">
        <TechnicalValue value={artifactId} label="artifact ID" />
      </Detail>
    </dl>
  )
}

function ContractSource({
  contract,
  currentSource,
  deployedSource,
  error,
  onOpenSources,
}: {
  readonly contract: LocalnetContract
  readonly currentSource?: RegisteredSource
  readonly deployedSource?: RegisteredSource
  readonly error?: string
  readonly onOpenSources: () => void
}) {
  const bundle = deployedSource?.source.bundle
  if (!bundle) {
    return (
      <ArtifactEmptyState
        title={error ? "Source could not be loaded" : "Source unavailable"}
        description={
          error ??
          "Register the Acton source artifact that produced the code deployed at this address"
        }
        action="Open Sources"
        onAction={onOpenSources}
      />
    )
  }

  return (
    <div className={styles.artifactContent}>
      <ArtifactContext
        contract={contract}
        currentSource={currentSource}
        deployedSource={deployedSource}
      />
      <CodeViewer
        attachedToTabs
        className={styles.sourceViewer}
        entrypoint={bundle.entrypoint}
        files={bundle.files}
      />
    </div>
  )
}

function ContractAbi({
  activeTab,
  abi,
  error,
  onOpenAbi,
  onTabChange,
}: {
  readonly activeTab: AbiTab
  readonly abi: ContractABI | undefined
  readonly error?: string
  readonly onOpenAbi: () => void
  readonly onTabChange: (tab: AbiTab) => void
}) {
  if (!abi) {
    return (
      <ArtifactEmptyState
        title={error ? "ABI could not be loaded" : "ABI unavailable"}
        description={error ?? "Register an ABI matching the code deployed at this address"}
        action="Open ABI catalog"
        onAction={onOpenAbi}
      />
    )
  }

  return (
    <div className={styles.abiViewer}>
      <AbiPanel
        abi={abi}
        activeTab={activeTab}
        attachedToTabs
        heightMode="content"
        showTabs={false}
        onTabChange={onTabChange}
      />
    </div>
  )
}

function ArtifactComparison({
  contract,
  currentSource,
  deployedSource,
  error,
}: {
  readonly contract: LocalnetContract
  readonly currentSource?: RegisteredSource
  readonly deployedSource?: RegisteredSource
  readonly error?: string
}) {
  const deployedId = deployedArtifactId(contract, deployedSource)
  const currentId = sourceArtifactId(currentSource)
  const changed = Boolean(deployedId && currentId && deployedId !== currentId)
  const unavailable = !deployedSource || !deployedId

  return (
    <section
      className={styles.comparison}
      data-state={error || unavailable ? "unknown" : changed ? "changed" : "current"}
    >
      <span className={styles.comparisonIcon} aria-hidden="true">
        {error || unavailable || changed ? <CircleAlert /> : <CheckCircle2 />}
      </span>
      <div className={styles.comparisonBody}>
        <strong>
          {error
            ? "Build comparison unavailable"
            : unavailable
              ? "Deployed source is not linked"
              : changed
                ? "Project source changed after this deployment"
                : "Deployment matches the latest project build"}
        </strong>
        <span>
          {error
            ? error
            : unavailable
              ? "Build or add the matching contract source to inspect this deployment"
              : changed
                ? "The network keeps the immutable deployed artifact until the contract is deployed again"
                : "The deployed code and latest artifact for this entrypoint are aligned"}
        </span>
      </div>
      {changed && deployedId && currentId ? (
        <div className={styles.comparisonArtifacts}>
          <ArtifactReference label="Deployed" value={deployedId} />
          <ArtifactReference label="Latest build" value={currentId} />
        </div>
      ) : null}
    </section>
  )
}

function ArtifactContext({
  contract,
  currentSource,
  deployedSource,
}: {
  readonly contract: LocalnetContract
  readonly currentSource?: RegisteredSource
  readonly deployedSource: RegisteredSource
}) {
  const deployedId = deployedArtifactId(contract, deployedSource)
  const currentId = sourceArtifactId(currentSource)
  const changed = Boolean(deployedId && currentId && deployedId !== currentId)
  const compiler = deployedSource.source.bundle?.compiler

  return (
    <div className={styles.artifactContext}>
      <div>
        <strong>Deployed source</strong>
        <span>{formatContractCompiler(compiler)}</span>
      </div>
      <span className={styles.artifactContextStatus} data-state={changed ? "changed" : "current"}>
        {changed ? <CircleAlert aria-hidden="true" /> : <CheckCircle2 aria-hidden="true" />}
        {changed ? "Different from latest build" : "Latest build"}
      </span>
    </div>
  )
}

function Detail({children, label}: {readonly children: ReactNode; readonly label: string}) {
  return (
    <div className={styles.detail}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  )
}

function TechnicalValue({
  label,
  shorten = true,
  value,
}: {
  readonly label: string
  readonly shorten?: boolean
  readonly value: string | undefined
}) {
  if (!value) return <span className={styles.unavailable}>Unavailable</span>

  return (
    <span className={styles.technicalValue}>
      <code title={value}>{shorten ? shortenTechnicalValue(value) : value}</code>
      <CopyInlineAction
        value={value}
        label={`Copy ${label}`}
        copiedLabel={`Copied ${label}`}
        size="compact"
      />
    </span>
  )
}

function ArtifactReference({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <span className={styles.artifactReference}>
      <span>{label}</span>
      <TechnicalValue label={`${label.toLowerCase()} artifact ID`} value={value} />
    </span>
  )
}

function ArtifactEmptyState({
  action,
  description,
  onAction,
  title,
}: {
  readonly action: string
  readonly description: string
  readonly onAction: () => void
  readonly title: string
}) {
  return (
    <div className={styles.artifactEmpty}>
      <span className={styles.artifactEmptyIcon} aria-hidden="true">
        <FileCode2 />
      </span>
      <strong>{title}</strong>
      <span>{description}</span>
      <Button size="sm" variant="outline" onClick={onAction}>
        {action}
      </Button>
    </div>
  )
}

function ContractPageSkeleton() {
  return (
    <div className={styles.page} aria-label="Loading contract" role="status">
      <section className={styles.identityPanel}>
        <div className={styles.identityHeader}>
          <div className={styles.skeletonIdentity}>
            <div>
              <Skeleton width="12rem" height="1.25rem" />
              <Skeleton width="22rem" height="0.875rem" />
            </div>
          </div>
          <Skeleton width="18rem" height="2rem" radius="md" />
        </div>
        <div className={styles.skeletonSummary}>
          {Array.from({length: 3}, (_, index) => (
            <div key={index}>
              <Skeleton width="5rem" height="0.75rem" />
              <Skeleton width="8rem" height="0.875rem" />
            </div>
          ))}
        </div>
      </section>
      <Skeleton shape="rect" radius="md" width="100%" height="25rem" />
    </div>
  )
}

function deployedArtifactId(
  contract: LocalnetContract,
  source: RegisteredSource | undefined,
): string | undefined {
  return contract.artifact?.artifactId ?? sourceArtifactId(source)
}

function sourceArtifactId(source: RegisteredSource | undefined): string | undefined {
  return source?.artifactId ?? source?.source.bundle?.source_bundle_hash
}
