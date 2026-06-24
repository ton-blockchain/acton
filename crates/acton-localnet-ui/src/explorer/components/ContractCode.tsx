import {Buffer} from "node:buffer"
import {useEffect, useMemo, useState} from "react"
import type {CSSProperties, FC, JSX} from "react"

import {
  decodeStorageDataCell,
  DataBlock,
  jetbrainsDarculaTheme,
  jetbrainsLightTheme,
  ParsedValueView,
  type ContractData,
} from "@acton/shared-ui"
import {Cell} from "@ton/core"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {Check, CheckCircle2, Copy, ExternalLink, FileCode2, Folder, Menu} from "lucide-react"
import {createHighlighterCore} from "shiki/core"
import {createOnigurumaEngine} from "shiki/engine/oniguruma"
import type {LanguageRegistration} from "shiki/types"
import {Cell as Cell2, runtime, text} from "ton-assembly"

import type {TonClient} from "../api/client"
import type {SourceBundle, SourceFile, VerificationSourceResponse} from "../api/types"
import {useAddressFormat} from "../hooks/useNetworkInfo"
import {AbiPanel, type AbiTab} from "./abi-viewer"
import type {AddressFormatOptions} from "./utils"

import funcGrammarRaw from "../../../../../docs/grammars/grammar-func.json"
import tasmGrammarRaw from "../../../../../docs/grammars/grammar-tasm.json"
import tolkGrammarRaw from "../../../../../docs/grammars/grammar-tolk.json"

import styles from "./ContractCode.module.css"

interface ContractCodeProps {
  readonly codeBoc: string
  readonly ownerAddress: string
  readonly client: TonClient
  readonly dataBoc?: string
  readonly compilerAbi?: ContractABI
  readonly compilerAbiLoading?: boolean
  readonly compilerAbiError?: string
  readonly verifiedSource?: VerificationSourceResponse
  readonly verifiedSourceLoading?: boolean
  readonly onContractClick?: (address: string) => void
}

type ContractCodeTab = "storage" | "source" | "abi"
type StorageTab = "parsed" | "base64" | "hex" | "hex-hash" | "base64-hash"
type SourceTab = "verified" | "decompiled" | "base64" | "hex" | "hex-hash" | "base64-hash"
type HighlightLanguage = "tasm" | "json" | "tolk" | "func"

const grammarWithName = (grammar: unknown, name: string): LanguageRegistration =>
  ({
    ...(grammar as Record<string, unknown>),
    name,
  }) as LanguageRegistration

const tasmGrammar = grammarWithName(tasmGrammarRaw, "tasm")
const tolkGrammar = grammarWithName(tolkGrammarRaw, "tolk")
const funcGrammar = grammarWithName(funcGrammarRaw, "func")

let contractCodeHighlighterPromise: ReturnType<typeof createHighlighterCore> | undefined
const VERIFIER_BASE_URL = "https://verifier.acton.monster"

