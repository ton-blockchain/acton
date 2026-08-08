import type {TonClient} from "@acton/explorer-core/api/client"
import {ExplorerBreadcrumbs} from "@acton/explorer-core/components/ExplorerBreadcrumbs"
import {CellInspectorPage} from "@acton/explorer-core/pages/CellInspectorPage"
import {EmulatePage} from "@acton/explorer-core/pages/EmulatePage"

import styles from "./CatalogPages.module.css"

function ExplorerToolHeader({title}: {readonly title: string}) {
  return (
    <section className={`${styles.container} ${styles.toolHeader}`}>
      <ExplorerBreadcrumbs items={[{label: title}]} />
      <header className={styles.hero}>
        <h1 className={styles.title}>{title}</h1>
      </header>
    </section>
  )
}

export function CellInspectorExplorerPage() {
  return (
    <>
      <ExplorerToolHeader title="Cell Inspector" />
      <CellInspectorPage />
    </>
  )
}

export function EmulateExplorerPage({client}: {readonly client: TonClient}) {
  return (
    <>
      <ExplorerToolHeader title="Emulate" />
      <EmulatePage client={client} shareApiPath="/api/emulations" />
    </>
  )
}
