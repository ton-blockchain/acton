import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {ChangeEvent, DragEvent, FC, FormEvent, JSX} from "react"
import {AbiPanel, type AbiTab} from "@acton/transaction-ui/abi"
import {
  InlineAction,
  InlineActions,
  Input,
  Pagination,
  humanizeIdentifier,
  useToast,
} from "@acton/ui"
import {CircleAlert, FolderUp, Plus, Trash2, Upload} from "lucide-react"
import {Link, useNavigate} from "react-router"

import type {ExtendedContractABI} from "../api/compilerAbi"
import {
  getBundledCompilerAbiCatalog,
  type BundledCompilerAbiCatalogEntry,
} from "../api/compilerAbiCatalog"
import {
  buildAbiImportPlan,
  collectDroppedImportFiles,
  collectPickedImportFiles,
  extendedAbiFromUpload,
  type AbiImportFile,
} from "./buildImport"
import {JsonUploadField} from "./JsonUploadField"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useSearchParamPagination} from "../hooks/useSearchParamPagination"
import {normalizeCodeHash} from "../metadata/codeHash"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import type {RegisteredCompilerAbi} from "../metadata/types"

import styles from "./AbiCatalog.module.css"

interface AbiCatalogState {
  readonly loading: boolean
  readonly entries: readonly BundledCompilerAbiCatalogEntry[]
}

interface RegisteredMetadataState {
  readonly loading: boolean
  readonly compilerAbis: readonly RegisteredCompilerAbi[]
}

export interface AbiCatalogTableEntry {
  readonly slug: string
  readonly source: "bundled" | "environment"
  readonly abi: ExtendedContractABI
  readonly deleteCodeHash?: string
}

