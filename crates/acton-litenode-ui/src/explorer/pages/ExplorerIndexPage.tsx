import { Address } from "@ton/core";
import { AlertCircle, History, Search, X } from "lucide-react";
import * as React from "react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

import styles from "./ExplorerIndexPage.module.css";

const STORAGE_KEY = "explorer_history";

export const ExplorerIndexPage: React.FC = () => {
  const [input, setInput] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [isFocused, setIsFocused] = useState(false);
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    const savedHistory = localStorage.getItem(STORAGE_KEY);
    if (savedHistory) {
      try {
        setHistory(JSON.parse(savedHistory) as string[]);
      } catch (error_) {
        console.error("Failed to parse history", error_);
      }
    }
  }, []);

  const addToHistory = (address: string) => {
    const newHistory = [address, ...history.filter((a) => a !== address)].slice(
      0,
      5,
    );
    setHistory(newHistory);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(newHistory));
  };

  const removeFromHistory = (e: React.MouseEvent, address: string) => {
    e.stopPropagation();
    const newHistory = history.filter((a) => a !== address);
    setHistory(newHistory);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(newHistory));
  };

  const handleSearch = (address: string) => {
    const trimmed = address.trim();
    if (!trimmed) return;

    try {
      Address.parse(trimmed);
      setError(null);
      addToHistory(trimmed);
      setShowHistoryDropdown(false);
      navigate(`/explorer/address/${trimmed}`);
    } catch {
      setError("Invalid address, only standard internal address is allowed");
    }
  };

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
              onChange={(e) => {
                setInput(e.target.value);
                if (error) setError(null);
              }}
              onKeyDown={(e) => e.key === "Enter" && handleSearch(input)}
              onFocus={() => {
                setIsFocused(true);
              }}
              onBlur={() => {
                setIsFocused(false);
                setTimeout(() => setShowHistoryDropdown(false), 100);
              }}
              onClick={() => {
                if (isFocused) {
                  setShowHistoryDropdown(true);
                }
              }}
            />
          </div>

          {showHistoryDropdown && history.length > 0 && !error && (
            <ul
              className={styles.historyDropdown}
              onMouseDown={(e) => e.preventDefault()}
            >
              {history.map((addr) => (
                <li
                  key={addr}
                  className={styles.historyItem}
                  onMouseDown={(e) => e.preventDefault()} // Prevent blur before click
                  onClick={() => handleSearch(addr)}
                >
                  <History
                    size={16}
                    className={styles.historyItemIcon}
                    aria-hidden="true"
                  />
                  <span className={styles.historyAddr}>{addr}</span>
                  <button
                    type="button"
                    className={styles.historyItemDeleteButton}
                    onClick={(e) => removeFromHistory(e, addr)}
                    title="Remove from history"
                  >
                    <X size={14} />
                  </button>
                </li>
              ))}
            </ul>
          )}

          {error && (
            <div className={styles.errorMessage}>
              <AlertCircle size={14} className={styles.errorIcon} />
              <span>{error}</span>
            </div>
          )}
        </section>
      </div>

      <footer className={styles.footer}>
        <span className={styles.createBy}>Powered by TON Litenode</span>
      </footer>
    </div>
  );
};
