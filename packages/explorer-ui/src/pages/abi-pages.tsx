import {AbiCatalog, AbiDetails, useAbiDetails} from "@acton/explorer-core/components/AbiCatalog"
import {ExplorerBreadcrumbs} from "@acton/explorer-core/components/ExplorerBreadcrumbs"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useParams} from "react-router"

import styles from "./CatalogPages.module.css"

export function AbiCatalogPage() {
  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "ABI catalog"}]} />
      <header className={styles.hero}>
        <h1 className={styles.title}>ABI catalog</h1>
      </header>
      <AbiCatalog />
    </section>
  )
}

export function AbiDetailsPage() {
  const {slug = ""} = useParams()
  const routes = useExplorerRoutePaths()
  const state = useAbiDetails(slug)
  let title = "ABI"
  if (state.status === "ready") {
    title = state.title
  } else if (state.status === "not-found") {
    title = "ABI not found"
  }

  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs
        ariaLabel="Contract breadcrumb"
        rootLabel="Contracts"
        rootPath={routes.contractsPath ?? routes.rootPath}
        items={[{label: "ABI catalog", path: routes.abiPath}, {label: title}]}
      />
      <header className={styles.hero}>
        <h1 className={styles.title}>{title}</h1>
      </header>
      <AbiDetails state={state} />
    </section>
  )
}
