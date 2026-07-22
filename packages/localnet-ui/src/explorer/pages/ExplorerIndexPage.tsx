import {Binary, Play} from "lucide-react"
import {Link} from "react-router-dom"
import type {FC} from "react"

import type {TonClient} from "../api/client"
import {ExplorerSearch} from "../components/ExplorerSearch"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"

import styles from "./ExplorerIndexPage.module.css"

interface ExplorerIndexPageProps {
  readonly client: TonClient
  readonly fillAvailableHeight?: boolean
}

export const ExplorerIndexPage: FC<ExplorerIndexPageProps> = ({
  client,
  fillAvailableHeight = false,
}) => {
  const routes = useExplorerRoutePaths()
  const pageClassName = fillAvailableHeight
    ? `${styles.inputPage} ${styles.inputPageFillAvailableHeight}`
    : styles.inputPage

  return (
    <div className={pageClassName}>
      <div className={styles.centeredInputContainer}>
        <header className={styles.logoSection}>
          <h1 className={styles.logoTitle}>
            <span>Explore</span>
            <span className={styles.logoTitleAccent}>any address</span>
          </h1>
        </header>

        <div className={styles.searchArea}>
          <ExplorerSearch autoFocus client={client} />
        </div>

        <nav className={styles.toolCards} aria-label="Developer tools">
          <Link className={styles.toolCard} to={routes.emulatePath}>
            <span className={`${styles.toolCardIcon} ${styles.emulateIcon}`}>
              <Play aria-hidden="true" />
            </span>
            <span className={styles.toolCardBadge}>Emulator</span>
            <span className={styles.toolCardTitle}>Emulate</span>
            <span className={styles.toolCardDescription}>
              Build and emulate TON messages, inspect the resulting transaction tree, and trace
              execution.
            </span>
          </Link>

          <Link className={styles.toolCard} to={routes.cellPath}>
            <span className={`${styles.toolCardIcon} ${styles.cellInspectorIcon}`}>
              <Binary aria-hidden="true" />
            </span>
            <span className={styles.toolCardBadge}>Inspect</span>
            <span className={styles.toolCardTitle}>Cell Inspector</span>
            <span className={styles.toolCardDescription}>
              Decode Cell and BoC data, inspect bits and references, and parse values with ABI or
              custom TL-B schemas.
            </span>
          </Link>
        </nav>
      </div>

      <footer className={styles.footer}>
        <span className={styles.footerCredit}>
          <span className={styles.footerBrand}>actonscan</span>
          <span className={styles.footerBy}>by</span>
          <a
            className={styles.footerCreditLink}
            href="https://t.me/toncore"
            target="_blank"
            rel="noreferrer"
          >
            TON Core
          </a>
        </span>
        <a href="https://ton-blockchain.github.io/acton/docs/welcome">Documentation</a>
        <a href="https://github.com/ton-blockchain/acton">GitHub</a>
      </footer>
    </div>
  )
}
