import {useCallback, useEffect, useMemo, useState} from "react"
import type {FC, FormEvent, JSX} from "react"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {useToast} from "@acton/shared-ui"
import {CircleAlert, Plus, Trash2, Upload} from "lucide-react"
import {Link, useParams} from "react-router-dom"

import type {ExtendedContractABI} from "../api/compilerAbi"
import {
  getBundledCompilerAbiCatalog,
  type BundledCompilerAbiCatalogEntry,
} from "../api/compilerAbiCatalog"
import {AbiPanel, type AbiTab} from "../components/abi-viewer"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {JsonUploadField} from "../components/JsonUploadField"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {normalizeCodeHash} from "../metadata/codeHash"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import type {RegisteredCompilerAbi} from "../metadata/types"

import styles from "./AbiCatalogPage.module.css"

interface AbiCatalogState {
  readonly loading: boolean
  readonly entries: readonly BundledCompilerAbiCatalogEntry[]
}

interface RegisteredMetadataState {
  readonly loading: boolean
  readonly compilerAbis: readonly RegisteredCompilerAbi[]
}

interface AbiCatalogTableEntry {
  readonly slug: string
  readonly source: "bundled" | "local"
  readonly abi: ExtendedContractABI
  readonly deleteCodeHash?: string
}

export const AbiCatalogPage: FC = () => {
  const routes = useExplorerRoutePaths()
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
        description: "ABI registered",
        variant: "success",
      })
      await loadRegisteredMetadata()
    } catch (error) {
      showToast({
        description: error instanceof Error ? error.message : "Failed to register ABI",
        variant: "error",
      })
    }
  }

  const handleDeleteAbi = async (codeHash: string) => {
    try {
      await metadataRegistry.deleteCompilerAbi(codeHash)
      showToast({
        description: "ABI deleted",
        variant: "success",
      })
      await loadRegisteredMetadata()
    } catch (error) {
      showToast({
        description: error instanceof Error ? error.message : "Failed to delete ABI",
        variant: "error",
      })
    }
  }

  const tableEntries = useMemo(
    () => buildAbiTableEntries(registeredState.compilerAbis, state.entries),
    [registeredState.compilerAbis, state.entries],
  )
  const tableLoading = state.loading || registeredState.loading
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
    <section className={styles.container}>
      <Breadcrumbs items={[{label: "ABI"}]} />
      <div className={styles.hero}>
        <h1 className={styles.title}>ABI</h1>
      </div>

      <section className={styles.tableFrame}>
        <header className={styles.tableTitle}>Registered ABI</header>
        {tableLoading ? (
          <AbiCatalogSkeleton />
        ) : (
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
                    <button
                      type="button"
                      className={styles.registerAbiButton}
                      aria-expanded={abiFormExpanded}
                      onClick={toggleAbiForm}
                    >
                      <Plus size={16} />
                      <span>Register ABI</span>
                    </button>
                  </td>
                </tr>
                {abiFormExpanded && (
                  <tr className={styles.expandedFormRow}>
                    <td colSpan={6}>
                      <form className={styles.abiInlineForm} onSubmit={handleAbiUpload}>
                        <div className={styles.formGrid}>
                          <label className={styles.fieldLabel}>
                            Display name
                            <input
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
                                  <input
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
                          <span>
                            Registered ABI is stored locally for this explorer and is not uploaded
                            to a remote server.
                          </span>
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
                {tableEntries.map(entry => {
                  const stats = abiStats(entry.abi)
                  const title = abiTitle(entry.abi)
                  const contractName = entry.abi.compiler_abi.contract_name
                  const deleteCodeHash = entry.deleteCodeHash
                  return (
                    <tr key={entry.slug} className={styles.tableRow}>
                      <td>
                        <Link
                          className={styles.rowOverlayLink}
                          to={routes.abiDetailsPath(entry.slug)}
                          aria-label={`Open ${title} ABI`}
                        >
                          <span className={styles.visuallyHidden}>Open {title} ABI</span>
                        </Link>
                        <div className={styles.nameCell}>
                          <div className={styles.primaryCell}>
                            <div className={styles.nameLine}>
                              <span className={styles.nameText}>{title}</span>
                              {entry.source === "local" && (
                                <span className={styles.localBadge}>local</span>
                              )}
                            </div>
                            {title !== contractName && <small>{contractName}</small>}
                          </div>
                          {deleteCodeHash && (
                            <button
                              type="button"
                              className={`${styles.iconButton} ${styles.rowActionButton}`}
                              onClick={event => {
                                event.preventDefault()
                                event.stopPropagation()
                                void handleDeleteAbi(deleteCodeHash)
                              }}
                              aria-label="Delete ABI"
                            >
                              <Trash2 size={15} />
                            </button>
                          )}
                        </div>
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
        )}
      </section>
    </section>
  )
}

export const AbiDetailsPage: FC = () => {
  const {slug = ""} = useParams()
  const routes = useExplorerRoutePaths()
  const metadataRegistry = useMetadataRegistry()
  const [state, setState] = useState<{
    readonly loading: boolean
    readonly entries: readonly AbiCatalogTableEntry[]
  }>({loading: true, entries: []})
  const [activeTab, setActiveTab] = useState<AbiTab>("view")

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
    return (
      <section className={styles.container}>
        <Breadcrumbs items={[{label: "ABI", path: routes.abiPath}, {label: "Loading"}]} />
        <AbiDetailsSkeleton />
      </section>
    )
  }

  if (!entry) {
    return (
      <section className={styles.container}>
        <Breadcrumbs items={[{label: "ABI", path: routes.abiPath}, {label: "Not found"}]} />
        <div className={styles.emptyPage}>ABI not found</div>
      </section>
    )
  }

  const title = abiTitle(entry.abi)
  const contractName = entry.abi.compiler_abi.contract_name

  return (
    <section className={styles.container}>
      <Breadcrumbs items={[{label: "ABI", path: routes.abiPath}, {label: title}]} />
      <section className={styles.detailsHeader}>
        <div className={styles.detailsMain}>
          <h1 className={styles.title}>{title}</h1>
          {title !== contractName && <p className={styles.subtitle}>{contractName}</p>}
        </div>
      </section>

      {entry.abi.links.length > 0 && (
        <div className={styles.links}>
          {entry.abi.links.map(link => (
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
        abi={entry.abi.compiler_abi}
        getMethodsMode="readonly"
        heightMode="content"
        showSymbolAnchors
      />
    </section>
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

function extendedAbiFromUpload(
  source: unknown,
  codeHashes: readonly string[],
  displayName: string,
): ExtendedContractABI {
  const record =
    source && typeof source === "object" ? (source as Partial<ExtendedContractABI>) : {}
  const compilerAbi =
    record.compiler_abi && typeof record.compiler_abi === "object"
      ? (record.compiler_abi as ContractABI)
      : (source as ContractABI)

  if (!isCompilerAbi(compilerAbi)) {
    throw new Error("Uploaded JSON must be a compiler ABI.")
  }

  return {
    compiler_abi: compilerAbi,
    display_name:
      displayName.trim() ||
      (typeof record.display_name === "string" ? record.display_name.trim() : "") ||
      compilerAbi.contract_name,
    code_hashes: codeHashes,
    links: Array.isArray(record.links) ? record.links : [],
  }
}

function isCompilerAbi(value: unknown): value is ContractABI {
  if (!value || typeof value !== "object") {
    return false
  }

  const abi = value as Partial<ContractABI>
  return (
    typeof abi.contract_name === "string" &&
    Array.isArray(abi.get_methods) &&
    Array.isArray(abi.incoming_messages) &&
    Array.isArray(abi.incoming_external) &&
    Array.isArray(abi.outgoing_messages) &&
    Array.isArray(abi.emitted_events) &&
    Array.isArray(abi.declarations) &&
    Array.isArray(abi.thrown_errors)
  )
}

function buildAbiTableEntries(
  registeredEntries: readonly RegisteredCompilerAbi[],
  bundledEntries: readonly BundledCompilerAbiCatalogEntry[],
): readonly AbiCatalogTableEntry[] {
  return [
    ...registeredEntries.map(entry => ({
      slug: localAbiSlug(entry.codeHash),
      source: "local" as const,
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

function localAbiSlug(codeHash: string): string {
  return `local-${normalizeCodeHash(codeHash) ?? codeHash.trim().toLowerCase()}`
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
  return labels[normalized] ?? kind.replaceAll("_", " ")
}
