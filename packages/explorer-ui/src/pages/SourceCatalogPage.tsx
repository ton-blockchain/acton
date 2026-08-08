import {SourceCatalog} from "@acton/explorer-core/components/SourceCatalog"
import {ExplorerBreadcrumbs} from "@acton/explorer-core/components/ExplorerBreadcrumbs"
import type {TonClient} from "@acton/explorer-core/api/client"

import styles from "./CatalogPages.module.css"

export function SourceCatalogPage({client}: {readonly client: TonClient}) {
  return (
    <section className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Source artifacts"}]} />
      <header className={styles.hero}>
        <h1 className={styles.title}>Source artifacts</h1>
      </header>
      <SourceCatalog client={client} />
    </section>
  )
}
