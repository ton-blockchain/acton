import {AbiCatalog, AbiDetails, useAbiDetails} from "@acton/explorer-core/components/AbiCatalog"
import {useParams} from "react-router"

import styles from "./AbiCatalogPage.module.css"

export function AbiCatalogPage() {
  return (
    <section className={styles.page}>
      <header className={styles.hero}>
        <h1 className={styles.title}>ABI catalog</h1>
      </header>
      <AbiCatalog />
    </section>
  )
}

export function AbiDetailsPage() {
  const {slug = ""} = useParams()
  const state = useAbiDetails(slug)
  const title =
    state.status === "ready" ? state.title : state.status === "not-found" ? "ABI not found" : "ABI"

  return (
    <section className={styles.page}>
      <header className={styles.hero}>
        <h1 className={styles.title}>{title}</h1>
      </header>
      <AbiDetails state={state} />
    </section>
  )
}
