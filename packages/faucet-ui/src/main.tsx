import {ThemeProvider, ThemeSwitch, ToastProvider} from "@acton/ui"
import {Droplets, Github} from "lucide-react"
import {createRoot} from "react-dom/client"
import {BrowserRouter} from "react-router"

import {FaucetPage, type FaucetBalanceClient} from "./FaucetPage"
import styles from "./App.module.css"
import "./global.css"

const testnetClient: FaucetBalanceClient = {
  async getAddressInformation(address) {
    const url = new URL("https://testnet.toncenter.com/api/v2/getAddressInformation")
    url.searchParams.set("address", address)
    const response = await fetch(url)
    if (!response.ok) throw new Error("Could not load the Testnet balance")
    const payload = (await response.json()) as {
      readonly ok?: boolean
      readonly result?: {readonly balance?: string}
    }
    if (!payload.ok || typeof payload.result?.balance !== "string") {
      throw new Error("Invalid Testnet balance response")
    }
    return {balance: payload.result.balance}
  },
}

function testnetAddressPath(address: string): string {
  return `https://actonscan.com/address/${encodeURIComponent(address)}?network=testnet`
}

function App() {
  return (
    <ThemeProvider storageKey="ton-faucet-theme">
      <BrowserRouter>
        <ToastProvider>
          <div className={styles.appShell}>
            <header className={styles.header}>
              <div className={styles.headerInner}>
                <a className={styles.brand} href="/" aria-label="TON Faucet home">
                  <Droplets size={26} aria-hidden="true" />
                  <span>TON Faucet</span>
                </a>
                <div className={styles.actions}>
                  <ThemeSwitch />
                  <a
                    className={styles.github}
                    href="https://github.com/ton-blockchain/acton"
                    target="_blank"
                    rel="noreferrer"
                    aria-label="Open GitHub"
                  >
                    <Github size={18} aria-hidden="true" />
                  </a>
                </div>
              </div>
            </header>
            <main className={styles.main}>
              <FaucetPage
                testnetClient={testnetClient}
                faucetBaseUrl={import.meta.env.VITE_FAUCET_URL || "/"}
                addressPath={testnetAddressPath}
              />
            </main>
            <footer className={styles.footer}>
              <span className={styles.footerCredit}>
                <span className={styles.footerBrand}>TON Faucet</span>
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
              <nav className={styles.footerLinks} aria-label="Footer navigation">
                <a href="https://ton-blockchain.github.io/acton/docs/wallets#fund-a-wallet-on-testnet">
                  Documentation
                </a>
                <a href="https://github.com/ton-blockchain/acton" target="_blank" rel="noreferrer">
                  GitHub
                </a>
              </nav>
            </footer>
          </div>
        </ToastProvider>
      </BrowserRouter>
    </ThemeProvider>
  )
}

const root = document.getElementById("root")
if (!root) throw new Error("Faucet root element was not found")
createRoot(root).render(<App />)
