//  SPDX-License-Identifier: MIT
//  Copyright © 2026 TON Core
import type React from "react"
import {useEffect, useState, useCallback, useRef} from "react"

import {WalletCard} from "./components/WalletCard"
import type {WalletInfo, WalletMessage, WalletVSCodeAPI} from "./wallet-types"

import styles from "./WalletApp.module.css"

const WALLET_VERSIONS = [
  "v5r1",
  "v4r2",
  "v4r1",
  "v3r2",
  "v3r1",
  "v2r2",
  "v2r1",
  "v1r3",
  "v1r2",
  "v1r1",
  "highloadv2r2",
  "highloadv2r1",
  "highloadv2",
  "highloadv1r2",
  "highloadv1r1",
]

interface Props {
  readonly vscode: WalletVSCodeAPI
}

export const WalletApp: React.FC<Props> = ({vscode}) => {
  const [wallets, setWallets] = useState<readonly WalletInfo[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [airdropStatus, setAirdropStatus] = useState<
    Readonly<Record<string, "idle" | "requesting">>
  >({})
  const [showNewWalletForm, setShowNewWalletForm] = useState(false)
  const [showImportWalletForm, setShowImportWalletForm] = useState(false)

  const newNameRef = useRef<HTMLInputElement>(null)
  const importNameRef = useRef<HTMLInputElement>(null)

  const [newName, setNewName] = useState("")
  const [newVersion, setNewVersion] = useState("v5r1")
  const [newGlobal, setNewGlobal] = useState(false)
  const [newSecure, setNewSecure] = useState(true)

  const [importName, setImportName] = useState("")
  const [importMnemonic, setImportMnemonic] = useState("")
  const [importVersion, setImportVersion] = useState("v5r1")
  const [importGlobal, setImportGlobal] = useState(false)
  const [importSecure, setImportSecure] = useState(true)

  const closeModals = useCallback(() => {
    setShowNewWalletForm(false)
    setShowImportWalletForm(false)
  }, [])

  useEffect(() => {
    if (showNewWalletForm) {
      newNameRef.current?.focus()
    }
  }, [showNewWalletForm])

  useEffect(() => {
    if (showImportWalletForm) {
      importNameRef.current?.focus()
    }
  }, [showImportWalletForm])

  useEffect(() => {
    const handler = (event: MessageEvent<WalletMessage>): void => {
      const message = event.data
      switch (message.type) {
        case "updateWallets": {
          setWallets(message.wallets)
          setIsLoading(message.isLoading)
          break
        }
        case "airdropStatus": {
          setAirdropStatus(prev => ({
            ...prev,
            [message.walletName]: message.status,
          }))
          break
        }
        case "showNewWalletForm": {
          setShowNewWalletForm(true)
          break
        }
        case "showImportWalletForm": {
          setShowImportWalletForm(true)
          break
        }
      }
    }

    const keyHandler = (e: KeyboardEvent): void => {
      if (e.key === "Escape") {
        closeModals()
      }
    }

    globalThis.addEventListener("message", handler)
    globalThis.addEventListener("keydown", keyHandler)
    vscode.postMessage({type: "webviewReady"})

    return () => {
      globalThis.removeEventListener("message", handler)
      globalThis.removeEventListener("keydown", keyHandler)
    }
  }, [vscode, closeModals])

  const handleCreateWallet = (e: React.FormEvent): void => {
    e.preventDefault()
    vscode.postMessage({
      type: "newWallet",
      name: newName,
      version: newVersion,
      global: newGlobal,
      secure: newSecure,
    })
    setShowNewWalletForm(false)
    setNewName("")
  }

  const handleImportWallet = (e: React.FormEvent): void => {
    e.preventDefault()
    vscode.postMessage({
      type: "importWallet",
      name: importName,
      mnemonic: importMnemonic,
      version: importVersion,
      global: importGlobal,
      secure: importSecure,
    })
    setShowImportWalletForm(false)
    setImportName("")
    setImportMnemonic("")
  }

  return (
    <div className={styles.container}>
      {isLoading && (
        <div className={styles.loadingOverlay}>
          <div className={styles.loadingSpinner}></div>
          <div className={styles.loadingText}>Loading wallets...</div>
        </div>
      )}

      {(showNewWalletForm || showImportWalletForm) && (
        <button
          type="button"
          className={styles.modalOverlay}
          aria-label="Close dialog"
          onClick={closeModals}
        />
      )}

      {showNewWalletForm && (
        <div className={styles.modal}>
          <h3>New Wallet</h3>
          <form onSubmit={handleCreateWallet}>
            <div className={styles.formGroup}>
              <label htmlFor="new-wallet-name">Name:</label>
              <input
                id="new-wallet-name"
                ref={newNameRef}
                type="text"
                value={newName}
                onChange={e => {
                  setNewName(e.target.value)
                }}
                required
              />
            </div>
            <div className={styles.formGroup}>
              <label htmlFor="new-wallet-version">Version:</label>
              <div className={styles.selectWrapper}>
                <select
                  id="new-wallet-version"
                  value={newVersion}
                  onChange={e => {
                    setNewVersion(e.target.value)
                  }}
                >
                  {WALLET_VERSIONS.map(v => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className={styles.checkboxGroup}>
              <input
                id="new-wallet-global"
                type="checkbox"
                checked={newGlobal}
                onChange={e => {
                  setNewGlobal(e.target.checked)
                }}
              />
              <label htmlFor="new-wallet-global">Store in global wallets</label>
            </div>
            <div className={styles.checkboxGroup}>
              <input
                id="new-wallet-secure"
                type="checkbox"
                checked={newSecure}
                onChange={e => {
                  setNewSecure(e.target.checked)
                }}
              />
              <label htmlFor="new-wallet-secure">Use secure native store</label>
            </div>
            <div className={styles.formActions}>
              <button
                type="button"
                onClick={() => {
                  setShowNewWalletForm(false)
                }}
              >
                Cancel
              </button>
              <button type="submit" className={styles.primaryButton}>
                Create
              </button>
            </div>
          </form>
        </div>
      )}

      {showImportWalletForm && (
        <div className={styles.modal}>
          <h3>Import Wallet</h3>
          <form onSubmit={handleImportWallet}>
            <div className={styles.formGroup}>
              <label htmlFor="import-wallet-name">Name:</label>
              <input
                id="import-wallet-name"
                ref={importNameRef}
                type="text"
                value={importName}
                onChange={e => {
                  setImportName(e.target.value)
                }}
                required
              />
            </div>
            <div className={styles.formGroup}>
              <label htmlFor="import-wallet-mnemonic">Mnemonic:</label>
              <textarea
                id="import-wallet-mnemonic"
                value={importMnemonic}
                onChange={e => {
                  setImportMnemonic(e.target.value)
                }}
                required
                rows={3}
                placeholder="24 words separated by spaces"
              />
            </div>
            <div className={styles.formGroup}>
              <label htmlFor="import-wallet-version">Version:</label>
              <div className={styles.selectWrapper}>
                <select
                  id="import-wallet-version"
                  value={importVersion}
                  onChange={e => {
                    setImportVersion(e.target.value)
                  }}
                >
                  {WALLET_VERSIONS.map(v => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className={styles.checkboxGroup}>
              <input
                id="import-wallet-global"
                type="checkbox"
                checked={importGlobal}
                onChange={e => {
                  setImportGlobal(e.target.checked)
                }}
              />
              <label htmlFor="import-wallet-global">Store in global wallets</label>
            </div>
            <div className={styles.checkboxGroup}>
              <input
                id="import-wallet-secure"
                type="checkbox"
                checked={importSecure}
                onChange={e => {
                  setImportSecure(e.target.checked)
                }}
              />
              <label htmlFor="import-wallet-secure">Use secure native store</label>
            </div>
            <div className={styles.formActions}>
              <button
                type="button"
                onClick={() => {
                  setShowImportWalletForm(false)
                }}
              >
                Cancel
              </button>
              <button type="submit" className={styles.primaryButton}>
                Import
              </button>
            </div>
          </form>
        </div>
      )}

      <div className={styles.walletList}>
        {!isLoading && wallets.length === 0 && <div className={styles.empty}>No wallets found</div>}
        {wallets.map(wallet => (
          <WalletCard
            key={wallet.name}
            info={wallet}
            airdropStatus={airdropStatus[wallet.name] ?? "idle"}
            onAirdrop={() => {
              vscode.postMessage({type: "airdrop", walletName: wallet.name})
            }}
            onCopy={() => {
              vscode.postMessage({type: "copyAddress", address: wallet.address})
            }}
            onOpenExplorer={() => {
              vscode.postMessage({type: "openInExplorer", address: wallet.address})
            }}
          />
        ))}
      </div>
    </div>
  )
}
