import {
  Checkbox,
  CopyInlineAction,
  HighlightedCode,
  Input,
  ParsedValueView,
  PillTab,
  PillTabs,
  RawDataBlock,
  SkeletonText,
} from "@acton/ui"
import {
  CodeCellDetails,
  disassembleBocHex,
  type ContractVerifiedSource,
} from "@acton/transaction-ui"
import {CircleAlert} from "lucide-react"
import {useCallback, useDeferredValue, useEffect, useRef, useState, type FC} from "react"

import type {ExtendedContractABI} from "../api/compilerAbi"
import {getBundledCompilerAbiCatalog} from "../api/compilerAbiCatalog"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {type ExplorerNavigationClickEvent, useOpenExplorerPath} from "../hooks/useOpenExplorerPath"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import type {ParserWarning, SerializableValue} from "../cell-inspector/model"
import type {CellInspectorParseResult} from "../cell-inspector/parseCell"

import styles from "./CellInspectorPage.module.css"

type OutputTab = "parsed" | "raw" | "code" | "boc"
type ParsedInspectionResult = Exclude<CellInspectorParseResult, {readonly status: "error"}>

interface AbiCandidate {
  readonly abi: ExtendedContractABI
}

type InspectionState =
  | {readonly status: "idle"}
  | {readonly status: "loading"; readonly previous?: CellInspectorParseResult}
  | {readonly status: "ready"; readonly result: CellInspectorParseResult}

interface AbiResolution {
  readonly abi?: ExtendedContractABI
  readonly codeHash?: string
  readonly warning?: ParserWarning
  readonly confidenceScore?: number
  readonly confidenceReason?: string
}

interface DisassemblyState {
  readonly source?: string
  readonly codeHash?: string
  readonly loading: boolean
  readonly available?: boolean
  readonly error?: string
  readonly verifiedSource?: ContractVerifiedSource
}

interface CellInspectorDraft {
  readonly input: string
  readonly rootIndex: number
  readonly strict: boolean
  readonly maxDepth: number
  readonly customTlb: string
  readonly customTlbEnabled: boolean
}

const CODE_OUTPUT_TAB = {id: "code", label: "TVM code"} as const
const OUTPUT_TABS: readonly {
  readonly id: OutputTab
  readonly label: string
}[] = [
  {id: "parsed", label: "Parsed"},
  {id: "raw", label: "Raw cells"},
  CODE_OUTPUT_TAB,
  {id: "boc", label: "BoC"},
]

const CELL_INSPECTOR_DRAFT_KEY = "acton:cell-inspector:draft"
const MAX_CELL_QUERY_LENGTH = 4096
const EMPTY_CELL_INSPECTOR_DRAFT: CellInspectorDraft = {
  input: "",
  rootIndex: 0,
  strict: false,
  maxDepth: 8,
  customTlb: "",
  customTlbEnabled: false,
}

