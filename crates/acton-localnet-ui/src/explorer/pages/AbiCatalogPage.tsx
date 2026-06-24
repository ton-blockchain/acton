import {useEffect, useMemo, useState} from "react"
import type {FC, JSX} from "react"
import {Link, useParams} from "react-router-dom"

import {
  getBundledCompilerAbiCatalog,
  type BundledCompilerAbiCatalogEntry,
} from "../api/compilerAbiCatalog"
import {AbiPanel, type AbiTab} from "../components/abi-viewer"
import {Breadcrumbs} from "../components/Breadcrumbs"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"

import styles from "./AbiCatalogPage.module.css"

interface AbiCatalogState {
  readonly loading: boolean
  readonly entries: readonly BundledCompilerAbiCatalogEntry[]
}

export const AbiCatalogPage: FC = () => {
  const routes = useExplorerRoutePaths()
  const [state, setState] = useState<AbiCatalogState>({loading: true, entries: []})

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

  return (
    <section className={styles.container}>
      <Breadcrumbs items={[{label: "ABI"}]} />
      <div className={styles.hero}>
        <h1 className={styles.title}>ABI</h1>
      </div>

      <section className={styles.tableFrame}>
        <header className={styles.tableTitle}>Known ABI</header>
        {state.loading ? (
          <AbiCatalogSkeleton />
        ) : state.entries.length > 0 ? (
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
                {state.entries.map(entry => {
                  const stats = abiStats(entry)
                  const title = entry.display_name ?? entry.compiler_abi.contract_name
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
                        <div className={styles.primaryCell}>
                          <span>{title}</span>
                          {entry.display_name &&
                            entry.display_name !== entry.compiler_abi.contract_name && (
                              <small>{entry.compiler_abi.contract_name}</small>
                            )}
                        </div>
                      </td>
                      <td>{stats.methods}</td>
                      <td>{stats.messages}</td>
                      <td>{stats.declarations}</td>
                      <td>{stats.errors}</td>
                      <td>{entry.code_hashes.length}</td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <div className={styles.empty}>No ABI found</div>
        )}
      </section>
    </section>
  )
}

export const AbiDetailsPage: FC = () => {
  const {slug = ""} = useParams()
  const routes = useExplorerRoutePaths()
  const [state, setState] = useState<AbiCatalogState>({loading: true, entries: []})
  const [activeTab, setActiveTab] = useState<AbiTab>("view")

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

  const title = entry.display_name ?? entry.compiler_abi.contract_name

  return (
    <section className={styles.container}>
      <Breadcrumbs items={[{label: "ABI", path: routes.abiPath}, {label: title}]} />
      <section className={styles.detailsHeader}>
        <div className={styles.detailsMain}>
          <h1 className={styles.title}>{title}</h1>
          {entry.display_name && entry.display_name !== entry.compiler_abi.contract_name && (
            <p className={styles.subtitle}>{entry.compiler_abi.contract_name}</p>
          )}
        </div>
      </section>

      {entry.links.length > 0 && (
        <div className={styles.links}>
          {entry.links.map(link => (
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
        abi={entry.compiler_abi}
        getMethodsMode="readonly"
        heightMode="content"
        showSymbolAnchors
      />
    </section>
  )
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

function abiStats(entry: BundledCompilerAbiCatalogEntry): {
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
