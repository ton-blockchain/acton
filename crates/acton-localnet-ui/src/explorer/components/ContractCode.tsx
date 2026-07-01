import {Buffer} from "node:buffer"
import {
  type ContractData,
  ContractSourcePanel,
  type ContractVerifiedSource,
  DataBlock,
  decodeStorageDataCell,
  ParsedValueView,
} from "@acton/shared-ui"
import {Cell} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import type {FC, JSX} from "react"
import {useMemo, useState} from "react"

import type {TonClient} from "../api/client"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import {AbiPanel, type AbiTab} from "./abi-viewer"
import styles from "./ContractCode.module.css"
import type {AddressFormatOptions} from "./utils"

interface ContractCodeProps {
  readonly codeBoc: string
  readonly ownerAddress: string
  readonly client: TonClient
  readonly dataBoc?: string
  readonly compilerAbi?: ContractABI
  readonly compilerAbiLoading?: boolean
  readonly compilerAbiError?: string
  readonly verifiedSource?: ContractVerifiedSource
  readonly verifiedSourceLoading?: boolean
  readonly onContractClick?: (address: string) => void
}

type ContractCodeTab = "storage" | "source" | "abi"
type StorageTab = "parsed" | "base64" | "hex" | "hex-hash" | "base64-hash"

function readContractHashTab(): ContractCodeTab {
  if (typeof globalThis.window === "undefined") {
    return "storage"
  }

  const hash = globalThis.window.location.hash.replace("#", "")
  if (hash === "contract-source") {
    return "source"
  }
  if (hash === "contract-abi") {
    return "abi"
  }
  return "storage"
}

function writeContractHashTab(tab: ContractCodeTab): void {
  if (typeof globalThis.window === "undefined") {
    return
  }

  const nextUrl = `${globalThis.window.location.pathname}${globalThis.window.location.search}#contract-${tab}`
  globalThis.window.history.replaceState(undefined, "", nextUrl)
}

export const ContractCode: FC<ContractCodeProps> = ({
  codeBoc,
  ownerAddress,
  client,
  dataBoc,
  compilerAbi,
  compilerAbiLoading = false,
  compilerAbiError,
  verifiedSource,
  verifiedSourceLoading = false,
  onContractClick,
}) => {
  const [activeTab, setActiveTab] = useState<ContractCodeTab>(() => readContractHashTab())
  const [activeStorageTab, setActiveStorageTab] = useState<StorageTab>("parsed")
  const [activeAbiTab, setActiveAbiTab] = useState<AbiTab>("view")
  const addressFormat = useAddressFormat()
  const routes = useExplorerRoutePaths()

  const parsedStorage = useMemo(
    () => decodeStorageDataCell(dataBoc, compilerAbi),
    [dataBoc, compilerAbi],
  )
  const storageData = useMemo(() => {
    if (!dataBoc) return
    try {
      const buf = Buffer.from(dataBoc, "base64")
      const dataCell = Cell.fromBase64(dataBoc)

      return {
        base64: dataBoc,
        dataHashBase64: dataCell.hash().toString("base64"),
        dataHashHex: dataCell.hash().toString("hex"),
        hex: buf.toString("hex").toUpperCase(),
      }
    } catch (error) {
      console.error("Failed to process contract data:", error)
      return {
        base64: dataBoc,
        dataHashBase64: "Error processing data hash",
        dataHashHex: "Error processing data hash",
        hex: "Error processing data HEX",
      }
    }
  }, [dataBoc])
  const contracts = useMemo(() => new Map<string, ContractData>(), [])
  const storageUnavailableMessage = compilerAbi
    ? dataBoc
      ? "Storage data could not be decoded with this ABI"
      : "No storage data available for this account"
    : "No ABI registered for storage decoding"
  const hasVerifiedSource = Boolean(verifiedSource?.verified && verifiedSource.bundles.length > 0)
  const hasLocalVerifiedSource = Boolean(
    verifiedSource?.bundles.some(bundle => bundle.storage_revision === "local"),
  )

  const handleContractTabChange = (tab: ContractCodeTab) => {
    setActiveTab(tab)
    writeContractHashTab(tab)
  }

  if (!codeBoc) {
    return (
      <div className={`${styles.container} ${styles.emptyContainer}`}>
        <div className={styles.empty}>No code available for this account</div>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.tabs}>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "storage" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("storage")}
        >
          Storage
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "source" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("source")}
        >
          Source
        </button>
        <button
          type="button"
          className={`${styles.tab} ${activeTab === "abi" ? styles.tabActive : ""}`}
          onClick={() => handleContractTabChange("abi")}
        >
          ABI
        </button>
      </div>

      <div className={styles.content}>
        {activeTab === "storage" ? (
          <StoragePanel
            activeTab={activeStorageTab}
            onTabChange={setActiveStorageTab}
            storageData={storageData}
            parsedStorage={parsedStorage}
            contracts={contracts}
            addressFormat={addressFormat}
            onContractClick={onContractClick}
            unavailableMessage={storageUnavailableMessage}
          />
        ) : activeTab === "abi" ? (
          compilerAbiError ? (
            <div className={`${styles.empty} ${styles.panelEmpty} ${styles.emptyError}`}>
              Failed to load ABI: {compilerAbiError}
            </div>
          ) : compilerAbiLoading ? (
            <AbiLoadingSkeleton />
          ) : compilerAbi ? (
            <AbiPanel
              activeTab={activeAbiTab}
              onTabChange={setActiveAbiTab}
              abi={compilerAbi}
              ownerAddress={ownerAddress}
              client={client}
              getMethodsMode="interactive"
            />
          ) : (
            <div className={`${styles.empty} ${styles.panelEmpty}`}>
              No ABI registered for this contract
            </div>
          )
        ) : (
          <ContractSourcePanel
            codeBoc={codeBoc}
            verifiedSource={hasVerifiedSource ? verifiedSource : undefined}
            verifiedSourceLoading={verifiedSourceLoading && !hasVerifiedSource}
            verificationUrl={hasLocalVerifiedSource ? routes.sourcesPath : undefined}
            verificationExternal={!hasLocalVerifiedSource}
          />
        )}
      </div>
    </div>
  )
}