export const CellInspectorPage: FC = () => {
  const metadataRegistry = useMetadataRegistry()
  const resolveVerifiedSourceByCodeHash = useCallback(
    async (codeHash: string): Promise<ContractVerifiedSource | undefined> => {
      try {
        const source = await metadataRegistry.getSource({codeHash})
        return source.verified && source.bundle ? source : undefined
      } catch {
        return undefined
      }
    },
    [metadataRegistry],
  )
  const [initialDraft] = useState(() => readCellInspectorDraft(readCellQuery()))
  const [input, setInput] = useState(initialDraft.input)
  const [rootIndex, setRootIndex] = useState(initialDraft.rootIndex)
  const [strict, setStrict] = useState(initialDraft.strict)
  const [maxDepth, setMaxDepth] = useState(initialDraft.maxDepth)
  const [customTlb, setCustomTlb] = useState(initialDraft.customTlb)
  const [customTlbEnabled, setCustomTlbEnabled] = useState(initialDraft.customTlbEnabled)
  const [abiCandidates, setAbiCandidates] = useState<readonly AbiCandidate[]>([])
  const [activeTab, setActiveTab] = useState<OutputTab>("parsed")
  const [inspection, setInspection] = useState<InspectionState>({
    status: "idle",
  })
  const [disassembly, setDisassembly] = useState<DisassemblyState>({
    loading: false,
  })
  const parseSequence = useRef(0)
  const deferredInput = useDeferredValue(input)

  useEffect(() => {
    const timer = globalThis.setTimeout(() => {
      writeCellInspectorDraft({
        input,
        rootIndex,
        strict,
        maxDepth,
        customTlb,
        customTlbEnabled,
      })
    }, 200)

    return () => globalThis.clearTimeout(timer)
  }, [customTlb, customTlbEnabled, input, maxDepth, rootIndex, strict])

  useEffect(() => {
    let active = true

    Promise.all([
      getBundledCompilerAbiCatalog(),
      metadataRegistry.listCompilerAbis().catch(() => []),
    ]).then(([bundled, registered]) => {
      if (!active) return

      const candidates = new Map<string, ExtendedContractABI>()
      for (const entry of registered) {
        candidates.set(compilerAbiIdentity(entry.abi), entry.abi)
      }
      for (const entry of bundled) {
        candidates.set(compilerAbiIdentity(entry), entry)
      }

      setAbiCandidates([...candidates.values()].map(abi => ({abi})))
    })

    return () => {
      active = false
    }
  }, [metadataRegistry])

  useEffect(() => {
    const trimmedInput = deferredInput.trim()
    const sequence = ++parseSequence.current
    if (!trimmedInput) {
      setInspection({status: "idle"})
      replaceCellQuery()
      return
    }

    setInspection(current => ({
      status: "loading",
      ...(current.status === "ready" ? {previous: current.result} : {}),
    }))

    const timer = globalThis.setTimeout(() => {
      void inspectCell({
        input: trimmedInput,
        rootIndex,
        strict,
        maxDepth,
        customTlb,
        customTlbEnabled,
        metadataRegistry,
        abiCandidates,
      })
        .then(result => {
          if (parseSequence.current === sequence) {
            setInspection({status: "ready", result})
            replaceCellQuery(cellQueryValue(result))
          }
        })
        .catch(error => {
          if (parseSequence.current === sequence) {
            setInspection({
              status: "ready",
              result: {
                status: "error",
                error: {
                  code: "inspection-failed",
                  message: "Cell inspection failed",
                  cause: error instanceof Error ? error.message : String(error),
                },
                warnings: [],
              },
            })
            replaceCellQuery()
          }
        })
    }, 140)

    return () => globalThis.clearTimeout(timer)
  }, [
    abiCandidates,
    customTlb,
    customTlbEnabled,
    deferredInput,
    maxDepth,
    metadataRegistry,
    rootIndex,
    strict,
  ])

  const result =
    inspection.status === "ready"
      ? inspection.result
      : inspection.status === "loading"
        ? inspection.previous
        : undefined
  const rootCount = result?.cell?.rootCount
  const selectedRootBocHex = result?.selectedRootBocHex
  const disassemblySource = selectedRootBocHex
  const selectedRootCodeHash = result?.cell?.hash
  const resultConfidenceLevel =
    result && result.status !== "error" ? result.provenance.confidence.level : undefined

  useEffect(() => {
    if (!disassemblySource) {
      setDisassembly({loading: false})
      return
    }

    let active = true
    const source = disassemblySource
    const codeHash = selectedRootCodeHash
    const preferCode = resultConfidenceLevel === "low"
    setDisassembly({source, codeHash, loading: true, available: false})
    void disassembleBocHex(source)
      .then(async value => {
        if (!active) return

        const disassembled = value.disasm.trim()
        const available = !value.isEmptyCell && !value.isEmbeddedData && disassembled.length > 0
        setDisassembly({source, codeHash, loading: false, available})
        if (available && preferCode) {
          setActiveTab(current => (current === "parsed" ? "code" : current))
        }
        if (!available || !codeHash) return

        const verifiedSource = await resolveVerifiedSourceByCodeHash(codeHash)
        if (!active || !verifiedSource) return

        setDisassembly(current =>
          current.source === source ? {...current, verifiedSource} : current,
        )
        setActiveTab(current => (current === "parsed" ? "code" : current))
      })
      .catch(error => {
        if (active) {
          setDisassembly({
            source,
            codeHash,
            loading: false,
            available: false,
            error: error instanceof Error ? error.message : String(error),
          })
        }
      })

    return () => {
      active = false
    }
  }, [
    disassemblySource,
    resolveVerifiedSourceByCodeHash,
    resultConfidenceLevel,
    selectedRootCodeHash,
  ])

  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Cell Inspector"}]} />
      <header className={styles.hero}>
        <div>
          <h1 className={styles.title}>Cell Inspector</h1>
        </div>
      </header>

      <div className={styles.workspace}>
        <CellInspectorInputPanel
          input={input}
          onInputChange={value => {
            replaceCellQuery()
            setInput(value)
            setRootIndex(0)
            setActiveTab("parsed")
          }}
          rootIndex={rootIndex}
          rootCount={rootCount}
          onRootIndexChange={value => {
            setRootIndex(value)
            setActiveTab("parsed")
          }}
          strict={strict}
          onStrictChange={setStrict}
          maxDepth={maxDepth}
          onMaxDepthChange={setMaxDepth}
          customTlb={customTlb}
          customTlbEnabled={customTlbEnabled}
          onCustomTlbChange={setCustomTlb}
          onCustomTlbEnabledChange={setCustomTlbEnabled}
        />

        <section className={styles.outputPanel} aria-live="polite">
          {inspection.status === "idle" ? (
            <EmptyOutput />
          ) : inspection.status === "loading" && !result ? (
            <LoadingOutput />
          ) : result ? (
            <ResultOutput
              result={result}
              loading={inspection.status === "loading"}
              activeTab={activeTab}
              onTabChange={setActiveTab}
              disassembly={disassembly}
              resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
            />
          ) : null}
        </section>
      </div>
    </section>
  )
}