const getContractCodeHighlighter = () => {
  contractCodeHighlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tasmGrammar, tolkGrammar, funcGrammar, import("shiki/langs/json.mjs")],
    engine: createOnigurumaEngine(() => import("shiki/wasm")),
  })

  return contractCodeHighlighterPromise
}

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
  const [activeSourceTab, setActiveSourceTab] = useState<SourceTab>("verified")
  const [activeAbiTab, setActiveAbiTab] = useState<AbiTab>("view")
  const addressFormat = useAddressFormat()

  const codeData = useMemo(() => {
    if (!codeBoc) return
    try {
      const buf = Buffer.from(codeBoc, "base64")
      const cell = Cell2.fromBoc(buf)[0]
      const codeCell = Cell.fromBase64(codeBoc)
      const decompiled = text.print(runtime.decompileCell(cell))

      return {
        base64: codeBoc,
        codeHashBase64: codeCell.hash().toString("base64"),
        codeHashHex: codeCell.hash().toString("hex"),
        hex: buf.toString("hex").toUpperCase(),
        decompiled: decompiled,
      }
    } catch (error) {
      console.error("Failed to process contract code:", error)
      return {
        base64: codeBoc,
        codeHashBase64: "Error processing code hash",
        codeHashHex: "Error processing code hash",
        hex: "Error processing HEX",
        decompiled: "Error: Failed to decompile code.",
      }
    }
  }, [codeBoc])

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

  const handleContractTabChange = (tab: ContractCodeTab) => {
    setActiveTab(tab)
    writeContractHashTab(tab)
  }

  if (!codeBoc || !codeData) {
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
          <SourcePanel
            activeTab={activeSourceTab}
            onTabChange={setActiveSourceTab}
            codeData={codeData}
            verifiedSource={hasVerifiedSource ? verifiedSource : undefined}
          />
        )}
        {verifiedSourceLoading && !hasVerifiedSource && activeTab === "source" && (
          <div className={styles.verifiedLoading}>Checking verified source...</div>
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

function SourcePanel({
  activeTab,
  onTabChange,
  codeData,
  verifiedSource,
}: {
  readonly activeTab: SourceTab
  readonly onTabChange: (tab: SourceTab) => void
  readonly codeData: {
    readonly base64: string
    readonly codeHashBase64: string
    readonly codeHashHex: string
    readonly hex: string
    readonly decompiled: string
  }
  readonly verifiedSource?: VerificationSourceResponse
}): JSX.Element {
  const activeSourceTab = activeTab === "verified" && !verifiedSource ? "decompiled" : activeTab
  const sourceTabs: readonly {
    tab: SourceTab
    label: string
    verified?: boolean
  }[] = [
    ...(verifiedSource ? [{tab: "verified" as const, label: "Verified code", verified: true}] : []),
    {tab: "decompiled", label: "disasm"},
    {tab: "base64", label: "base64"},
    {tab: "hex", label: "hex"},
    {tab: "hex-hash", label: "hex hash"},
    {tab: "base64-hash", label: "base64 hash"},
  ]
  const activeSource =
    activeSourceTab === "verified"
      ? undefined
      : activeSourceTab === "decompiled"
        ? {
            title: "Disassembly",
            value: codeData.decompiled,
            language: "tasm" as const,
            wrap: false,
          }
        : activeSourceTab === "base64"
          ? {
              title: "Code BoC Base64",
              value: codeData.base64,
              wrap: true,
            }
          : activeSourceTab === "hex"
            ? {
                title: "Code BoC HEX",
                value: codeData.hex,
                wrap: true,
              }
            : activeSourceTab === "hex-hash"
              ? {
                  title: "Code hash HEX",
                  value: codeData.codeHashHex,
                  wrap: true,
                }
              : {
                  title: "Code hash Base64",
                  value: codeData.codeHashBase64,
                  wrap: true,
                }

  return (
    <section className={styles.sourceShell}>
      <div className={styles.editorTabBar}>
        {sourceTabs.map(item => (
          <button
            key={item.tab}
            type="button"
            className={`${styles.editorTab} ${item.verified ? styles.editorTabVerified : ""} ${
              activeSourceTab === item.tab ? styles.editorTabActive : ""
            }`}
            onClick={() => onTabChange(item.tab)}
          >
            {item.verified && <CheckCircle2 size={15} aria-hidden="true" />}
            {item.label}
          </button>
        ))}
      </div>
      {activeSourceTab === "verified" && verifiedSource ? (
        <VerifiedSourcePanel source={verifiedSource} />
      ) : activeSource ? (
        <ContractTextPanel
          title={activeSource.title}
          value={activeSource.value}
          language={activeSource.language}
          wrap={activeSource.wrap}
        />
      ) : undefined}
    </section>
  )
}

function VerifiedSourcePanel({source}: {readonly source: VerificationSourceResponse}): JSX.Element {
  const bundles = useMemo(
    () => source.bundles.filter(bundle => bundle.files.length > 0),
    [source.bundles],
  )
  const [selectedBundleHash, setSelectedBundleHash] = useState(bundles[0]?.source_bundle_hash ?? "")
  const activeBundle =
    bundles.find(bundle => bundle.source_bundle_hash === selectedBundleHash) ?? bundles[0]

  useEffect(() => {
    setSelectedBundleHash(bundles[0]?.source_bundle_hash ?? "")
  }, [bundles])

  if (!activeBundle) {
    return <div className={styles.empty}>No verified source files stored for this contract</div>
  }

  return (
    <section className={styles.verifiedShell}>
      {bundles.length > 1 && (
        <div className={styles.verifiedHeader}>
          <div className={styles.bundleTabs} role="tablist" aria-label="Verified source bundles">
            {bundles.map(bundle => (
              <button
                key={bundle.source_bundle_hash}
                type="button"
                className={`${styles.bundleTab} ${
                  bundle.source_bundle_hash === activeBundle.source_bundle_hash
                    ? styles.bundleTabActive
                    : ""
                }`}
                onClick={() => setSelectedBundleHash(bundle.source_bundle_hash)}
              >
                {shortenMiddle(bundle.source_bundle_hash, 8, 6)}
              </button>
            ))}
          </div>
        </div>
      )}
      <VerifiedCodeViewer
        bundle={activeBundle}
        verificationUrl={`${VERIFIER_BASE_URL}/${encodeURIComponent(source.code_hash)}`}
      />
    </section>
  )
}

function VerifiedCodeViewer({
  bundle,
  verificationUrl,
}: {
  readonly bundle: SourceBundle
  readonly verificationUrl: string
}): JSX.Element {
  const entrypointPath = useMemo(
    () => findEntrypointFile(bundle.files, bundle.entrypoint)?.path,
    [bundle.entrypoint, bundle.files],
  )
  const defaultActivePath = entrypointPath ?? bundle.files[0]?.path ?? ""
  const [activePath, setActivePath] = useState(defaultActivePath)
  const [isFileTreeOpen, setFileTreeOpen] = useState(false)

  useEffect(() => {
    setActivePath(defaultActivePath)
    setFileTreeOpen(false)
  }, [bundle.source_bundle_hash, defaultActivePath])

  const activeFile = useMemo(
    () =>
      findFileByPath(bundle.files, activePath) ??
      findFileByPath(bundle.files, entrypointPath) ??
      bundle.files[0],
    [activePath, bundle.files, entrypointPath],
  )
  const tree = useMemo(() => buildFileTree(bundle.files), [bundle.files])
  const code = activeFile ? fileContent(activeFile) : ""
  const language = activeFile ? languageForPath(activeFile.path) : undefined

  if (!activeFile) {
    return <div className={styles.empty}>No verified source files stored for this bundle</div>
  }

  const selectFile = (path: string) => {
    setActivePath(path)
    setFileTreeOpen(false)
  }

  return (
    <section className={styles.verifiedWorkspace} aria-label="Verified source code">
      <aside className={`${styles.fileTree} ${styles.fileTreeDesktop}`} aria-label="Source files">
        <div className={styles.fileTreeList}>
          <FileTreeRows
            nodes={tree}
            activePath={activeFile.path}
            entrypoint={entrypointPath}
            onSelect={selectFile}
          />
        </div>
      </aside>
      <div className={styles.codePane}>
        <div className={styles.codePaneHeader}>
          <button
            type="button"
            className={`${styles.mobileFileTreeToggle} ${
              isFileTreeOpen ? styles.mobileFileTreeToggleOpen : ""
            }`}
            aria-label="Toggle source files"
            aria-expanded={isFileTreeOpen}
            onClick={() => setFileTreeOpen(current => !current)}
          >
            <Menu size={16} aria-hidden="true" />
          </button>
          <span className={styles.codePanePath} title={activeFile.path}>
            {activeFile.path}
          </span>
          <a
            className={styles.verificationLink}
            href={verificationUrl}
            target="_blank"
            rel="noreferrer"
          >
            <ExternalLink size={13} aria-hidden="true" />
            View verification
          </a>
          <CopyTextButton
            className={styles.codePaneCopyButton}
            title={activeFile.path}
            value={code}
          />
        </div>
        <aside
          className={`${styles.fileTree} ${styles.fileTreeMobile} ${
            isFileTreeOpen ? styles.fileTreeOpen : ""
          }`}
          aria-label="Source files"
        >
          <div className={styles.fileTreeList}>
            <FileTreeRows
              nodes={tree}
              activePath={activeFile.path}
              entrypoint={entrypointPath}
              onSelect={selectFile}
            />
          </div>
        </aside>
        <div className={styles.codeFrame}>
          <div className={styles.lineNumbers} aria-hidden="true">
            {Array.from({length: lineCount(code)}, (_, index) => (
              <span key={index + 1}>{index + 1}</span>
            ))}
          </div>
          <div className={styles.verifiedCode}>
            <CodeContent value={code} language={language} wrap={false} />
          </div>
        </div>
      </div>
    </section>
  )
}

interface FileTreeNode {
  readonly kind: "folder" | "file"
  readonly name: string
  readonly path: string
  readonly children: readonly FileTreeNode[]
  readonly file?: SourceFile
}

interface FileTreeDraftNode {
  readonly kind: "folder" | "file"
  readonly name: string
  readonly path: string
  readonly children: Map<string, FileTreeDraftNode>
  readonly file?: SourceFile
}

function FileTreeRows({
  nodes,
  activePath,
  entrypoint,
  depth = 0,
  onSelect,
}: {
  readonly nodes: readonly FileTreeNode[]
  readonly activePath: string
  readonly entrypoint?: string
  readonly depth?: number
  readonly onSelect: (path: string) => void
}): JSX.Element {
  return (
    <>
      {nodes.map(node => {
        const depthStyle = {"--depth": String(depth)} as CSSProperties
        if (node.kind === "folder") {
          return (
            <div key={node.path}>
              <div className={`${styles.fileTreeRow} ${styles.fileTreeFolder}`} style={depthStyle}>
                <Folder size={14} aria-hidden="true" />
                <span>{node.name}</span>
              </div>
              <FileTreeRows
                nodes={node.children}
                activePath={activePath}
                entrypoint={entrypoint}
                depth={depth + 1}
                onSelect={onSelect}
              />
            </div>
          )
        }

        return (
          <button
            key={node.path}
            type="button"
            className={`${styles.fileTreeRow} ${styles.fileTreeFile} ${
              node.path === activePath ? styles.fileTreeRowActive : ""
            }`}
            style={depthStyle}
            title={node.path}
            aria-current={node.path === activePath ? "true" : undefined}
            onClick={() => onSelect(node.path)}
          >
            <FileCode2 size={14} aria-hidden="true" />
            <span>{node.name}</span>
            {node.path === entrypoint && <span className={styles.fileTreeEntrypoint}>main</span>}
          </button>
        )
      })}
    </>
  )
}

function fileContent(file: SourceFile): string {
  const content = file.content_text ?? Buffer.from(file.content_base64, "base64").toString("utf8")
  return content.endsWith("\n") ? content.slice(0, -1) : content
}

function languageForPath(path: string): HighlightLanguage | undefined {
  const normalizedPath = path.toLowerCase()
  if (normalizedPath.endsWith(".tolk")) {
    return "tolk"
  }
  if (normalizedPath.endsWith(".fc") || normalizedPath.endsWith(".func")) {
    return "func"
  }
  if (
    normalizedPath.endsWith(".json") ||
    normalizedPath.endsWith(".abi") ||
    normalizedPath.endsWith(".pkg")
  ) {
    return "json"
  }
  return undefined
}

function lineCount(code: string): number {
  return code.length === 0 ? 1 : code.split("\n").length
}

function normalizeFilePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.?\//, "")
}

function findFileByPath(
  files: readonly SourceFile[],
  path: string | undefined,
): SourceFile | undefined {
  if (!path) {
    return undefined
  }

  const normalizedPath = normalizeFilePath(path)
  return (
    files.find(file => file.path === path) ??
    files.find(file => normalizeFilePath(file.path) === normalizedPath)
  )
}

function findEntrypointFile(
  files: readonly SourceFile[],
  entrypoint: string | undefined,
): SourceFile | undefined {
  const exactMatch = findFileByPath(files, entrypoint)
  if (exactMatch || !entrypoint) {
    return exactMatch
  }

  const normalizedEntrypoint = normalizeFilePath(entrypoint)
  const suffix = `/${normalizedEntrypoint}`
  const suffixMatches = files.filter(file => normalizeFilePath(file.path).endsWith(suffix))
  return suffixMatches.length === 1 ? suffixMatches[0] : undefined
}

function buildFileTree(files: readonly SourceFile[]): readonly FileTreeNode[] {
  const root = new Map<string, FileTreeDraftNode>()

  for (const file of files) {
    const parts = normalizeFilePath(file.path).split("/").filter(Boolean)
    let currentLevel = root
    let currentPath = ""

    for (const [index, part] of parts.entries()) {
      currentPath = currentPath ? `${currentPath}/${part}` : part
      const isFile = index === parts.length - 1
      let node = currentLevel.get(part)
      if (!node) {
        node = {
          kind: isFile ? "file" : "folder",
          name: part,
          path: currentPath,
          children: new Map(),
        }
        currentLevel.set(part, node)
      }

      if (isFile) {
        node = {
          ...node,
          kind: "file",
          file,
        }
        currentLevel.set(part, node)
      }

      currentLevel = node.children
    }
  }

  return sortTree([...root.values()].map(node => freezeTree(node)))
}

function freezeTree(node: FileTreeDraftNode): FileTreeNode {
  return {
    kind: node.kind,
    name: node.name,
    path: node.path,
    children: sortTree([...node.children.values()].map(child => freezeTree(child))),
    file: node.file,
  }
}

function sortTree(nodes: readonly FileTreeNode[]): FileTreeNode[] {
  return [...nodes].sort((left, right) => {
    if (left.kind !== right.kind) {
      return left.kind === "folder" ? -1 : 1
    }
    return left.name.localeCompare(right.name)
  })
}

function shortenMiddle(value: string, prefix = 8, suffix = 6): string {
  if (value.length <= prefix + suffix + 1) {
    return value
  }
  return `${value.slice(0, prefix)}...${value.slice(-suffix)}`
}

function ContractTextPanel({
  title,
  value,
  language,
  wrap = false,
}: {
  readonly title: string
  readonly value: string
  readonly language?: HighlightLanguage
  readonly wrap?: boolean
}): JSX.Element {
  return (
    <DataBlock
      className={styles.sourceDataBlock}
      variant="standalone"
      copyLabel={title}
      copyValue={value}
    >
      <CodeContent value={value} language={language} wrap={wrap} />
    </DataBlock>
  )
}

function CopyTextButton({
  className,
  title,
  value,
}: {
  readonly className: string
  readonly title: string
  readonly value: string
}): JSX.Element {
  const [isCopied, setIsCopied] = useState(false)

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 1600)
    return () => clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={className}
      onClick={() => {
        void navigator.clipboard.writeText(value)
        setIsCopied(true)
      }}
      aria-label={isCopied ? `${title} copied` : `Copy ${title}`}
      title={isCopied ? "Copied" : `Copy ${title}`}
    >
      {isCopied ? <Check size={14} /> : <Copy size={14} />}
    </button>
  )
}