function AbiLoadingSkeleton(): JSX.Element {
  return (
    <div className={styles.abiResultSkeleton} aria-label="Loading compiler ABI">
      <span />
      <span />
      <span />
    </div>
  )
}

function StoragePanel({
  activeTab,
  onTabChange,
  storageData,
  parsedStorage,
  contracts,
  addressFormat,
  onContractClick,
  unavailableMessage,
}: {
  readonly activeTab: StorageTab
  readonly onTabChange: (tab: StorageTab) => void
  readonly storageData?: {
    readonly base64: string
    readonly dataHashBase64: string
    readonly dataHashHex: string
    readonly hex: string
  }
  readonly parsedStorage?: ReturnType<typeof decodeStorageDataCell>
  readonly contracts: Map<string, ContractData>
  readonly addressFormat: AddressFormatOptions
  readonly onContractClick?: (address: string) => void
  readonly unavailableMessage: string
}): JSX.Element {
  const storageTabs: readonly {tab: StorageTab; label: string}[] = [
    {tab: "parsed", label: "parsed"},
    {tab: "base64", label: "base64"},
    {tab: "hex", label: "hex"},
    {tab: "hex-hash", label: "hex hash"},
    {tab: "base64-hash", label: "base64 hash"},
  ]
  const activeStorage =
    activeTab === "base64"
      ? {
          title: "Data BoC Base64",
          value: storageData?.base64,
          wrap: true,
        }
      : activeTab === "hex"
        ? {
            title: "Data BoC HEX",
            value: storageData?.hex,
            wrap: true,
          }
        : activeTab === "hex-hash"
          ? {
              title: "Data hash HEX",
              value: storageData?.dataHashHex,
              wrap: true,
            }
          : activeTab === "base64-hash"
            ? {
                title: "Data hash Base64",
                value: storageData?.dataHashBase64,
                wrap: true,
              }
            : undefined

  return (
    <section className={styles.sourceShell}>
      <div className={styles.editorTabBar}>
        {storageTabs.map(item => (
          <button
            key={item.tab}
            type="button"
            className={`${styles.editorTab} ${activeTab === item.tab ? styles.editorTabActive : ""}`}
            onClick={() => onTabChange(item.tab)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {activeTab === "parsed" ? (
        parsedStorage ? (
          <section className={styles.dataPanel}>
            <div className={styles.storageBlock}>
              <ParsedValueView
                value={parsedStorage.value}
                contracts={contracts}
                addressFormat={addressFormat}
                onContractClick={onContractClick}
                fallbackTypeName={parsedStorage.name}
              />
            </div>
          </section>
        ) : (
          <div className={`${styles.empty} ${styles.panelEmpty}`}>{unavailableMessage}</div>
        )
      ) : activeStorage?.value ? (
        <ContractTextPanel
          title={activeStorage.title}
          value={activeStorage.value}
          wrap={activeStorage.wrap}
        />
      ) : (
        <div className={`${styles.empty} ${styles.panelEmpty}`}>
          No storage data available for this account
        </div>
      )}
    </section>
  )
}

function ContractTextPanel({
  title,
  value,
  wrap = false,
}: {
  readonly title: string
  readonly value: string
  readonly wrap?: boolean
}): JSX.Element {
  return (
    <DataBlock
      className={styles.sourceDataBlock}
      variant="standalone"
      copyLabel={title}
      copyValue={value}
    >
      <pre className={`${styles.code} ${wrap ? styles.codeWrap : ""}`}>
        <code>{value}</code>
      </pre>
    </DataBlock>
  )
}