interface CellInspectorInputPanelProps {
  readonly input: string
  readonly onInputChange: (value: string) => void
  readonly rootIndex: number
  readonly rootCount?: number
  readonly onRootIndexChange: (value: number) => void
  readonly strict: boolean
  readonly onStrictChange: (value: boolean) => void
  readonly maxDepth: number
  readonly onMaxDepthChange: (value: number) => void
  readonly customTlb: string
  readonly customTlbEnabled: boolean
  readonly onCustomTlbChange: (value: string) => void
  readonly onCustomTlbEnabledChange: (value: boolean) => void
}

const CellInspectorInputPanel: FC<CellInspectorInputPanelProps> = ({
  input,
  onInputChange,
  rootIndex,
  rootCount,
  onRootIndexChange,
  strict,
  onStrictChange,
  maxDepth,
  onMaxDepthChange,
  customTlb,
  customTlbEnabled,
  onCustomTlbChange,
  onCustomTlbEnabledChange,
}) => (
  <section className={styles.inputPanel}>
    <label className={`${styles.textareaField} ${styles.cellField}`} htmlFor="cell-inspector-input">
      <span className={`${styles.fieldLabel} ${styles.cellFieldLabel}`}>Cell</span>
      <span className={styles.fieldHint}>Paste Base64, hex, a ton:// URL, or an explorer link</span>
      <textarea
        id="cell-inspector-input"
        className={styles.cellInput}
        value={input}
        onChange={event => onInputChange(event.target.value)}
        placeholder="te6cc… or b5ee9c72…"
        spellCheck={false}
        autoCapitalize="off"
        autoComplete="off"
      />
    </label>

    <div className={styles.optionsGrid}>
      <Input
        className={styles.numberInput}
        label="Root"
        type="number"
        min={0}
        max={rootCount === undefined ? undefined : Math.max(0, rootCount - 1)}
        disabled={rootCount === 1}
        value={rootIndex}
        onChange={event => onRootIndexChange(nonNegativeInteger(event.target.value, 0))}
        description={
          rootCount === undefined
            ? "0-based index"
            : `${rootCount} ${rootCount === 1 ? "root" : "roots"} available`
        }
      />
      <Input
        className={styles.numberInput}
        label="Tree depth"
        type="number"
        min={0}
        max={128}
        value={maxDepth}
        onChange={event => onMaxDepthChange(boundedInteger(event.target.value, 8, 0, 128))}
        description="Raw depth limit"
      />
    </div>

    <Checkbox
      className={styles.strictOption}
      label="Strict parsing"
      description="Require full cell consumption"
      checked={strict}
      onChange={event => onStrictChange(event.currentTarget.checked)}
    />

    <div className={styles.customTlbSection}>
      <Checkbox
        label="Use custom TL-B schema"
        description="Ignore ABI and automatic detection"
        checked={customTlbEnabled}
        onChange={event => onCustomTlbEnabledChange(event.currentTarget.checked)}
      />
      {customTlbEnabled && (
        <label className={styles.textareaField} htmlFor="cell-inspector-custom-tlb">
          <span className={styles.fieldLabel}>Schema</span>
          <span className={styles.fieldHint}>Applied to the selected root</span>
          <textarea
            id="cell-inspector-custom-tlb"
            aria-label="Custom TL-B schema"
            className={styles.tlbInput}
            value={customTlb}
            onChange={event => onCustomTlbChange(event.target.value)}
            placeholder="message#1234 value:uint32 = Message;"
            spellCheck={false}
          />
        </label>
      )}
    </div>
  </section>
)

