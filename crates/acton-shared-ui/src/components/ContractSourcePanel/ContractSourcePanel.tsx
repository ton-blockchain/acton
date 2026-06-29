import {useEffect, useMemo, useState} from "react"
import type {CSSProperties, JSX} from "react"

import {Cell} from "@ton/core"
import {Cell as TasmCell, runtime, text} from "@ton/tasm"
import {Check, CheckCircle2, Copy, ExternalLink, FileCode2, Folder, Menu} from "lucide-react"
import {createHighlighterCore} from "shiki/core"
import {createJavaScriptRegexEngine} from "shiki/engine/javascript"
import type {LanguageRegistration} from "shiki/types"

import {jetbrainsDarculaTheme, jetbrainsLightTheme} from "../CodeSnippet/jetbrains-themes"
import {DataBlock} from "../DataBlock/DataBlock"

import funcGrammarRaw from "../../../../../docs/grammars/grammar-func.json"
import tasmGrammarRaw from "../../../../../docs/grammars/grammar-tasm.json"
import tolkGrammarRaw from "../../../../../docs/grammars/grammar-tolk.json"

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
type HighlightLanguage = "tasm" | "json" | "tolk" | "func"

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
  readonly verifiedSource?: ContractVerifiedSource
  readonly verifiedSourceLoading?: boolean
  readonly verificationUrl?: string
  readonly verificationExternal?: boolean
  readonly compact?: boolean
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

const grammarWithName = (grammar: unknown, name: string): LanguageRegistration =>
  ({
    ...(grammar as Record<string, unknown>),
    name,
  }) as LanguageRegistration

const tasmGrammar = grammarWithName(tasmGrammarRaw, "tasm")
const tolkGrammar = grammarWithName(tolkGrammarRaw, "tolk")
const funcGrammar = grammarWithName(funcGrammarRaw, "func")
const VERIFIER_BASE_URL = "https://verifier.acton.monster"

let contractSourceHighlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

const getContractSourceHighlighter = () => {
  contractSourceHighlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tasmGrammar, tolkGrammar, funcGrammar, import("shiki/langs/json.mjs")],
    engine: createJavaScriptRegexEngine(),
  })

  return contractSourceHighlighterPromise
}

export function ContractSourcePanel({
  codeBoc,
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
  verifiedSource,
  verificationUrl,
  verificationExternal,
  compact,
}: {
  readonly activeTab: ContractSourceTab
  readonly onTabChange: (tab: ContractSourceTab) => void
  readonly codeData: ContractCodeData
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
          verificationUrl={verificationUrl}
          verificationExternal={verificationExternal}
        />
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

function VerifiedSourcePanel({
  source,
  verificationUrl,
  verificationExternal,
}: {
  readonly source: ContractVerifiedSource
  readonly verificationUrl?: string
  readonly verificationExternal: boolean
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
      <VerifiedCodeViewer
        bundle={activeBundle}
        verificationUrl={
          verificationUrl ?? `${VERIFIER_BASE_URL}/${encodeURIComponent(source.code_hash)}`
        }
        verificationExternal={verificationUrl ? verificationExternal : true}
      />
    </section>
  )
}

function VerifiedCodeViewer({
  bundle,
  verificationUrl,
  verificationExternal,
}: {
  readonly bundle: SourceBundle
  readonly verificationUrl: string
  readonly verificationExternal: boolean
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
  const treeEntrypoint = entrypointPath ?? bundle.entrypoint
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
            entrypoint={treeEntrypoint}
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
            target={verificationExternal ? "_blank" : undefined}
            rel={verificationExternal ? "noreferrer" : undefined}
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
              entrypoint={treeEntrypoint}
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

function FileTreeRows({
  nodes,
  activePath,
  entrypoint,
  depth = 0,
  onSelect,
}: {
  readonly nodes: readonly FileTreeNode[]
  readonly activePath: string
  readonly entrypoint: string
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
  return file.content.endsWith("\n") ? file.content.slice(0, -1) : file.content
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
  entrypoint: string,
): SourceFile | undefined {
  const exactMatch = findFileByPath(files, entrypoint)
  if (exactMatch) {
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
  return `${value.slice(0, prefix)}…${value.slice(-suffix)}`
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
        const highlighter = await getContractSourceHighlighter()
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