export const AbiCatalog: FC = () => {
  const routes = useExplorerRoutePaths()
  const navigate = useNavigate()
  const metadataRegistry = useMetadataRegistry()
  const {showToast} = useToast()
  const [state, setState] = useState<AbiCatalogState>({loading: true, entries: []})
  const [registeredState, setRegisteredState] = useState<RegisteredMetadataState>({
    loading: true,
    compilerAbis: [],
  })
  const [abiName, setAbiName] = useState("")
  const [abiCodeHashes, setAbiCodeHashes] = useState<readonly string[]>([""])
  const [abiJson, setAbiJson] = useState("")
  const [abiFormExpanded, setAbiFormExpanded] = useState(false)
  const [dropActive, setDropActive] = useState(false)
  const [importing, setImporting] = useState(false)
  const dragDepth = useRef(0)
  const directoryInputRef = useRef<HTMLInputElement>(null)

  const loadRegisteredMetadata = useCallback(async () => {
    setRegisteredState(current => ({...current, loading: true}))
    const compilerAbis = await metadataRegistry.listCompilerAbis()
    setRegisteredState({loading: false, compilerAbis})
  }, [metadataRegistry])

  useEffect(() => {
    let isActive = true

    const loadCatalog = async () => {
      const entries = await getBundledCompilerAbiCatalog()
      if (isActive) {
        setState({loading: false, entries})
      }
    }

    void loadCatalog()

    return () => {
      isActive = false
    }
  }, [])

  useEffect(() => {
    let isActive = true
    setRegisteredState(current => ({...current, loading: true}))
    metadataRegistry
      .listCompilerAbis()
      .then(compilerAbis => {
        if (isActive) {
          setRegisteredState({loading: false, compilerAbis})
        }
      })
      .catch(error => {
        if (isActive) {
          console.debug("Failed to load registered metadata", error)
          setRegisteredState({loading: false, compilerAbis: []})
        }
      })
    return () => {
      isActive = false
    }
  }, [metadataRegistry])

  const handleAbiUpload = async (event: FormEvent) => {
    event.preventDefault()

    try {
      const parsed = JSON.parse(abiJson) as unknown
      const codeHashes = parseCodeHashes(abiCodeHashes.join("\n"), parsed)
      if (codeHashes.length === 0) {
        throw new Error("Add at least one code hash.")
      }
      const abi = extendedAbiFromUpload(parsed, codeHashes, abiName)
      await metadataRegistry.registerCompilerAbis([{abi}])
      setAbiName("")
      setAbiCodeHashes([""])
      setAbiJson("")
      setAbiFormExpanded(false)
      showToast({
        title: "ABI registered",
        variant: "success",
      })
      await loadRegisteredMetadata()
    } catch (error) {
      showToast({
        title: "ABI not registered",
        description: error instanceof Error ? error.message : "Failed to register ABI.",
        variant: "error",
      })
    }
  }

  const importAbiFiles = useCallback(
    async (files: readonly AbiImportFile[]) => {
      if (importing) {
        return
      }
      setImporting(true)
      try {
        if (!metadataRegistry.canWriteCompilerAbis) {
          throw new Error("This environment does not accept ABI registrations.")
        }
        const plan = buildAbiImportPlan(files)
        if (plan.registrations.length === 0) {
          throw new Error(
            plan.warnings[0] ??
              "No ABI JSON files found. Drop an acton build/ directory (or its abi/ files together with the compiled <Name>.json files).",
          )
        }
        await metadataRegistry.registerCompilerAbis(plan.registrations)
        showToast({
          title: `Registered ${plan.registrations.length} ABI${plan.registrations.length === 1 ? "" : "s"}`,
          description: formatImportSummary(plan.registeredNames, plan.warnings),
          variant: "success",
        })
        await loadRegisteredMetadata()
      } catch (error) {
        showToast({
          title: "ABIs not registered",
          description: error instanceof Error ? error.message : "Failed to import ABIs.",
          variant: "error",
        })
      } finally {
        setImporting(false)
      }
    },
    [importing, loadRegisteredMetadata, metadataRegistry, showToast],
  )

  const handleDragEnter = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) {
      return
    }
    event.preventDefault()
    dragDepth.current += 1
    setDropActive(true)
  }

  const handleDragOver = (event: DragEvent) => {
    if (event.dataTransfer.types.includes("Files")) {
      event.preventDefault()
    }
  }

  const handleDragLeave = (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) {
      return
    }
    dragDepth.current = Math.max(0, dragDepth.current - 1)
    if (dragDepth.current === 0) {
      setDropActive(false)
    }
  }

  const handleDrop = async (event: DragEvent) => {
    if (!event.dataTransfer.types.includes("Files")) {
      return
    }
    event.preventDefault()
    dragDepth.current = 0
    setDropActive(false)
    await importAbiFiles(await collectDroppedImportFiles(event.dataTransfer))
  }

  const handleDirectoryPick = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = await collectPickedImportFiles(event.target.files)
    event.target.value = ""
    await importAbiFiles(files)
  }

  const handleDeleteAbi = async (codeHash: string) => {
    try {
      await metadataRegistry.deleteCompilerAbi(codeHash)
      showToast({
        title: "ABI deleted",
        variant: "success",
      })
      await loadRegisteredMetadata()
    } catch (error) {
      showToast({
        title: "ABI not deleted",
        description: error instanceof Error ? error.message : "Failed to delete ABI.",
        variant: "error",
      })
    }
  }

  const tableEntries = useMemo(
    () => buildAbiTableEntries(registeredState.compilerAbis, state.entries),
    [registeredState.compilerAbis, state.entries],
  )
  const tableLoading = state.loading || registeredState.loading
  const pagination = useSearchParamPagination(tableEntries, {ready: !tableLoading})
  const hasAbiCodeHash = abiCodeHashes.some(codeHash => codeHash.trim().length > 0)
  const toggleAbiForm = () => setAbiFormExpanded(expanded => !expanded)
  const updateAbiCodeHash = (index: number, value: string) => {
    setAbiCodeHashes(current =>
      current.map((item, itemIndex) => (itemIndex === index ? value : item)),
    )
  }
  const addAbiCodeHash = () => setAbiCodeHashes(current => [...current, ""])
  const removeAbiCodeHash = (index: number) => {
    setAbiCodeHashes(current => current.filter((_, itemIndex) => itemIndex !== index))
  }

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: Drag-and-drop is a pointer-only shortcut; the "Import build folder" button is the accessible path.
    <section
      className={styles.tableFrame}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={event => {
        void handleDrop(event)
      }}
    >
      {dropActive && (
        <div className={styles.dropOverlay}>
          <FolderUp size={28} />
          <span>Drop an acton build/ directory to register its ABIs</span>
        </div>
      )}
      {tableLoading ? (
        <AbiCatalogSkeleton />
      ) : (
        <>
          <div className={styles.tableScroller}>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Get Methods</th>
                  <th>Messages</th>
                  <th>Declarations</th>
                  <th>Errors</th>
                  <th>Code hashes</th>
                </tr>
              </thead>
              <tbody>
                <tr className={styles.formButtonRow}>
                  <td colSpan={6}>
                    <div className={styles.actionButtonsRow}>
                      <button
                        type="button"
                        className={styles.registerAbiButton}
                        aria-expanded={abiFormExpanded}
                        onClick={toggleAbiForm}
                      >
                        <Plus size={16} />
                        <span>Register ABI</span>
                      </button>
                      <button
                        type="button"
                        className={styles.importBuildButton}
                        disabled={importing || !metadataRegistry.canWriteCompilerAbis}
                        onClick={() => directoryInputRef.current?.click()}
                        title="Register every ABI from an acton build/ directory (or drop the directory anywhere on this table)"
                      >
                        <FolderUp size={16} />
                        <span>{importing ? "Importing…" : "Import build folder"}</span>
                      </button>
                      <input
                        ref={directoryInputRef}
                        className={styles.hiddenFileInput}
                        type="file"
                        multiple
                        {...directoryInputProps}
                        onChange={event => {
                          void handleDirectoryPick(event)
                        }}
                      />
                    </div>
                  </td>
                </tr>
                {abiFormExpanded && (
                  <tr className={styles.expandedFormRow}>
                    <td colSpan={6}>
                      <form className={styles.abiInlineForm} onSubmit={handleAbiUpload}>
                        <div className={styles.formGrid}>
                          <label className={styles.fieldLabel} htmlFor="abi-display-name">
                            Display name
                            <Input
                              id="abi-display-name"
                              size="sm"
                              className={styles.textInput}
                              value={abiName}
                              onChange={event => setAbiName(event.target.value)}
                              placeholder="optional display name"
                            />
                          </label>
                          <div className={styles.fieldLabel}>
                            Code hashes
                            <div className={styles.codeHashList}>
                              {abiCodeHashes.map((codeHash, index) => (
                                <div key={index} className={styles.codeHashRow}>
                                  <Input
                                    size="sm"
                                    className={styles.textInput}
                                    value={codeHash}
                                    onChange={event => updateAbiCodeHash(index, event.target.value)}
                                    placeholder="hex or base64"
                                  />
                                  {index === abiCodeHashes.length - 1 ? (
                                    <button
                                      type="button"
                                      className={styles.iconButton}
                                      onClick={addAbiCodeHash}
                                      aria-label="Add code hash"
                                    >
                                      <Plus size={15} />
                                    </button>
                                  ) : (
                                    <button
                                      type="button"
                                      className={styles.iconButton}
                                      onClick={() => removeAbiCodeHash(index)}
                                      aria-label="Remove code hash"
                                    >
                                      <Trash2 size={15} />
                                    </button>
                                  )}
                                </div>
                              ))}
                            </div>
                          </div>
                          <div className={styles.jsonField}>
                            <JsonUploadField
                              label="ABI JSON"
                              value={abiJson}
                              onChange={setAbiJson}
                            />
                          </div>
                        </div>
                        <p className={styles.localNote}>
                          <CircleAlert size={15} />
                          <span>Registered ABI remains in this virtual environment</span>
                        </p>
                        <button
                          type="submit"
                          className={`${styles.primaryButton} ${styles.formSubmitButton}`}
                          disabled={
                            !metadataRegistry.canWriteCompilerAbis ||
                            !hasAbiCodeHash ||
                            abiJson.trim().length === 0
                          }
                        >
                          <Upload size={15} />
                          Register ABI
                        </button>
                      </form>
                    </td>
                  </tr>
                )}
                {pagination.currentItems.map(entry => {
                  const stats = abiStats(entry.abi)
                  const title = abiTitle(entry.abi)
                  const contractName = entry.abi.compiler_abi.contract_name
                  const deleteCodeHash = entry.deleteCodeHash
                  const detailsPath = routes.abiDetailsPath(entry.slug)
                  return (
                    <tr
                      key={entry.slug}
                      className={styles.tableRow}
                      onClick={() => void navigate(detailsPath)}
                    >
                      <td>
                        <InlineActions
                          className={styles.nameCell}
                          visibility="always"
                          actions={
                            deleteCodeHash ? (
                              <InlineAction
                                label="Delete ABI"
                                icon={<Trash2 />}
                                onClick={event => {
                                  event.preventDefault()
                                  event.stopPropagation()
                                  void handleDeleteAbi(deleteCodeHash)
                                }}
                              />
                            ) : undefined
                          }
                        >
                          <span className={styles.primaryCell}>
                            <span className={styles.nameLine}>
                              <Link
                                className={styles.nameText}
                                to={detailsPath}
                                aria-label={`Open ${title} ABI`}
                                onClick={event => event.stopPropagation()}
                              >
                                {title}
                              </Link>
                              {entry.source === "environment" && (
                                <span className={styles.environmentBadge}>environment</span>
                              )}
                            </span>
                            {title !== contractName && <small>{contractName}</small>}
                          </span>
                        </InlineActions>
                      </td>
                      <td>{stats.methods}</td>
                      <td>{stats.messages}</td>
                      <td>{stats.declarations}</td>
                      <td>{stats.errors}</td>
                      <td>{entry.abi.code_hashes.length}</td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          <Pagination
            currentPage={pagination.currentPage}
            totalItems={pagination.totalItems}
            pageSize={pagination.pageSize}
            onPageChange={pagination.setCurrentPage}
            label="ABI catalog pagination"
          />
        </>
      )}
    </section>
  )
}

export type AbiDetailsState =
  | {readonly status: "loading"}
  | {readonly status: "not-found"}
  | {
      readonly status: "ready"
      readonly entry: AbiCatalogTableEntry
      readonly title: string
    }

export function useAbiDetails(slug: string): AbiDetailsState {
  const metadataRegistry = useMetadataRegistry()
  const [state, setState] = useState<{
    readonly loading: boolean
    readonly entries: readonly AbiCatalogTableEntry[]
  }>({loading: true, entries: []})

  useEffect(() => {
    let isActive = true

    const loadCatalog = async () => {
      const [bundledEntries, registeredEntries] = await Promise.all([
        getBundledCompilerAbiCatalog(),
        metadataRegistry.listCompilerAbis().catch(error => {
          console.debug("Failed to load registered ABI", error)
          return []
        }),
      ])
      if (isActive) {
        setState({
          loading: false,
          entries: buildAbiTableEntries(registeredEntries, bundledEntries),
        })
      }
    }

    void loadCatalog()

    return () => {
      isActive = false
    }
  }, [metadataRegistry])

  const entry = useMemo(() => state.entries.find(item => item.slug === slug), [slug, state.entries])

  if (state.loading) {
    return {status: "loading"}
  }

  if (!entry) {
    return {status: "not-found"}
  }

  return {status: "ready", entry, title: abiTitle(entry.abi)}
}

export const AbiDetails: FC<{readonly state: AbiDetailsState}> = ({state}) => {
  const [activeTab, setActiveTab] = useState<AbiTab>("view")

  if (state.status === "loading") {
    return <AbiDetailsSkeleton />
  }

  if (state.status === "not-found") {
    return <div className={styles.emptyPage}>ABI not found</div>
  }

  return (
    <>
      {state.entry.abi.links.length > 0 && (
        <div className={styles.links}>
          {state.entry.abi.links.map(link => (
            <a key={`${link.kind}:${link.url}`} href={link.url} target="_blank" rel="noreferrer">
              <span>{formatLinkKind(link.kind)}</span>
              {link.title}
            </a>
          ))}
        </div>
      )}

      <AbiPanel
        activeTab={activeTab}
        onTabChange={setActiveTab}
        abi={state.entry.abi.compiler_abi}
        heightMode="content"
        showSymbolAnchors
      />
    </>
  )
}

function parseCodeHashes(raw: string, source: unknown): readonly string[] {
  const explicit = raw
    .split(/[\s,]+/)
    .map(normalizeCodeHash)
    .filter((value): value is string => Boolean(value))
  if (explicit.length > 0) {
    return [...new Set(explicit)]
  }

  if (!source || typeof source !== "object") {
    return []
  }

  const record = source as {
    readonly code_hash?: unknown
    readonly codeHash?: unknown
    readonly code_hashes?: unknown
    readonly codeHashes?: unknown
    readonly hashes?: unknown
  }
  const candidates = [
    record.code_hash,
    record.codeHash,
    ...(Array.isArray(record.code_hashes) ? record.code_hashes : []),
    ...(Array.isArray(record.codeHashes) ? record.codeHashes : []),
    ...(Array.isArray(record.hashes) ? record.hashes : []),
  ]

  return [
    ...new Set(
      candidates
        .filter((value): value is string => typeof value === "string")
        .map(normalizeCodeHash)
        .filter((value): value is string => Boolean(value)),
    ),
  ]
}

// `webkitdirectory` enables directory picking but is missing from React's input
// prop types, so it goes in via a spread.
const directoryInputProps = {webkitdirectory: ""} as Record<string, string>

function formatImportSummary(names: readonly string[], warnings: readonly string[]): string {
  const shownNames = names.slice(0, 8)
  const parts = [
    shownNames.join(", ") +
      (names.length > shownNames.length ? ` +${names.length - shownNames.length} more` : ""),
  ]
  if (warnings.length > 0) {
    parts.push(`Skipped: ${warnings.join("; ")}`)
  }
  return parts.join(". ")
}

function buildAbiTableEntries(
  registeredEntries: readonly RegisteredCompilerAbi[],
  bundledEntries: readonly BundledCompilerAbiCatalogEntry[],
): readonly AbiCatalogTableEntry[] {
  return [
    ...registeredEntries.map(entry => ({
      slug: environmentAbiSlug(entry.codeHash),
      source: "environment" as const,
      abi: entry.abi,
      deleteCodeHash: entry.codeHash,
    })),
    ...bundledEntries.map(entry => ({
      slug: entry.slug,
      source: "bundled" as const,
      abi: entry,
    })),
  ]
}

function environmentAbiSlug(codeHash: string): string {
  return `environment-${normalizeCodeHash(codeHash) ?? codeHash.trim().toLowerCase()}`
}

function abiTitle(abi: ExtendedContractABI): string {
  return abi.display_name?.trim() || abi.compiler_abi.contract_name
}

function AbiCatalogSkeleton(): JSX.Element {
  return (
    <div className={styles.skeletonList} aria-label="Loading ABI catalog">
      {Array.from({length: 8}, (_, index) => (
        <div key={index} className={styles.skeletonRow}>
          <span />
          <span />
          <span />
        </div>
      ))}
    </div>
  )
}

function AbiDetailsSkeleton(): JSX.Element {
  return (
    <div className={styles.detailsSkeleton} aria-label="Loading ABI">
      <span />
      <span />
      <span />
    </div>
  )
}

function abiStats(entry: ExtendedContractABI): {
  readonly methods: number
  readonly messages: number
  readonly declarations: number
  readonly errors: number
} {
  const abi = entry.compiler_abi
  return {
    methods: abi.get_methods.length,
    messages:
      abi.incoming_messages.length +
      abi.incoming_external.length +
      abi.outgoing_messages.length +
      abi.emitted_events.length,
    declarations: abi.declarations.length,
    errors: abi.thrown_errors.length,
  }
}

function formatLinkKind(kind: string): string {
  const labels: Record<string, string> = {
    api: "API",
    audit: "Audit",
    docs: "Docs",
    sdk: "SDK",
    source: "Source",
    website: "Website",
  }
  const normalized = kind.toLowerCase()
  return labels[normalized] ?? humanizeIdentifier(kind)
}
