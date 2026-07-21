import {
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FileCode2,
  Folder,
  FolderOpen,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react"
import {useMemo, useState} from "react"
import type {CSSProperties} from "react"

import {cx} from "../../lib/cx"
import {HighlightedCode} from "../HighlightedCode/HighlightedCode"
import type {HighlightedCodeLanguage} from "../HighlightedCode/types"
import {CopyInlineAction} from "../InlineActions/InlineActions"
import styles from "./CodeViewer.module.css"

export interface CodeViewerFile {
  readonly path: string
  readonly content: string
}

export interface CodeViewerProps {
  readonly attachedToTabs?: boolean
  readonly className?: string
  readonly compact?: boolean
  readonly defaultFileTreeVisible?: boolean
  readonly emptyMessage?: string
  readonly entrypoint?: string
  readonly externalActionExternal?: boolean
  readonly externalActionLabel?: string
  readonly externalActionUrl?: string
  readonly files: readonly CodeViewerFile[]
}

interface FileTreeNode {
  readonly children: readonly FileTreeNode[]
  readonly file?: CodeViewerFile
  readonly kind: "file" | "folder"
  readonly name: string
  readonly path: string
}

interface FileTreeDraftNode {
  readonly children: Map<string, FileTreeDraftNode>
  readonly file?: CodeViewerFile
  readonly kind: "file" | "folder"
  readonly name: string
  readonly path: string
}

export function CodeViewer({
  attachedToTabs = false,
  className,
  compact = false,
  defaultFileTreeVisible = true,
  emptyMessage = "No source files",
  entrypoint,
  externalActionExternal = true,
  externalActionLabel = "Open source",
  externalActionUrl,
  files,
}: CodeViewerProps) {
  const entrypointFile = useMemo(() => findEntrypointFile(files, entrypoint), [entrypoint, files])
  const [selectedPath, setSelectedPath] = useState<string>()
  const [collapsedFolders, setCollapsedFolders] = useState<ReadonlySet<string>>(() => new Set())
  const [isDesktopTreeVisible, setDesktopTreeVisible] = useState(defaultFileTreeVisible)
  const [isMobileTreeOpen, setMobileTreeOpen] = useState(false)
  const activeFile =
    findFileByPath(files, selectedPath) ?? entrypointFile ?? files.find(file => file.path.trim())
  const tree = useMemo(() => buildFileTree(files), [files])

  if (!activeFile) {
    return <div className={cx(styles.empty, className)}>{emptyMessage}</div>
  }

  const code = trimFinalNewline(activeFile.content)
  const activePath = normalizeFilePath(activeFile.path)
  const entrypointPath = normalizeFilePath(entrypointFile?.path ?? entrypoint ?? "")

  const selectFile = (path: string) => {
    setSelectedPath(path)
    setMobileTreeOpen(false)
  }

  const toggleFolder = (path: string) => {
    setCollapsedFolders(current => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const fileTree = (
    <div className={styles.fileTreeList}>
      <FileTreeRows
        nodes={tree}
        activePath={activePath}
        collapsedFolders={collapsedFolders}
        entrypointPath={entrypointPath}
        onSelect={selectFile}
        onToggleFolder={toggleFolder}
      />
    </div>
  )

  return (
    <section
      className={cx(
        styles.workspace,
        compact && styles.compact,
        attachedToTabs && styles.attachedToTabs,
        !isDesktopTreeVisible && styles.desktopTreeHidden,
        className,
      )}
      aria-label="Source code"
    >
      {isDesktopTreeVisible && (
        <aside className={cx(styles.fileTree, styles.desktopFileTree)} aria-label="Source files">
          <button
            type="button"
            className={styles.desktopTreeToggle}
            title="Hide source files"
            aria-label="Hide source files"
            onClick={() => setDesktopTreeVisible(false)}
          >
            <PanelLeftClose aria-hidden="true" />
          </button>
          {fileTree}
        </aside>
      )}
      <div className={styles.codePane}>
        <div className={styles.codePaneHeader}>
          {!isDesktopTreeVisible && (
            <button
              type="button"
              className={styles.desktopTreeExpand}
              title="Show source files"
              aria-label="Show source files"
              onClick={() => setDesktopTreeVisible(true)}
            >
              <PanelLeftOpen aria-hidden="true" />
            </button>
          )}
          <button
            type="button"
            className={cx(styles.mobileTreeToggle, isMobileTreeOpen && styles.mobileTreeToggleOpen)}
            aria-label="Toggle source files"
            aria-expanded={isMobileTreeOpen}
            onClick={() => setMobileTreeOpen(current => !current)}
          >
            <Menu aria-hidden="true" />
          </button>
          <span className={styles.codePanePath} title={activeFile.path}>
            {activeFile.path}
          </span>
          {externalActionUrl && (
            <a
              className={styles.externalAction}
              href={externalActionUrl}
              target={externalActionExternal ? "_blank" : undefined}
              rel={externalActionExternal ? "noreferrer" : undefined}
            >
              <ExternalLink aria-hidden="true" />
              {externalActionLabel}
            </a>
          )}
          <CopyInlineAction
            className={styles.copyAction}
            value={code}
            label={`Copy ${activeFile.path}`}
            copiedLabel={`${activeFile.path} copied`}
          />
        </div>
        <aside
          className={cx(
            styles.fileTree,
            styles.mobileFileTree,
            isMobileTreeOpen && styles.mobileFileTreeOpen,
          )}
          aria-label="Source files"
        >
          {fileTree}
        </aside>
        <div className={styles.codeFrame}>
          <div className={styles.lineNumbers} aria-hidden="true">
            {Array.from({length: lineCount(code)}, (_, index) => (
              <span key={index + 1}>{index + 1}</span>
            ))}
          </div>
          <div className={styles.code}>
            <HighlightedCode
              className={styles.highlightedCode}
              value={code}
              language={languageForPath(activeFile.path)}
            />
          </div>
        </div>
      </div>
    </section>
  )
}

function FileTreeRows({
  activePath,
  collapsedFolders,
  depth = 0,
  entrypointPath,
  nodes,
  onSelect,
  onToggleFolder,
}: {
  readonly activePath: string
  readonly collapsedFolders: ReadonlySet<string>
  readonly depth?: number
  readonly entrypointPath: string
  readonly nodes: readonly FileTreeNode[]
  readonly onSelect: (path: string) => void
  readonly onToggleFolder: (path: string) => void
}) {
  return (
    <ul className={styles.treeLevel}>
      {nodes.map(node => {
        const depthStyle = {"--code-tree-depth": String(depth)} as CSSProperties
        if (node.kind === "folder") {
          const expanded = !collapsedFolders.has(node.path)
          return (
            <li key={node.path} className={styles.treeItem}>
              <button
                type="button"
                className={cx(styles.treeRow, styles.folderRow)}
                style={depthStyle}
                aria-expanded={expanded}
                onClick={() => onToggleFolder(node.path)}
              >
                {expanded ? (
                  <ChevronDown className={styles.disclosureIcon} aria-hidden="true" />
                ) : (
                  <ChevronRight className={styles.disclosureIcon} aria-hidden="true" />
                )}
                {expanded ? (
                  <FolderOpen className={styles.rowIcon} aria-hidden="true" />
                ) : (
                  <Folder className={styles.rowIcon} aria-hidden="true" />
                )}
                <span>{node.name}</span>
              </button>
              {expanded && (
                <FileTreeRows
                  nodes={node.children}
                  activePath={activePath}
                  collapsedFolders={collapsedFolders}
                  depth={depth + 1}
                  entrypointPath={entrypointPath}
                  onSelect={onSelect}
                  onToggleFolder={onToggleFolder}
                />
              )}
            </li>
          )
        }

        const isActive = node.path === activePath
        return (
          <li key={node.path} className={styles.treeItem}>
            <button
              type="button"
              className={cx(styles.treeRow, isActive && styles.activeRow)}
              style={depthStyle}
              title={node.path}
              aria-current={isActive ? "true" : undefined}
              onClick={() => node.file && onSelect(node.file.path)}
            >
              <span className={styles.disclosurePlaceholder} aria-hidden="true" />
              <FileCode2 className={styles.rowIcon} aria-hidden="true" />
              <span>{node.name}</span>
              {node.path === entrypointPath && <span className={styles.entrypoint}>main</span>}
            </button>
          </li>
        )
      })}
    </ul>
  )
}

function buildFileTree(files: readonly CodeViewerFile[]): readonly FileTreeNode[] {
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
        node = {...node, kind: "file", file}
        currentLevel.set(part, node)
      }

      currentLevel = node.children
    }
  }

  return sortTree([...root.values()].map(freezeTree))
}

