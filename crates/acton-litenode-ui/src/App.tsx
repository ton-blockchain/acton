import * as React from "react";
import { useEffect, useMemo, useState } from "react";
import {
  BrowserRouter,
  Navigate,
  Route,
  Routes,
  useNavigate,
} from "react-router-dom";

import { Moon, Sun } from "lucide-react";

import { TonClient } from "./explorer/api/client";
import { toTestnetAddress } from "./explorer/components/utils";
import { AddressBookProvider } from "./explorer/hooks/useAddressBook";
import { AccountPage } from "./explorer/pages/AccountPage";
import { ExplorerIndexPage } from "./explorer/pages/ExplorerIndexPage";
import { TransactionPage } from "./explorer/pages/TransactionPage";
import "@acton/shared-ui/styles/tokens.css";
import "./index.css";
import styles from "./App.module.css";

export const App: React.FC = () => {
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (globalThis.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light")
    );
  });

  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: "http://localhost:3010/api/v2",
        v3BaseUrl: "http://localhost:3010/api/v3",
        addressNameBaseUrl: "http://localhost:3010",
      }),
    [],
  );

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark");
    localStorage.setItem("theme", theme);
  }, [theme]);

  return (
    <BrowserRouter>
      <AddressBookProvider client={client}>
        <div className={styles.app}>
          <header className={styles.header}>
            <div className={styles.headerContent}>
              <div className={styles.logoSection}>
                <button
                  type="button"
                  className={styles.logo}
                  onClick={() => {
                    globalThis.location.href = "/";
                  }}
                >
                  <svg
                    width="20"
                    height="20"
                    viewBox="0 0 24 24"
                    fill="white"
                    role="img"
                    aria-label="Logo"
                  >
                    <title>Logo</title>
                    <path d="M12 2L2 19h20L12 2zm0 3.8L18.4 17H5.6L12 5.8z" />
                  </svg>
                </button>
                <nav className={styles.nav}>
                  <div className={`${styles.navItem} ${styles.navItemActive}`}>
                    Explorer
                  </div>
                  <div className={styles.navItem}>TOKENS</div>
                </nav>
              </div>

              <HeaderSearch />

              <div className={styles.themeSection}>
                <button
                  type="button"
                  onClick={() => setTheme(theme === "light" ? "dark" : "light")}
                  className={styles.themeButton}
                  aria-label="Toggle theme"
                >
                  <div className={styles.themeIconWrapper}>
                    <Sun
                      className={`${styles.themeIcon} ${theme === "light" ? styles.active : ""}`}
                      size={18}
                    />
                    <Moon
                      className={`${styles.themeIcon} ${theme === "dark" ? styles.active : ""}`}
                      size={18}
                    />
                  </div>
                </button>
              </div>
            </div>
          </header>
          <main className={styles.main}>
            <Routes>
              <Route path="/" element={<Navigate to="/explorer" replace />} />
              <Route path="/explorer" element={<ExplorerIndexPage />} />
              <Route
                path="/explorer/address/:address"
                element={<AccountPage client={client} />}
              />
              <Route
                path="/tx/:hash"
                element={<TransactionPage client={client} />}
              />
              <Route path="*" element={<Navigate to="/explorer" replace />} />
            </Routes>
          </main>
        </div>
      </AddressBookProvider>
    </BrowserRouter>
  );
};

const HeaderSearch: React.FC = () => {
  const navigate = useNavigate();
  return (
    <div className={styles.searchSection}>
      <div className={styles.searchBox}>
        <input
          type="text"
          placeholder="Search by address or hash"
          className={styles.searchInput}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const val = (e.target as HTMLInputElement).value;
              if (val.length === 64) {
                navigate(`/tx/${val}`);
              } else {
                const formatted = toTestnetAddress(val);
                navigate(`/explorer/address/${formatted ?? val}`);
              }
            }
          }}
        />
      </div>
    </div>
  );
};