async function inspectCell({
  input,
  rootIndex,
  strict,
  maxDepth,
  customTlb,
  customTlbEnabled,
  metadataRegistry,
  abiCandidates,
}: {
  readonly input: string
  readonly rootIndex: number
  readonly strict: boolean
  readonly maxDepth: number
  readonly customTlb: string
  readonly customTlbEnabled: boolean
  readonly metadataRegistry: ReturnType<typeof useMetadataRegistry>
  readonly abiCandidates: readonly AbiCandidate[]
}): Promise<CellInspectorParseResult> {
  const parser = await import("../cell-inspector")
  const parseOptions = {
    rootIndex,
    strict,
    maxDepth,
    customTlb: customTlbEnabled ? customTlb : "",
    customTlbAuthoritative: customTlbEnabled,
  } as const
  const preliminaryResult = parser.parseCell(input, parseOptions)
  if (preliminaryResult.status === "error") return preliminaryResult

  if (
    preliminaryResult.parser === "custom-tlb" ||
    preliminaryResult.parser === "standard-comment"
  ) {
    return preliminaryResult
  }

  const normalized = parser.decodeCellInput(input, {rootIndex})
  const candidates = normalized.ok
    ? parser.collectCellHashCandidates([normalized.decoded.selectedRoot])
    : []
  let resolution = await resolveAbi({metadataRegistry, candidates})
  if (!resolution.abi && normalized.ok) {
    resolution = parser.inferAbiByOpcode(normalized.decoded.selectedRoot, abiCandidates)
  }
  const result = resolution.abi
    ? parser.parseCell(input, {
        ...parseOptions,
        abi: resolution.abi,
        abiCodeHash: resolution.codeHash,
        warnOnAbiMismatch: false,
        ...(resolution.confidenceScore === undefined
          ? {}
          : {
              abiConfidence: {
                score: resolution.confidenceScore,
                reason:
                  resolution.confidenceReason ?? "The ABI was inferred without contract context",
              },
            }),
      })
    : preliminaryResult

  if (!resolution.warning || result.status === "error" || result.parser !== "abi-registry") {
    return result
  }
  if (result.status === "success") {
    return {
      ...result,
      status: "partial",
      warnings: [...result.warnings, resolution.warning],
    }
  }
  return {
    ...result,
    warnings: [...result.warnings, resolution.warning],
  }
}

async function resolveAbi({
  metadataRegistry,
  candidates,
}: {
  readonly metadataRegistry: ReturnType<typeof useMetadataRegistry>
  readonly candidates: readonly {
    readonly hash: string
    readonly path: string
  }[]
}): Promise<AbiResolution> {
  const codeHashes = candidates.map(candidate => candidate.hash)
  if (codeHashes.length === 0) return {}
  const chunkSize = 16
  for (let index = 0; index < codeHashes.length; index += chunkSize) {
    const chunk = codeHashes.slice(index, index + chunkSize)
    const abis = await withTimeout(
      metadataRegistry.getCompilerAbis(chunk),
      ABI_LOOKUP_TIMEOUT_MS,
    ).catch((): Record<string, ExtendedContractABI | null> => ({}))
    const matches = chunk.flatMap(codeHash => {
      const abi = abis[codeHash]
      return abi ? [{abi, codeHash}] : []
    })
    const selected = matches[0]
    if (selected) {
      const distinctAbis = new Set(matches.map(match => compilerAbiIdentity(match.abi)))
      return {
        ...selected,
        ...(distinctAbis.size > 1
          ? {
              warning: {
                code: "ambiguous-match" as const,
                message: `Several contract ABIs match this cell tree. Showing ${abiDisplayName(selected.abi)}`,
              },
            }
          : {}),
      }
    }
  }

  return {}
}

