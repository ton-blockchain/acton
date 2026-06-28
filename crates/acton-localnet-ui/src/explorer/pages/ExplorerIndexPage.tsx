import type {FC} from "react"

import {ExplorerSearch} from "../components/ExplorerSearch"

import styles from "./ExplorerIndexPage.module.css"

export const ExplorerIndexPage: FC = () => {
  return (
    <div className={styles.inputPage}>
      <div className={styles.centeredInputContainer}>
        <header className={styles.logoSection}>
          <h1 className={styles.logoTitle}>
            <span>Explore</span>
            <span className={styles.logoTitleAccent}>any address</span>
          </h1>
        </header>

        <ExplorerSearch autoFocus />
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
