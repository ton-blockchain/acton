import {Address} from "@ton/core"
import {AlertCircle, History, Search, X} from "lucide-react"
import * as React from "react"
import {useCallback, useEffect, useState} from "react"
import {useNavigate} from "react-router-dom"

import {
  EXPLORER_HISTORY_STORAGE_KEY,
  readExplorerInput,
  writeExplorerInput,
} from "../explorerResume"

import styles from "./ExplorerIndexPage.module.css"

export const ExplorerIndexPage: React.FC = () => {
  const [input, setInput] = useState(() => readExplorerInput())
  const [history, setHistory] = useState<string[]>([])
  const [isFocused, setIsFocused] = useState(false)
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const navigate = useNavigate()

  useEffect(() => {
    const savedHistory = localStorage.getItem(EXPLORER_HISTORY_STORAGE_KEY)
    if (savedHistory) {
      try {
        setHistory(JSON.parse(savedHistory) as string[])
      } catch (error) {
        console.error("Failed to parse history", error)
      }
    }
  }, [])

  const addToHistory = useCallback(
    (address: string) => {
      const newHistory = [address, ...history.filter(a => a !== address)].slice(0, 5)
      setHistory(newHistory)
      localStorage.setItem(EXPLORER_HISTORY_STORAGE_KEY, JSON.stringify(newHistory))
    },
    [history],
  )

  const removeFromHistory = useCallback(
    (e: React.MouseEvent, address: string) => {
      e.stopPropagation()
      const newHistory = history.filter(a => a !== address)
      setHistory(newHistory)
      localStorage.setItem(EXPLORER_HISTORY_STORAGE_KEY, JSON.stringify(newHistory))
      setShowHistoryDropdown(newHistory.length > 0)
    },
    [history],
  )

  const handleSearch = useCallback(
    (address: string) => {
      const trimmed = address.trim()
      if (!trimmed) return

      try {
        Address.parse(trimmed)
        setError(undefined)
        writeExplorerInput(trimmed)
        addToHistory(trimmed)
        setShowHistoryDropdown(false)
        void navigate(`/explorer/address/${trimmed}`)
      } catch {
        setError("Invalid address, only standard internal address is allowed")
      }
    },
    [addToHistory, navigate],
  )

  return (
    <div className={styles.inputPage}>
      <div className={styles.centeredInputContainer}>
        <header className={styles.logoSection}>
          <h1 className={styles.logoTitle}>Explore any address</h1>
        </header>

        <section className={styles.inputCard}>
          <div
            className={`${styles.inputWrapper} ${isFocused ? styles.focused : ""} ${error ? styles.inputError : ""}`}
          >
            <div className={styles.searchIcon} aria-hidden="true">
              <Search size={20} />
            </div>
            <input
              type="text"
              spellCheck="false"
              className={styles.input}
              placeholder="Search by address or hash"
              value={input}
              onChange={e => {
                const nextInput = e.target.value
                setInput(nextInput)
                writeExplorerInput(nextInput)
                if (error) setError(undefined)
              }}
              onKeyDown={e => e.key === "Enter" && handleSearch(input)}
              onFocus={() => {
                setIsFocused(true)
                if (!error && history.length > 0) {
                  setShowHistoryDropdown(true)
                }
              }}
              onBlur={() => {
                setIsFocused(false)
                setTimeout(() => setShowHistoryDropdown(false), 100)
              }}
              onClick={() => {
                if (isFocused) {
                  setShowHistoryDropdown(true)
                }
              }}
            />
          </div>

          {showHistoryDropdown && history.length > 0 && !error && (
            <div className={styles.historyDropdown}>
              {history.map(addr => (
                <div key={addr} className={styles.historyItem}>
                  <button
                    type="button"
                    className={styles.historyItemButton}
                    onClick={() => handleSearch(addr)}
                  >
                    <History size={16} className={styles.historyItemIcon} aria-hidden="true" />
                    <span className={styles.historyAddr}>{addr}</span>
                  </button>
                  <button
                    type="button"
                    className={styles.historyItemDeleteButton}
                    onMouseDown={e => e.preventDefault()}
                    onClick={e => removeFromHistory(e, addr)}
                    title="Remove from history"
                  >
                    <X size={14} />
                  </button>
                </div>
              ))}
            </div>
          )}

          {error && (
            <div className={styles.errorMessage}>
              <AlertCircle size={14} className={styles.errorIcon} />
              <span>{error}</span>
            </div>
          )}
        </section>
      </div>

    </div>
  )
}