function compilerAbiIdentity(abi: ExtendedContractABI): string {
  return JSON.stringify({
    name: abi.compiler_abi.contract_name,
    codeHashes: abi.code_hashes.toSorted(),
  })
}

const ABI_LOOKUP_TIMEOUT_MS = 5000

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timeout = globalThis.setTimeout(
      () => reject(new Error(`Timed out after ${timeoutMs}ms`)),
      timeoutMs,
    )
    promise.then(
      value => {
        globalThis.clearTimeout(timeout)
        resolve(value)
      },
      error => {
        globalThis.clearTimeout(timeout)
        reject(error)
      },
    )
  })
}

function ResultOutput({
  result,
  loading,
  activeTab,
  onTabChange,
  disassembly,
  resolveVerifiedSourceByCodeHash,
}: {
  readonly result: CellInspectorParseResult
  readonly loading: boolean
  readonly activeTab: OutputTab
  readonly onTabChange: (tab: OutputTab) => void
  readonly disassembly: DisassemblyState
  readonly resolveVerifiedSourceByCodeHash: (
    codeHash: string,
  ) => Promise<ContractVerifiedSource | undefined>
}) {
  if (result.status === "error") {
    return <ErrorOutput message={result.error.message} cause={result.error.cause} />
  }

  const confidence = result.provenance.confidence
  const codeTabAvailable =
    disassembly.source === result.selectedRootBocHex && disassembly.available === true
  const verifiedCodeAvailable =
    codeTabAvailable &&
    disassembly.source === result.selectedRootBocHex &&
    disassembly.verifiedSource !== undefined
  const orderedTabs =
    codeTabAvailable && (confidence.level === "low" || verifiedCodeAvailable)
      ? [CODE_OUTPUT_TAB, ...OUTPUT_TABS.filter(tab => tab.id !== "code")]
      : OUTPUT_TABS
  const outputTabs = orderedTabs.filter(tab => tab.id !== "code" || codeTabAvailable)
  const codeTabStatus =
    disassembly.source !== result.selectedRootBocHex || disassembly.loading
      ? "loading"
      : disassembly.available
        ? "available"
        : "unavailable"
  const visibleActiveTab = outputTabs.some(tab => tab.id === activeTab) ? activeTab : "parsed"
  const visibleWarnings = verifiedCodeAvailable
    ? result.warnings.filter(
        warning => warning.code !== "partial-match" && warning.code !== "ambiguous-match",
      )
    : result.warnings
  return (
    <>
      <header className={styles.resultHeader} data-loading={loading || undefined}>
        <div className={styles.provenance}>
          <div className={styles.provenanceTitle}>
            {verifiedCodeAvailable ? "Verified contract code" : result.provenance.label}
          </div>
          <div className={styles.provenanceMeta}>
            {verifiedCodeAvailable
              ? "Verified source"
              : provenanceSourceLabel(result.provenance.source)}
            <span aria-hidden="true">·</span>
            <span
              className={styles.confidence}
              data-level={verifiedCodeAvailable ? "exact" : confidence.level}
            >
              {verifiedCodeAvailable
                ? "exact · 100%"
                : `${confidence.level} · ${Math.round(confidence.score * 100)}%`}
            </span>
          </div>
        </div>
        {result.cell && (
          <dl className={styles.cellSummary}>
            <SummaryItem
              label="Root"
              value={`${result.cell.rootIndex + 1}/${result.cell.rootCount}`}
            />
            <SummaryItem label="Bits" value={String(result.cell.bits)} />
            <SummaryItem label="Refs" value={String(result.cell.refs)} />
            <SummaryItem label="Depth" value={String(result.cell.depth)} />
          </dl>
        )}
      </header>

      {result.cell && (
        <div className={styles.hashRow}>
          <span className={styles.hashLabel}>Root hash</span>
          <code className={styles.hashValue}>{result.cell.hash}</code>
          <CopyInlineAction
            value={result.cell.hash}
            label="Copy root hash"
            copiedLabel="Copied root hash"
            size="compact"
          />
        </div>
      )}

      {visibleWarnings.length > 0 && (
        <div className={styles.warnings} role="status">
          {visibleWarnings.map(warning => (
            <div className={styles.warning} key={`${warning.code}-${warning.message}`}>
              <CircleAlert size={16} aria-hidden="true" />
              <span>{warning.message}</span>
            </div>
          ))}
        </div>
      )}

      <PillTabs
        className={styles.outputTabs}
        ariaLabel="Cell Inspector output"
        data-tvm-code-status={codeTabStatus}
      >
        {outputTabs.map(tab => (
          <PillTab
            key={tab.id}
            selected={visibleActiveTab === tab.id}
            onClick={() => onTabChange(tab.id)}
          >
            {tab.label}
          </PillTab>
        ))}
      </PillTabs>

      <div className={styles.outputContent}>
        {visibleActiveTab === "parsed" && <ParsedOutput result={result} />}
        {visibleActiveTab === "raw" && (
          <CodeBlock
            title="Raw cell structure"
            value={stringifyValue(result.raw)}
            copyLabel="raw cell structure"
          />
        )}
        {visibleActiveTab === "code" && (
          <CodeOutput
            state={disassembly}
            resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
          />
        )}
        {visibleActiveTab === "boc" && <BocOutput result={result} />}
      </div>
    </>
  )
}