function CodeContent({
  value,
  language,
  wrap,
}: {
  readonly value: string
  readonly language?: HighlightLanguage
  readonly wrap: boolean
}): JSX.Element {
  if (language) {
    return <HighlightedCode value={value} language={language} wrap={wrap} />
  }

  return (
    <pre className={`${styles.code} ${wrap ? styles.codeWrap : ""}`}>
      <code>{value}</code>
    </pre>
  )
}

function HighlightedCode({
  value,
  language,
  wrap,
}: {
  readonly value: string
  readonly language: HighlightLanguage
  readonly wrap: boolean
}): JSX.Element {
  const [highlightedHtml, setHighlightedHtml] = useState<string | undefined>()

  useEffect(() => {
    let isActive = true

    const highlight = async () => {
      setHighlightedHtml(undefined)
      try {
        const highlighter = await getContractCodeHighlighter()
        const isDark = document.documentElement.classList.contains("dark-theme")
        const html = highlighter.codeToHtml(value, {
          lang: language,
          theme: isDark ? "jetbrains-darcula" : "jetbrains-light",
        })

        if (isActive) {
          setHighlightedHtml(html)
        }
      } catch (error) {
        console.error("Failed to highlight contract code:", error)
        if (isActive) {
          setHighlightedHtml(undefined)
        }
      }
    }

    void highlight()

    const observer = new MutationObserver(mutations => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "class") {
          void highlight()
        }
      }
    })
    observer.observe(document.documentElement, {attributes: true})

    return () => {
      isActive = false
      observer.disconnect()
    }
  }, [language, value])

  if (!highlightedHtml) {
    return (
      <pre className={`${styles.code} ${wrap ? styles.codeWrap : ""}`}>
        <code>{value}</code>
      </pre>
    )
  }

  return (
    <div
      className={`${styles.highlightedCode} ${wrap ? styles.highlightedCodeWrap : ""}`}
      dangerouslySetInnerHTML={{__html: highlightedHtml}}
    />
  )
}
