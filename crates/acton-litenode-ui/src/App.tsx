import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { BrowserRouter, Routes, Route, useNavigate, Link, useLocation } from 'react-router-dom';
import { Layout, Activity, Wallet, Database, Search, Sun, Moon } from 'lucide-react';
import { motion } from 'framer-motion';

import { 
  StatusIndicator,
  Card,
  SearchInput
} from '@acton/ui-shared';

import styles from './App.module.css';
import { DashboardView } from './views/Dashboard';
import { AccountExplorer } from './views/AccountExplorer';
import { AccountDetails } from './views/AccountDetails';
import { TransactionDetailsView } from './views/TransactionDetailsView';

// --- Components ---

const SidebarItem = ({ icon: Icon, label, to, active }: { icon: any, label: string, to: string, active: boolean }) => (
  <Link
    to={to}
    className={`${styles.sidebarItem} ${active ? styles.sidebarItemActive : ''}`}
  >
    {active && (
      <motion.div
        layoutId="sidebar-active-bg"
        className={styles.sidebarActiveBg}
        transition={{ type: "spring", stiffness: 380, damping: 30 }}
      />
    )}
    <Icon size={20} />
    <span>{label}</span>
  </Link>
);

// --- Layout ---

export default function App() {
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  });

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark");
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem("theme", theme);
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setTheme((prev) => (prev === "light" ? "dark" : "light"));
  }, []);

  return (
    <BrowserRouter>
      <AppContent theme={theme} onToggleTheme={toggleTheme} />
    </BrowserRouter>
  );
}

function AppContent({ theme, onToggleTheme }: { theme: string, onToggleTheme: () => void }) {
  const [nodeStatus, setNodeStatus] = useState<any>(null);
  const [online, setOnline] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');

  const fetchStatus = async () => {
    try {
      const res = await fetch('/api/v2/getMasterchainInfo', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({})
      });
      if (!res.ok) throw new Error();
      const data = await res.json();
      setNodeStatus(data.result);
      setOnline(true);
    } catch (e) {
      setOnline(false);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery) return;
    const query = searchQuery.trim();
    if (query.length > 40) {
      navigate(`/account/${query}`);
    } else if (query.match(/^[0-9a-fA-F]{64}$/)) {
      navigate(`/transaction/${query}`);
    }
    setSearchQuery('');
  };

  return (
    <div className={styles.layout}>
      {/* Sidebar */}
      <aside className={styles.sidebar}>
        <div className={styles.sidebarHeader}>
          <div className={styles.sidebarHeaderLeft}>
            <div className={styles.sidebarLogo}>
              <Activity size={28} />
              <motion.div
                className="absolute inset-0 bg-white/20"
                animate={{ opacity: [0, 0.2, 0] }}
                transition={{ duration: 2, repeat: Infinity }}
              />
            </div>
            <div>
              <h1 className={styles.logoTitle}>Acton</h1>
              <p className={styles.logoSubtitle}>LiteNode</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onToggleTheme}
            className={styles.themeButton}
            title={`Switch to ${theme === "light" ? "dark" : "light"} theme`}
          >
            {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
          </button>
        </div>

        <nav className={styles.nav}>
          <SidebarItem
            icon={Layout}
            label="Dashboard"
            to="/"
            active={location.pathname === '/'}
          />
          <SidebarItem
            icon={Wallet}
            label="Accounts"
            to="/account"
            active={location.pathname.startsWith('/account')}
          />
          <SidebarItem
            icon={Database}
            label="Explorer"
            to="/blocks"
            active={location.pathname === '/blocks'}
          />
        </nav>

        <StatusIndicator 
          online={online} 
          detailText={online && nodeStatus?.last?.seqno ? `SEQNO ${nodeStatus.last.seqno}` : "Waiting..."} 
          statusText={online ? "LiteNode Online" : "Connection Lost"}
        />
      </aside>

      {/* Main Content */}
      <main className={styles.main}>
        <header className={styles.header}>
          <div className="flex flex-col">
            <motion.h2
              key={location.pathname}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className={styles.viewTitle}
            >
              {location.pathname === '/' ? 'Network Overview' :
               location.pathname.startsWith('/account') ? 'Account Explorer' :
               location.pathname.startsWith('/transaction') ? 'Transaction Details' :
               'Blockchain Explorer'}
            </motion.h2>
          </div>

          <div className={styles.headerActions}>
            <form onSubmit={handleSearch} className="flex items-center gap-4">
              <SearchInput
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search address or hash..."
                style={{ width: '20rem' }}
              />
            </form>
          </div>
        </header>

        <div className={styles.content}>
          <Routes>
            <Route path="/" element={<DashboardView nodeStatus={nodeStatus} />} />
            <Route path="/account" element={<AccountExplorer />} />
            <Route path="/account/:address" element={<AccountDetails />} />
            <Route path="/transaction/:hash" element={<TransactionDetailsView />} />
            <Route path="/blocks" element={<Card>Coming soon...</Card>} />
          </Routes>
        </div>
      </main>
    </div>
  );
}