function ParsedOutput({result}: {readonly result: ParsedInspectionResult}) {
  const routes = useExplorerRoutePaths()
  const openExplorerPath = useOpenExplorerPath()
  const handleContractClick = useCallback(
    (address: string, event?: ExplorerNavigationClickEvent) => {
      openExplorerPath(routes.addressPath(address), event)
    },
    [openExplorerPath, routes],
  )

  if (result.abiValue) {
    return (
      <div className={styles.parsedValue}>
        <ParsedValueView value={result.abiValue} onContractClick={handleContractClick} />
      </div>
    )
  }

  const textComment = readTextComment(result.data)
  return (
    <div className={styles.parsedOutput}>
      {textComment && (
        <RawDataBlock title="Text" value={textComment} copyLabel="text comment" maxHeight={180} />
      )}
      <CodeBlock
        title="Decoded value"
        value={stringifyValue(result.data)}
        copyLabel="decoded value"
      />
    </div>
  )
}

function CodeBlock({
  title,
  value,
  copyLabel,
}: {
  readonly title: string
  readonly value: string
  readonly copyLabel: string
}) {
  return (
    <RawDataBlock
      title={title}
      value={value}
      copyLabel={copyLabel}
      maxHeight={560}
      customContent={<HighlightedCode value={value} language="json" maxHeight={560} />}
    />
  )
}

function CodeOutput({
  state,
  resolveVerifiedSourceByCodeHash,
}: {
  readonly state: DisassemblyState
  readonly resolveVerifiedSourceByCodeHash: (
    codeHash: string,
  ) => Promise<ContractVerifiedSource | undefined>
}) {
  if (state.loading) {
    return (
      <div className={styles.outputLoading}>
        <SkeletonText lineCount={8} />
      </div>
    )
  }
  if (state.error) {
    return <ErrorOutput message="This cell could not be read as TVM code" cause={state.error} />
  }
  if (!state.source) {
    return <div className={styles.outputEmpty}>No TVM instructions found in this root cell</div>
  }

  return (
    <CodeCellDetails
      cell={{bocHex: state.source, fieldName: "Cell Inspector root"}}
      verifiedSourcesByCodeHash={
        state.codeHash && state.verifiedSource
          ? new Map([[state.codeHash, state.verifiedSource]])
          : undefined
      }
      resolveVerifiedSourceByCodeHash={resolveVerifiedSourceByCodeHash}
    />
  )
}

function BocOutput({result}: {readonly result: ParsedInspectionResult}) {
  return (
    <div className={styles.bocList}>
      <RawDataBlock
        title="Base64"
        value={result.bocBase64 ?? ""}
        copyLabel="base64 BoC"
        maxHeight={180}
      />
      <RawDataBlock title="Hex" value={result.bocHex ?? ""} copyLabel="hex BoC" maxHeight={260} />
    </div>
  )
}

function EmptyOutput() {
  return (
    <div className={styles.emptyOutput}>
      <div className={styles.emptyTitle}>Paste a cell or BoC to inspect it</div>
      <p>
        Known ABIs and TON formats are detected automatically, and the original cell data is always
        available for comparison
      </p>
    </div>
  )
}

function LoadingOutput() {
  return (
    <div className={styles.loadingOutput} role="status" aria-label="Inspecting cell">
      <SkeletonText lineCount={6} />
    </div>
  )
}

