import type React from "react"

import type {TonClient} from "../explorer/api/client"

import {DashboardNavigation} from "./DashboardNavigation"
import styles from "./DashboardPage.module.css"

interface DashboardPageProps {
  readonly client: TonClient
  readonly theme: string
  readonly setTheme: (theme: string) => void
  readonly children?: React.ReactNode
  readonly embedded?: boolean
}

export const DashboardPage: React.FC<DashboardPageProps> = ({
  children,
  client,
  embedded = false,
  theme,
  setTheme,
}) => {
  return (
    <div className={styles.page}>
      <DashboardNavigation client={client} theme={theme} setTheme={setTheme} />

      <section className={styles.contentArea}>
        <main className={`${styles.content} ${embedded ? styles.contentEmbedded : ""}`}>
          {embedded ? <div className={styles.embeddedPage}>{children}</div> : children}
        </main>
      </section>
    </div>
  )
}
