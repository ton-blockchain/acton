import {useState, useEffect, useCallback} from "react"

const LOCAL_STORAGE_KEY = "TxTracerHistoryV1"
const MAX_HISTORY_LENGTH = 5

export interface TxHistoryEntry {
  readonly hash: string
  readonly exitCode?: number
  readonly testnet?: boolean
}

export function useTxHistory() {
  const [history, setHistory] = useState<TxHistoryEntry[]>([])

  useEffect(() => {
    try {
      const storedHistory = localStorage.getItem(LOCAL_STORAGE_KEY)
      if (storedHistory) {
        setHistory(JSON.parse(storedHistory) as TxHistoryEntry[])
      }
    } catch (error) {
      console.error("Failed to load transaction history from localStorage", error)
      setHistory([])
    }
  }, [])

  const addToHistory = useCallback((entry: TxHistoryEntry) => {
    if (!entry.hash || entry.hash.trim() === "") {
      return
    }
    setHistory(prevHistory => {
      const newHistory = prevHistory.filter(item => item.hash !== entry.hash)
      newHistory.unshift(entry)
      const finalHistory = newHistory.slice(0, MAX_HISTORY_LENGTH)

      try {
        localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(finalHistory))
      } catch (error) {
        console.error("Failed to save transaction history to localStorage", error)
      }
      return finalHistory
    })
  }, [])

  const removeFromHistory = useCallback((txHashToRemove: string) => {
    setHistory(prevHistory => {
      const newHistory = prevHistory.filter(item => item.hash !== txHashToRemove)
      try {
        localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(newHistory))
      } catch (error) {
        console.error("Failed to save updated transaction history to localStorage", error)
      }
      return newHistory
    })
  }, [])

  return {history, addToHistory, removeFromHistory}
}
