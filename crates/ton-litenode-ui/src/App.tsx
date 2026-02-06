import React, { useState, useEffect, useMemo } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { Address } from "@ton/core";
import { TonClient } from "./explorer/api/client";
import { TonExplorer } from "./explorer/TonExplorer";
import { TransactionPage } from "./explorer/pages/TransactionPage";
import "@acton/shared-ui/styles/tokens.css";
import "./index.css";
import styles from "./App.module.css";
import { Sun, Moon } from "lucide-react";

export const App: React.FC = () => {
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    );
  });

  const client = useMemo(() => new TonClient("http://localhost:3010/api"), []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark");
    localStorage.setItem("theme", theme);
  }, [theme]);

  return (
    <BrowserRouter>
      <div className={styles.app}>
        <header className={styles.header}>
          <div className={styles.headerContent}>
            <div className={styles.logoSection}>
              <div className={styles.logo} onClick={() => window.location.href = "/"}>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="white"><path d="M12 2L2 19h20L12 2zm0 3.8L18.4 17H5.6L12 5.8z"/></svg>
              </div>
              <nav className={styles.nav}>
                <div className={`${styles.navItem} ${styles.navItemActive}`}>STATS</div>
                <div className={styles.navItem}>TOKENS</div>
                <div className={styles.navItem}>APPS</div>
                <div className={styles.navItem}>MORE</div>
              </nav>
            </div>
            
            <div className={styles.searchSection}>
              <div className={styles.searchBox}>
                <input
                  type="text"
                  placeholder="Search by address or hash"
                  className={styles.searchInput}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      const val = (e.target as HTMLInputElement).value;
                      if (val.length === 64) {
                        window.location.href = `/tx/${val}`;
                      } else {
                        try {
                          const formatted = Address.parse(val).toString({ testOnly: true });
                          window.location.href = `/?address=${formatted}`;
                        } catch {
                          window.location.href = `/?address=${val}`;
                        }
                      }
                    }
                  }}
                />
              </div>
              <button
                onClick={() => setTheme(theme === "light" ? "dark" : "light")}
                className={styles.themeButton}
              >
                {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
              </button>
            </div>
          </div>
        </header>
        <main className={styles.main}>
          <Routes>
            <Route path="/" element={<TonExplorerWrapper client={client} />} />
            <Route path="/tx/:hash" element={<TransactionPage client={client} />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
};

const TonExplorerWrapper: React.FC<{ client: TonClient }> = ({ client }) => {
  const [searchAddress, setSearchAddress] = useState("");

  // Handle URL address on initial load and browser navigation (back/forward)
  useEffect(() => {
    const handleLocationChange = () => {
      const urlParams = new URLSearchParams(window.location.search);
      const urlAddress = urlParams.get('address');
      setSearchAddress(urlAddress || "");
    };

    handleLocationChange(); // Initial load

    window.addEventListener('popstate', handleLocationChange);
    return () => window.removeEventListener('popstate', handleLocationChange);
  }, []);

  const handleSearch = (addr: string) => {
    let finalAddr = addr;
    try {
      if (addr) {
        finalAddr = Address.parse(addr).toString({ testOnly: true });
      }
    } catch {
      // Keep original if not a valid TON address
    }
    
    setSearchAddress(finalAddr);
    const url = new URL(window.location.href);
    if (finalAddr) {
      url.searchParams.set('address', finalAddr);
    } else {
      url.searchParams.delete('address');
    }
    window.history.pushState({}, '', url);
  };

  return (
    <TonExplorer 
      client={client} 
      externalAddress={searchAddress} 
      onAddressChange={handleSearch} 
    />
  );
};