function ErrorOutput({message, cause}: {readonly message: string; readonly cause?: string}) {
  return (
    <div className={styles.errorOutput} role="alert">
      <CircleAlert size={20} aria-hidden="true" />
      <div>
        <div className={styles.errorTitle}>{message}</div>
        {cause && <div className={styles.errorCause}>{cause}</div>}
      </div>
    </div>
  )
}

function SummaryItem({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  )
}

function provenanceSourceLabel(source: string): string {
  switch (source) {
    case "abi-registry":
      return "Contract ABI"
    case "user-schema":
      return "Custom TL-B"
    case "ton-standard":
      return "TON comment"
    case "canonical-block-tlb":
      return "TON block format"
    default:
      return "Raw cell data"
  }
}

function abiDisplayName(abi: ExtendedContractABI): string {
  return abi.display_name || abi.compiler_abi.contract_name || "Unnamed ABI"
}

function stringifyValue(value: SerializableValue | undefined): string {
  return value === undefined ? "" : JSON.stringify(value, null, 2)
}

function readTextComment(value: SerializableValue): string | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined
  const record = value as Readonly<Record<string, SerializableValue>>
  return record.kind === "text-comment" && typeof record.text === "string" ? record.text : undefined
}

function readCellQuery(): string | null {
  if (typeof globalThis.location === "undefined") return null
  return new URLSearchParams(globalThis.location.search).get("cell")
}

function cellQueryValue(result: CellInspectorParseResult): string | undefined {
  return result.status !== "error" &&
    result.bocBase64 &&
    result.bocBase64.length <= MAX_CELL_QUERY_LENGTH
    ? result.bocBase64
    : undefined
}

function replaceCellQuery(cell?: string): void {
  if (typeof globalThis.location === "undefined" || typeof globalThis.history === "undefined") {
    return
  }

  const url = new URL(globalThis.location.href)
  if ((url.searchParams.get("cell") ?? undefined) === cell) return

  if (cell) {
    url.searchParams.set("cell", cell)
  } else {
    url.searchParams.delete("cell")
  }
  globalThis.history.replaceState(
    globalThis.history.state,
    "",
    `${url.pathname}${url.search}${url.hash}`,
  )
}

function readCellInspectorDraft(urlCell: string | null = null): CellInspectorDraft {
  try {
    const raw = globalThis.localStorage?.getItem(CELL_INSPECTOR_DRAFT_KEY)
    if (!raw) {
      return urlCell === null
        ? EMPTY_CELL_INSPECTOR_DRAFT
        : {...EMPTY_CELL_INSPECTOR_DRAFT, input: urlCell}
    }
    const value = JSON.parse(raw) as unknown
    if (!isRecord(value)) {
      return urlCell === null
        ? EMPTY_CELL_INSPECTOR_DRAFT
        : {...EMPTY_CELL_INSPECTOR_DRAFT, input: urlCell}
    }

    return {
      input: urlCell ?? (typeof value.input === "string" ? value.input : ""),
      rootIndex: boundedInteger(String(value.rootIndex ?? ""), 0, 0, Number.MAX_SAFE_INTEGER),
      strict: typeof value.strict === "boolean" ? value.strict : false,
      maxDepth: boundedInteger(String(value.maxDepth ?? ""), 8, 0, 128),
      customTlb: typeof value.customTlb === "string" ? value.customTlb : "",
      customTlbEnabled:
        typeof value.customTlbEnabled === "boolean"
          ? value.customTlbEnabled
          : typeof value.customTlbVisible === "boolean"
            ? value.customTlbVisible
            : false,
    }
  } catch {
    return urlCell === null
      ? EMPTY_CELL_INSPECTOR_DRAFT
      : {...EMPTY_CELL_INSPECTOR_DRAFT, input: urlCell}
  }
}

function writeCellInspectorDraft(draft: CellInspectorDraft): void {
  try {
    globalThis.localStorage?.setItem(CELL_INSPECTOR_DRAFT_KEY, JSON.stringify(draft))
  } catch {
    // Storage can be unavailable or full. Inspection itself must keep working.
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

function nonNegativeInteger(value: string, fallback: number): number {
  return boundedInteger(value, fallback, 0, Number.MAX_SAFE_INTEGER)
}

function boundedInteger(value: string, fallback: number, min: number, max: number): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, Math.trunc(parsed)))
}