function freezeTree(node: FileTreeDraftNode): FileTreeNode {
  return {
    kind: node.kind,
    name: node.name,
    path: node.path,
    children: sortTree([...node.children.values()].map(freezeTree)),
    file: node.file,
  }
}

function sortTree(nodes: readonly FileTreeNode[]): FileTreeNode[] {
  return nodes.toSorted((left, right) => {
    if (left.kind !== right.kind) return left.kind === "folder" ? -1 : 1
    return left.name.localeCompare(right.name)
  })
}

function normalizeFilePath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.?\//, "")
}

function findFileByPath(
  files: readonly CodeViewerFile[],
  path: string | undefined,
): CodeViewerFile | undefined {
  if (!path) return undefined
  const normalizedPath = normalizeFilePath(path)
  return files.find(file => normalizeFilePath(file.path) === normalizedPath)
}

function findEntrypointFile(
  files: readonly CodeViewerFile[],
  entrypoint: string | undefined,
): CodeViewerFile | undefined {
  const exactMatch = findFileByPath(files, entrypoint)
  if (exactMatch || !entrypoint) return exactMatch

  const suffix = `/${normalizeFilePath(entrypoint)}`
  const suffixMatches = files.filter(file => normalizeFilePath(file.path).endsWith(suffix))
  return suffixMatches.length === 1 ? suffixMatches[0] : undefined
}

function trimFinalNewline(content: string): string {
  return content.endsWith("\n") ? content.slice(0, -1) : content
}

function lineCount(code: string): number {
  return code.length === 0 ? 1 : code.split("\n").length
}

function languageForPath(path: string): HighlightedCodeLanguage | undefined {
  const normalizedPath = path.toLowerCase()
  if (normalizedPath.endsWith(".tolk")) return "tolk"
  if (normalizedPath.endsWith(".fc") || normalizedPath.endsWith(".func")) return "func"
  if (
    normalizedPath.endsWith(".json") ||
    normalizedPath.endsWith(".abi") ||
    normalizedPath.endsWith(".pkg")
  ) {
    return "json"
  }
  return undefined
}
