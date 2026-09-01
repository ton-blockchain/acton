import {AppShell} from "../components/AppShell"
import {SearchBox} from "../components/SearchBox"
import styles from "./HomePage.module.css"

export function HomePage() {
  return (
    <AppShell>
      <div className={styles.inputPage}>
        <section className={styles.centeredInputContainer} aria-labelledby="home-title">
          <header className={styles.logoSection}>
            <h1 id="home-title" className={styles.logoTitle}>
              <span>Verify</span>
              <span className={styles.logoTitleAccent}>any contract</span>
            </h1>
          </header>
          <SearchBox autoFocus />
        </section>
        <footer className={styles.footer}>
          <span className={styles.footerCredit}>
            <span className={styles.footerBrand}>TON Verifier</span>
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
          <a href="https://ton-blockchain.github.io/acton/docs/verify">Documentation</a>
          <a href="https://github.com/ton-blockchain/acton" target="_blank" rel="noreferrer">
            GitHub
          </a>
        </footer>
        <section className={styles.srOnly} aria-labelledby="home-description">
          <h2 id="home-description">TON source registry</h2>
          <p>
            Search by contract address or code hash. The verifier checks the source registry and
            returns the stored source bundle when the code hash is verified.
          </p>
        </section>
      </div>
    </AppShell>
  )
}
