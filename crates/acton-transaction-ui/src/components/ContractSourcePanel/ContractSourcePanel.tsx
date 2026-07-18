import {useEffect, useMemo, useState} from "react"
import type {JSX} from "react"

import {Cell} from "@ton/core"
import {Cell as TasmCell, runtime, text} from "@ton/tasm"
import {CheckCircle2} from "lucide-react"
import {CodeViewer, HighlightedCode, RawDataBlock} from "@acton/ui"

import styles from "./ContractSourcePanel.module.css"

type ContractSourceBuffer = Parameters<typeof TasmCell.fromBoc>[0] & {
  toString(encoding: "base64" | "hex" | "utf8"): string
}

declare const Buffer: {
  from(value: string, encoding: "base64"): ContractSourceBuffer
}

export type ContractSourceTab =
  | "verified"
  | "decompiled"
  | "base64"
  | "hex"
  | "hex-hash"
  | "base64-hash"

export interface ContractVerifiedSource {
  readonly code_hash: string
  readonly verified: boolean
  readonly bundles: readonly SourceBundle[]
}

interface SourceBundle {
  readonly source_bundle_hash: string
  readonly verified_at: number
  readonly storage_revision: string
  readonly entrypoint: string
  readonly compiler: CompilerMetadata
  readonly files: readonly SourceFile[]
}

interface CompilerMetadata {
  readonly language: string
  readonly version: string
  readonly params: unknown
}

interface SourceFile {
  readonly path: string
  readonly content_hash: string
  readonly include_in_command: boolean | null
  readonly is_stdlib: boolean | null
  readonly has_include_directives: boolean | null
  readonly content: string
}

interface ContractCodeData {
  readonly base64: string
  readonly codeHashBase64: string
  readonly codeHashHex: string
  readonly hex: string
  readonly decompiled: string
}

interface ContractSourcePanelProps {
  readonly codeBoc: string
  readonly defaultFileTreeVisible?: boolean
  readonly verifiedSource?: ContractVerifiedSource
  readonly verifiedSourceLoading?: boolean
  readonly verificationUrl?: string
  readonly verificationExternal?: boolean
  readonly compact?: boolean
}

const VERIFIER_BASE_URL = "https://verifier.acton.monster"

export function ContractSourcePanel({
  codeBoc,
  defaultFileTreeVisible = true,
  verifiedSource,
  verifiedSourceLoading = false,
  verificationUrl,
  verificationExternal = true,
  compact = false,
}: ContractSourcePanelProps): JSX.Element {
  const [activeTab, setActiveTab] = useState<ContractSourceTab>("verified")
  const codeData = useMemo(() => buildContractCodeData(codeBoc), [codeBoc])
  const resolvedVerifiedSource =
    verifiedSource?.verified && verifiedSource.bundles.length > 0 ? verifiedSource : undefined

  if (!codeData) {
    return (
      <div className={`${styles.empty} ${styles.panelEmpty}`}>Code cell could not be decoded</div>
    )
  }

  return (
    <>
      <SourcePanel
        activeTab={activeTab}
        onTabChange={setActiveTab}
        codeData={codeData}
        defaultFileTreeVisible={defaultFileTreeVisible}
        verifiedSource={resolvedVerifiedSource}
        verificationUrl={verificationUrl}
        verificationExternal={verificationExternal}
        compact={compact}
      />
      {verifiedSourceLoading && !resolvedVerifiedSource && (
        <div className={styles.verifiedLoading}>Checking verified source...</div>
      )}
    </>
  )
}

function buildContractCodeData(codeBoc: string): ContractCodeData | undefined {
  if (!codeBoc.trim()) {
    return undefined
  }

  try {
    const buf = Buffer.from(codeBoc, "base64")
    const cell = TasmCell.fromBoc(buf)[0]
    const codeCell = Cell.fromBase64(codeBoc)
    const decompiled = text.print(runtime.decompileCell(cell))

    return {
      base64: codeBoc,
      codeHashBase64: codeCell.hash().toString("base64"),
      codeHashHex: codeCell.hash().toString("hex"),
      hex: buf.toString("hex").toUpperCase(),
      decompiled,
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
}

function SourcePanel({
  activeTab,
  onTabChange,
  codeData,
  defaultFileTreeVisible,
  verifiedSource,
  verificationUrl,
  verificationExternal,
  compact,
}: {
  readonly activeTab: ContractSourceTab
  readonly onTabChange: (tab: ContractSourceTab) => void
  readonly codeData: ContractCodeData
  readonly defaultFileTreeVisible: boolean
  readonly verifiedSource?: ContractVerifiedSource
  readonly verificationUrl?: string
  readonly verificationExternal: boolean
  readonly compact: boolean
}): JSX.Element {
  const activeSourceTab = activeTab === "verified" && !verifiedSource ? "decompiled" : activeTab
  const sourceTabs: readonly {
    tab: ContractSourceTab
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
    <section className={`${styles.sourceShell} ${compact ? styles.sourceShellCompact : ""}`}>
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
            {item.verified && !compact && <CheckCircle2 size={15} aria-hidden="true" />}
            {item.label}
          </button>
        ))}
      </div>
      {activeSourceTab === "verified" && verifiedSource ? (
        <VerifiedSourcePanel
          source={verifiedSource}
          defaultFileTreeVisible={defaultFileTreeVisible}
          verificationUrl={verificationUrl}
          verificationExternal={verificationExternal}
          compact={compact}
        />
      ) : activeSource ? (
        <RawDataBlock
          className={styles.sourceDataBlock}
          variant="standalone"
          value={activeSource.value}
          copyLabel={activeSource.title}
          customContent={
            <HighlightedCode
              className={styles.highlightedCode}
              value={activeSource.value}
              language={activeSource.language}
              wrap={activeSource.wrap}
            />
          }
        />
      ) : undefined}
    </section>
  )
}

function VerifiedSourcePanel({
  source,
  defaultFileTreeVisible,
  verificationUrl,
  verificationExternal,
  compact,
}: {
  readonly source: ContractVerifiedSource
  readonly defaultFileTreeVisible: boolean
  readonly verificationUrl?: string
  readonly verificationExternal: boolean
  readonly compact: boolean
}): JSX.Element {
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
      <CodeViewer
        key={activeBundle.source_bundle_hash}
        attachedToTabs
        compact={compact}
        defaultFileTreeVisible={defaultFileTreeVisible}
        files={activeBundle.files}
        entrypoint={activeBundle.entrypoint}
        externalActionLabel="View verification"
        externalActionUrl={
          verificationUrl ?? `${VERIFIER_BASE_URL}/${encodeURIComponent(source.code_hash)}`
        }
        externalActionExternal={verificationUrl ? verificationExternal : true}
      />
    </section>
  )
}

function shortenMiddle(value: string, prefix = 8, suffix = 6): string {
  if (value.length <= prefix + suffix + 1) {
    return value
  }
  return `${value.slice(0, prefix)}…${value.slice(-suffix)}`
}
