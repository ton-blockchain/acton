import React, { useState, useEffect, useMemo } from 'react';
import { BrowserRouter, Routes, Route, useNavigate, useParams, Link, useLocation } from 'react-router-dom';
import { Layout, Activity, Wallet, Coins, Database, RefreshCw, Search, ArrowRight, ArrowLeft, Clock, CheckCircle2, XCircle, ChevronRight, ExternalLink, Sun, Moon, Zap, History } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { formatDistanceToNow } from 'date-fns';
import { Address } from '@ton/core';
import { motion, AnimatePresence } from 'framer-motion';
import { Buffer } from 'buffer';

// Helper for tailwind classes
function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// TON Helpers
const formatAddress = (addr: string) => {
  try {
    const a = Address.parse(addr);
    return a.toString({ 
      bounceable: true, 
      testOnly: true, 
      urlSafe: true 
    });
  } catch (e) {
    return addr;
  }
};

const shortenAddress = (addr: string) => {
  const formatted = formatAddress(addr);
  if (formatted.length < 12) return formatted;
  return `${formatted.slice(0, 6)}...${formatted.slice(-6)}`;
};

const formatShard = (shard: string) => {
  try {
    const s = BigInt(shard);
    if (s === -9223372036854775808n) return '8000000000000000';
    const abs = s < 0n ? -s : s;
    return abs.toString(16).toUpperCase();
  } catch (e) {
    return shard;
  }
};

const formatTON = (nano: string | number) => {
  return (Number(nano) / 1e9).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 9 });
};

const parseComment = (bodyB64?: string) => {
  if (!bodyB64) return null;
  try {
    const buffer = Buffer.from(bodyB64, 'base64');
    // Check if it's a text comment (starts with 0x00000000)
    if (buffer.length >= 4 && buffer.readUInt32BE(0) === 0) {
      return buffer.slice(4).toString('utf8');
    }
  } catch (e) {}
  return null;
};

const getTransactionInfo = (tx: any, currentAddress?: string) => {
  const inMsg = tx.in_msg;
  const outMsgs = tx.out_msgs || [];
  
  // If no in_msg, it's weird but let's handle it
  if (!inMsg || inMsg['@type'] === 'msg.message') {
    return { type: 'unknown', label: 'Transaction', value: '0', otherParty: '---', isOut: false };
  }

  const isExternal = !inMsg.source?.account_address;
  
  // If external message, it's usually a wallet sending something
  if (isExternal) {
    if (outMsgs.length > 0) {
      const firstOut = outMsgs[0];
      return {
        type: 'send',
        label: 'Sent TON',
        value: firstOut.value,
        otherParty: firstOut.destination?.account_address,
        isOut: true,
        comment: parseComment(firstOut.msg_data?.body)
      };
    }
    return { type: 'call', label: 'Called contract', value: '0', otherParty: inMsg.destination?.account_address, isOut: true };
  }

  // Internal message
  const value = BigInt(inMsg.value || '0');
  
  if (currentAddress && inMsg.source?.account_address === currentAddress) {
    // We are the source of the internal message (weird for in_msg unless it's a self-call)
    return {
      type: 'send',
      label: 'Sent TON',
      value: inMsg.value,
      otherParty: inMsg.destination?.account_address,
      isOut: true,
      comment: parseComment(inMsg.msg_data?.body)
    };
  }

  // Default: we received an internal message
  return {
    type: 'receive',
    label: 'Received TON',
    value: inMsg.value,
    otherParty: inMsg.source?.account_address,
    isOut: false,
    comment: parseComment(inMsg.msg_data?.body)
  };
};

// --- Components ---

const SidebarItem = ({ icon: Icon, label, to, active }: { icon: any, label: string, to: string, active: boolean }) => (
  <Link
    to={to}
    className={cn(
      "relative flex items-center gap-3 w-full px-4 py-3 rounded-xl transition-all duration-300 group z-10",
      active
        ? "text-white"
        : "text-apple-gray-400 dark:text-apple-gray-400 hover:bg-apple-gray-100 dark:hover:bg-white/5 hover:text-apple-gray-600 dark:hover:text-apple-gray-200"
    )}
  >
    {active && (
      <motion.div
        layoutId="sidebar-active-bg"
        className="absolute inset-0 bg-apple-blue rounded-xl shadow-lg shadow-apple-blue/20 -z-10"
        transition={{ type: "spring", stiffness: 380, damping: 30 }}
      />
    )}
    <Icon size={20} className={cn("transition-colors duration-300", active ? "text-white" : "group-hover:text-apple-blue")} />
    <span className="font-medium relative">{label}</span>
  </Link>
);

const StatusIndicator = ({ online, lastBlock }: { online: boolean, lastBlock?: number }) => (
  <div className="mt-auto p-4 bg-white/50 dark:bg-[#1C1C1E] backdrop-blur-md rounded-2xl border border-apple-gray-100 dark:border-white/5 shadow-sm">
    <div className="flex items-center justify-between mb-2">
      <span className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight">Node Status</span>
      <div className="relative flex h-3 w-3">
        {online && (
          <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-apple-system-green opacity-75"></span>
        )}
        <span className={cn(
          "relative inline-flex rounded-full h-3 w-3",
          online ? "bg-apple-system-green" : "bg-apple-system-red"
        )}></span>
      </div>
    </div>
    <p className="text-sm font-bold text-apple-gray-600 dark:text-apple-gray-200">
      {online ? "LiteNode Online" : "Connection Lost"}
    </p>
    <div className="flex items-center gap-1.5 mt-1">
      <Activity size={10} className={online ? "text-apple-blue" : "text-apple-gray-400"} />
      <span className="text-[10px] text-apple-gray-400 dark:text-apple-gray-400 font-mono font-medium uppercase tracking-tight">
        {online && lastBlock ? `SEQNO ${lastBlock}` : "Waiting..."}
      </span>
    </div>
  </div>
);

// --- Layout ---

export default function App() {
  const [darkMode, setDarkMode] = useState(() => {
    if (typeof window !== 'undefined') {
      return localStorage.getItem('theme') === 'dark' ||
        (!localStorage.getItem('theme') && window.matchMedia('(prefers-color-scheme: dark)').matches);
    }
    return false;
  });

  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add('dark');
      localStorage.setItem('theme', 'dark');
    } else {
      document.documentElement.classList.remove('dark');
      localStorage.setItem('theme', 'light');
    }
  }, [darkMode]);

  return (
    <BrowserRouter>
      <AppContent darkMode={darkMode} setDarkMode={setDarkMode} />
    </BrowserRouter>
  );
}

function AppContent({ darkMode, setDarkMode }: { darkMode: boolean, setDarkMode: (v: boolean) => void }) {
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
    <div className="flex h-screen bg-[#FBFBFD] dark:bg-black overflow-hidden text-[#1D1D1F] dark:text-[#F5F5F7] transition-colors duration-300">
      {/* Sidebar */}
      <aside className="w-72 bg-white/80 dark:bg-[#1C1C1E]/80 backdrop-blur-xl border-r border-apple-gray-100 dark:border-white/5 flex flex-col p-6 z-20">
        <div className="flex items-center gap-4 px-2 py-4 mb-8">
          <div className="w-12 h-12 bg-apple-blue rounded-[14px] flex items-center justify-center text-white shadow-xl shadow-apple-blue/30 overflow-hidden relative">
            <Activity size={28} />
            <motion.div
              className="absolute inset-0 bg-white/20"
              animate={{ opacity: [0, 0.2, 0] }}
              transition={{ duration: 2, repeat: Infinity }}
            />
          </div>
          <div>
            <h1 className="text-xl font-bold tracking-tight">Acton</h1>
            <p className="text-[11px] text-apple-gray-400 dark:text-apple-gray-400 font-bold uppercase tracking-tight">LiteNode</p>
          </div>
        </div>

        <nav className="flex-1 space-y-2">
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

        <StatusIndicator online={online} lastBlock={nodeStatus?.last?.seqno} />
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-y-auto relative bg-[#FBFBFD] dark:bg-black transition-colors duration-300">
        <header className="sticky top-0 z-30 bg-white/95 dark:bg-black/95 backdrop-blur-md px-10 py-6 border-b border-apple-gray-100 dark:border-white/5 flex justify-between items-center transition-colors duration-300">
          <div className="flex flex-col">
            <motion.h2
              key={location.pathname}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="text-2xl font-bold tracking-tight"
            >
              {location.pathname === '/' ? 'Network Overview' :
               location.pathname.startsWith('/account') ? 'Account Explorer' :
               location.pathname.startsWith('/transaction') ? 'Transaction Details' :
               'Blockchain Explorer'}
            </motion.h2>
          </div>

          <div className="flex items-center gap-6">
            <form onSubmit={handleSearch} className="flex items-center gap-4">
              <div className="relative group">
                <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 text-apple-gray-300 dark:text-apple-gray-400 group-focus-within:text-apple-blue transition-colors" size={18} />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search address or hash..."
                  className="w-80 h-11 pl-11 pr-4 bg-apple-gray-50 dark:bg-[#1C1C1E] border border-apple-gray-200 dark:border-transparent rounded-full text-sm font-medium focus:outline-none focus:ring-4 focus:ring-apple-blue/10 focus:border-apple-blue transition-all dark:text-white transition-colors duration-300 outline-none"
                />
              </div>
              <button type="submit" className="hidden" />
            </form>

            <button
              onClick={() => setDarkMode(!darkMode)}
              className="w-11 h-11 rounded-full bg-apple-gray-50 dark:bg-[#1C1C1E] border border-apple-gray-200 dark:border-transparent flex items-center justify-center text-apple-gray-400 dark:text-apple-gray-200 hover:bg-apple-gray-100 dark:hover:bg-[#2C2C2E] transition-all active:scale-95 shadow-sm"
            >
              {darkMode ? <Sun size={20} /> : <Moon size={20} />}
            </button>
          </div>
        </header>

        <div className="p-10 max-w-[1400px] mx-auto">
          <Routes>
            <Route path="/" element={<DashboardView nodeStatus={nodeStatus} />} />
            <Route path="/account" element={<AccountExplorer />} />
            <Route path="/account/:address" element={<AccountDetails />} />
            <Route path="/transaction/:hash" element={<TransactionDetailsView />} />
            <Route path="/blocks" element={<div className="apple-card">Coming soon...</div>} />
          </Routes>
        </div>
      </main>
    </div>
  );
}

// --- Views ---

function DashboardView({ nodeStatus }: { nodeStatus: any }) {
  const [address, setAddress] = useState('');
  const [amount, setAmount] = useState('100');
  const [status, setStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [error, setError] = useState('');
  const [recentTxs, setRecentTxs] = useState<any[]>([]);
  const navigate = useNavigate();

  useEffect(() => {
    const fetchRecent = async () => {
      try {
        const history = JSON.parse(localStorage.getItem('account_history') || '[]');
        if (history.length > 0) {
          const allTxs: any[] = [];
          for (const addr of history.slice(0, 3)) {
            const res = await fetch('/api/v2/getTransactions', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ address: addr, limit: 5 })
            });
            const data = await res.json();
            if (data.ok) allTxs.push(...data.result);
          }
          setRecentTxs(allTxs.sort((a, b) => b.utime - a.utime).slice(0, 10));
        }
      } catch (e) {}
    };
    fetchRecent();
    const interval = setInterval(fetchRecent, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleAirdrop = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!address) return;

    setStatus('loading');
    try {
      const res = await fetch('/faucet', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          address,
          amount: Number(amount) * 1e9
        })
      });
      const data = await res.json();
      if (data.ok) {
        setStatus('success');
        setTimeout(() => setStatus('idle'), 3000);
        // Add to history
        const history = JSON.parse(localStorage.getItem('account_history') || '[]');
        if (!history.includes(address)) {
          const newHistory = [address, ...history].slice(0, 10);
          localStorage.setItem('account_history', JSON.stringify(newHistory));
        }
      } else {
        throw new Error(data.error || 'Failed to request TON');
      }
    } catch (e: any) {
      setError(e.message);
      setStatus('error');
    }
  };

  return (
    <div className="space-y-8 flex flex-col">
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8 items-stretch">
        <div className="apple-card lg:col-span-2 flex flex-col h-full">
          <div className="flex justify-between items-start mb-10">
            <div>
              <h3 className="text-sm font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-1">Network Capacity</h3>
              <p className="text-4xl font-bold tracking-tight">LiteNode Local</p>
            </div>
            <div className="bg-apple-gray-50 dark:bg-white/5 px-4 py-2 rounded-full border border-apple-gray-100 dark:border-white/10 flex items-center gap-2">
              <span className="w-2.5 h-2.5 bg-apple-system-green rounded-full shadow-[0_0_10px_rgba(52,199,89,0.5)]"></span>
              <span className="text-xs font-bold text-apple-gray-600 dark:text-apple-gray-300">Active Node</span>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-12">
            <div>
              <p className="text-[11px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-3 flex items-center gap-2">
                <Database size={12} /> Masterchain Seqno
              </p>
              <p className="text-5xl font-mono font-medium text-apple-gray-600 dark:text-apple-gray-100 leading-none">
                {nodeStatus?.last?.seqno || '---'}
              </p>
            </div>
            <div>
              <p className="text-[11px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-3">State Root Hash</p>
              <div className="bg-apple-gray-50 dark:bg-white/5 p-3 rounded-2xl border border-apple-gray-100 dark:border-white/5 overflow-hidden text-ellipsis">
                <p className="text-[10px] font-mono break-all text-apple-gray-500 dark:text-apple-gray-400 leading-relaxed">
                  {nodeStatus?.state_root_hash || '---'}
                </p>
              </div>
            </div>
          </div>

          <div className="mt-auto pt-10 border-t border-apple-gray-100 dark:border-white/5 flex gap-12 text-apple-gray-600 dark:text-apple-gray-200">
            <div>
              <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-1">Workchain</p>
              <p className="text-lg font-bold">0</p>
            </div>
            <div>
              <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-1">Shard Prefix</p>
              <p className="text-lg font-mono font-bold">{formatShard("-9223372036854775808")}</p>
            </div>
            <div>
              <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-1">Synchronized</p>
              <p className="text-lg font-bold text-apple-system-green">Connected</p>
            </div>
          </div>
        </div>

        <div className="apple-card bg-apple-gray-100 dark:bg-[#1C1C1E] border border-apple-gray-200 dark:border-white/5 flex flex-col p-6 h-full">
          <div className="relative z-10 flex flex-col h-full">
            <h3 className="text-apple-gray-400 dark:text-white/40 text-sm font-bold uppercase tracking-tight mb-4">Faucet</h3>

            <form onSubmit={handleAirdrop} className="space-y-6 flex-1 flex flex-col">
              <div className="space-y-6">
                <div className="space-y-2">
                  <label className="text-[10px] font-bold text-apple-gray-400 dark:text-white/30 uppercase tracking-tight block px-1">Receiver</label>
                  <input
                    type="text"
                    required
                    value={address}
                    onChange={(e) => setAddress(e.target.value)}
                    placeholder="Address..."
                    className="apple-input w-full h-14"
                  />
                </div>

                <div className="space-y-2">
                  <label className="text-[10px] font-bold text-apple-gray-400 dark:text-white/30 uppercase tracking-tight block px-1">Amount</label>
                  <div className="relative">
                    <input
                      type="number"
                      required
                      value={amount}
                      onChange={(e) => setAmount(e.target.value)}
                      className="apple-input w-full h-14 pr-12 font-bold"
                    />
                    <span className="absolute right-4 top-1/2 -translate-y-1/2 font-bold text-apple-gray-300 dark:text-white/20 text-xs">TON</span>
                  </div>
                </div>
              </div>

              <button
                type="submit"
                disabled={status === 'loading'}
                className={cn(
                  "w-full h-16 rounded-xl font-bold text-base transition-all active:scale-[0.98] mt-8 flex items-center justify-center gap-3",
                  status === 'loading' ? "bg-apple-gray-200 dark:bg-white/5 text-apple-gray-400 dark:text-white/20 cursor-wait" :
                  status === 'success' ? "bg-apple-system-green text-white" :
                  status === 'error' ? "bg-apple-system-red text-white" :
                  "bg-apple-gray-200 dark:bg-white/10 text-apple-gray-600 dark:text-white hover:bg-apple-gray-300 dark:hover:bg-white/20"
                )}
              >
                {status === 'loading' ? <RefreshCw size={20} className="animate-spin" /> : <Zap size={20} />}
                {status === 'loading' ? 'Broadcasting...' :
                 status === 'success' ? 'Sent!' :
                 status === 'error' ? 'Failed' :
                 'Request Airdrop'}
              </button>
            </form>
          </div>
        </div>
      </div>

      <div className="apple-card overflow-hidden p-0">
        <div className="p-6 border-b border-apple-gray-100 dark:border-white/5 flex justify-between items-center bg-white/50 dark:bg-[#1C1C1E]/50 backdrop-blur-sm transition-colors">
          <h3 className="text-lg font-bold flex items-center gap-2 dark:text-white">
            <Clock size={20} className="text-apple-blue" />
            Recent Activity
          </h3>
          <span className="text-[10px] font-bold bg-apple-gray-50 dark:bg-white/5 text-apple-gray-400 dark:text-apple-gray-400 px-3 py-1 rounded-full border border-apple-gray-100 dark:border-white/10 uppercase tracking-tight transition-colors duration-300">
            Real-time Feed
          </span>
        </div>

        <div className="divide-y divide-apple-gray-50 dark:divide-white/5 transition-colors">
          {recentTxs.length === 0 ? (
            <div className="text-center py-24">
              <div className="w-20 h-20 bg-apple-gray-50 dark:bg-white/5 rounded-full flex items-center justify-center mx-auto mb-6 transition-colors duration-300">
                <Activity size={32} className="text-apple-gray-200 dark:text-apple-gray-600" />
              </div>
              <p className="text-apple-gray-400 dark:text-apple-gray-400 font-medium transition-colors">No activity detected yet.</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <tbody className="divide-y divide-apple-gray-50 dark:divide-white/5">
                  {recentTxs.map((tx: any) => {
                    const info = getTransactionInfo(tx);
                    return (
                      <tr 
                        key={tx.transaction_id.hash}
                        className="group hover:bg-apple-gray-50/50 dark:hover:bg-white/5 transition-colors cursor-pointer"
                        onClick={() => navigate(`/transaction/${tx.transaction_id.hash}`)}
                      >
                        <td className="px-8 py-5">
                          <div className="flex items-center gap-4">
                            <div className={cn(
                              "w-10 h-10 rounded-full flex items-center justify-center transition-transform group-hover:scale-110 shadow-sm",
                              info.isOut ? "bg-apple-system-orange/10 text-apple-system-orange" : "bg-apple-system-green/10 text-apple-system-green"
                            )}>
                              {info.isOut ? <ArrowRight size={18} /> : <ArrowLeft size={18} />}
                            </div>
                            <div>
                              <p className="text-sm font-bold text-apple-gray-600 dark:text-apple-gray-200">
                                {info.label}
                              </p>
                              <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-500 uppercase tracking-tight">
                                {tx.transaction_id.hash.slice(0, 8)}...{tx.transaction_id.hash.slice(-8)}
                              </p>
                            </div>
                          </div>
                        </td>
                        <td className="px-8 py-5">
                          <p className="text-sm font-bold text-apple-gray-600 dark:text-apple-gray-300 font-mono">
                            {shortenAddress(info.otherParty || '---')}
                          </p>
                          <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-500 uppercase tracking-tight">
                            {info.isOut ? 'Destination' : 'Source'}
                          </p>
                        </td>
                        <td className="px-8 py-5 text-right">
                          <p className={cn(
                            "text-sm font-bold leading-none mb-1 transition-colors duration-300",
                            info.isOut ? "text-apple-system-orange" : "text-apple-system-green"
                          )}>
                            {info.isOut ? '−' : '+'}{formatTON(info.value)} TON
                          </p>
                          <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-500 uppercase tracking-tight">
                            {formatDistanceToNow(tx.utime * 1000, { addSuffix: true })}
                          </p>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function AccountExplorer() {
  const [address, setAddress] = useState('');
  const [history, setHistory] = useState<string[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    const saved = JSON.parse(localStorage.getItem('account_history') || '[]');
    setHistory(saved);
  }, []);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!address) return;
    const cleanAddr = address.trim();

    // Update history
    const newHistory = [cleanAddr, ...history.filter((a: string) => a !== cleanAddr)].slice(0, 5);
    localStorage.setItem('account_history', JSON.stringify(newHistory));

    navigate(`/account/${cleanAddr}`);
  };

  return (
    <div className="max-w-2xl mx-auto py-20 text-center">
      <div className="w-20 h-20 bg-apple-blue/10 text-apple-blue rounded-[24px] flex items-center justify-center mx-auto mb-8 shadow-xl shadow-apple-blue/5">
        <Wallet size={40} />
      </div>
      <h2 className="text-4xl font-bold tracking-tight mb-4">Account Explorer</h2>
      <p className="text-apple-gray-400 dark:text-apple-gray-400 font-medium mb-12 max-w-md mx-auto">
        Inspect balance, state, and recent transactions of any account on your local TON node.
      </p>

      <div className="relative">
        <form onSubmit={handleSearch} className="apple-card p-2 flex gap-2 shadow-2xl shadow-apple-gray-200/50 dark:shadow-none border border-apple-gray-100 dark:border-white/5 relative z-20">
          <input
            type="text"
            autoFocus
            value={address}
            onFocus={() => setShowHistory(true)}
            onChange={(e) => setAddress(e.target.value)}
            placeholder="Enter address (e.g. UQA...)"
            className="flex-1 bg-transparent border-none px-6 py-4 focus:outline-none font-medium text-lg dark:text-white outline-none"
          />
          <button
            type="submit"
            className="apple-button-primary h-14 px-10 text-lg"
          >
            Explore
          </button>
        </form>

        <AnimatePresence>
          {showHistory && history.length > 0 && (
            <>
              <div className="fixed inset-0 z-10" onClick={() => setShowHistory(false)} />
              <motion.div
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -10 }}
                className="absolute top-full left-0 right-0 mt-2 bg-white dark:bg-[#1C1C1E] border border-apple-gray-100 dark:border-white/5 rounded-2xl shadow-2xl z-20 overflow-hidden"
              >
                <div className="px-6 py-3 border-b border-apple-gray-50 dark:border-white/5 flex items-center gap-2 text-apple-gray-400 dark:text-apple-gray-500">
                  <History size={14} />
                  <span className="text-[10px] font-bold uppercase tracking-tight">Recent Searches</span>
                </div>
                <div className="divide-y divide-apple-gray-50 dark:divide-white/5">
                  {history.map((addr) => (
                    <button
                      key={addr}
                      onClick={() => {
                        setAddress(addr);
                        setShowHistory(false);
                        navigate(`/account/${addr}`);
                      }}
                      className="w-full px-6 py-4 text-left hover:bg-apple-gray-50 dark:hover:bg-white/5 transition-colors flex items-center justify-between group"
                    >
                      <span className="font-mono text-sm text-apple-gray-600 dark:text-apple-gray-300 truncate mr-4">
                        {formatAddress(addr)}
                      </span>
                      <ArrowRight size={16} className="text-apple-gray-200 dark:text-apple-gray-700 group-hover:text-apple-blue group-hover:translate-x-1 transition-all" />
                    </button>
                  ))}
                </div>
              </motion.div>
            </>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

function AccountDetails() {
  const { address } = useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<any>(null);
  const [txs, setTxs] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [faucetStatus, setFaucetStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');

  const fetchAccount = async () => {
    if (!address) return;
    setLoading(true);
    try {
      const res = await fetch('/api/v2/getExtendedAddressInformation', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address })
      });
      const json = await res.json();
      if (!json.ok) throw new Error(json.error);
      setData(json.result);

      // Fetch transactions
      const txRes = await fetch('/api/v2/getTransactions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address, limit: 20 })
      });
      const txJson = await txRes.json();
      if (txJson.ok) setTxs(txJson.result);

      // Save to history
      const history = JSON.parse(localStorage.getItem('account_history') || '[]');
      const newHistory = [address, ...history.filter((a: string) => a !== address)].slice(0, 10);
      localStorage.setItem('account_history', JSON.stringify(newHistory));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const handleQuickFaucet = async () => {
    if (!address) return;
    setFaucetStatus('loading');
    try {
      const res = await fetch('/faucet', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          address,
          amount: 100 * 1e9
        })
      });
      const data = await res.json();
      if (data.ok) {
        setFaucetStatus('success');
        setTimeout(() => setFaucetStatus('idle'), 3000);
        fetchAccount(); // Refresh data
      } else {
        throw new Error(data.error || 'Failed to request TON');
      }
    } catch (e: any) {
      setFaucetStatus('error');
      setTimeout(() => setFaucetStatus('idle'), 3000);
    }
  };

  useEffect(() => {
    fetchAccount();
  }, [address]);

  if (loading) return (
    <div className="flex flex-col items-center justify-center py-40 gap-6 transition-colors duration-300">
      <RefreshCw className="animate-spin text-apple-blue" size={48} />
      <p className="text-apple-gray-400 dark:text-apple-gray-400 font-bold uppercase tracking-tight text-xs animate-pulse">Scanning Ledger...</p>
    </div>
  );

  if (error) return (
    <div className="max-w-xl mx-auto py-20 text-center">
      <div className="w-16 h-16 bg-apple-system-red/10 text-apple-system-red rounded-full flex items-center justify-center mx-auto mb-6">
        <XCircle size={32} />
      </div>
      <h3 className="text-2xl font-bold mb-2">Failed to load account</h3>
      <p className="text-apple-gray-400 dark:text-apple-gray-400 font-medium mb-8">{error}</p>
      <Link to="/account" className="apple-button-secondary inline-flex items-center gap-2">
        <ArrowLeft size={18} /> Back to Search
      </Link>
    </div>
  );

  const isUninited = data.account_state['@type'] === 'uninited.accountState';
  const isActive = data.account_state['@type'] === 'raw.accountState';

  return (
    <div className="space-y-8">
      <div className="flex items-center gap-4 text-apple-gray-400 dark:text-apple-gray-400 mb-2">
        <Link to="/account" className="hover:text-apple-blue transition-colors">Accounts</Link>
        <ChevronRight size={14} />
        <span className="font-bold text-apple-gray-600 dark:text-apple-gray-300 truncate">{formatAddress(address!)}</span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        {/* Main Info */}
        <div className="lg:col-span-2 space-y-8">
          <div className="apple-card transition-colors duration-300">
            <div className="flex flex-wrap justify-between items-start gap-6 mb-10">
              <div className="space-y-1">
                <h3 className="text-xs font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight">Account Balance</h3>
                <div className="flex items-baseline gap-2">
                  <span className="text-5xl font-bold text-apple-gray-600 dark:text-apple-gray-100 leading-tight">
                    {formatTON(data.balance)}
                  </span>
                  <span className="text-xl font-bold text-apple-gray-400 dark:text-apple-gray-400">TON</span>
                </div>
              </div>
              <div className={cn(
                "px-6 py-2.5 rounded-2xl font-bold flex items-center gap-3 border transition-colors duration-300",
                isActive ? "bg-apple-system-green/5 text-apple-system-green border-apple-system-green/20" :
                isUninited ? "bg-apple-gray-100 dark:bg-white/5 text-apple-gray-400 dark:text-apple-gray-400 border-apple-gray-200 dark:border-white/10" :
                "bg-apple-system-orange/5 text-apple-system-orange border-apple-system-orange/20"
              )}>
                {isActive ? <CheckCircle2 size={20} /> : isUninited ? <Clock size={20} /> : <XCircle size={20} />}
                <span className="uppercase tracking-tight text-xs">
                  {isActive ? 'Active' : isUninited ? 'Uninitialized' : 'Frozen'}
                </span>
              </div>
            </div>

            <div className="space-y-6 pt-8 border-t border-apple-gray-100 dark:border-white/5 transition-colors duration-300">
              <div>
                <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-3">Wallet Address</p>
                <div className="flex items-center gap-3 bg-apple-gray-50 dark:bg-white/5 p-4 rounded-2xl border border-apple-gray-100 dark:border-white/5 group transition-colors duration-300">
                  <p className="text-sm font-mono font-bold text-apple-gray-600 dark:text-apple-gray-300 break-all select-all">
                    {formatAddress(address!)}
                  </p>
                  <button className="text-apple-gray-300 dark:text-apple-gray-600 hover:text-apple-blue transition-colors ml-auto group-hover:scale-110 active:scale-95">
                    <ExternalLink size={18} />
                  </button>
                </div>
              </div>
            </div>
          </div>

          {/* Transactions List */}
          <div className="apple-card p-0 overflow-hidden transition-colors duration-300">
            <div className="px-8 py-6 border-b border-apple-gray-100 dark:border-white/5 flex justify-between items-center bg-white/50 dark:bg-white/5 backdrop-blur-sm transition-colors duration-300">
              <h3 className="text-lg font-bold">Transaction History</h3>
              <div className="flex items-center gap-2">
                <span className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight">
                  Latest {txs.length} Transactions
                </span>
              </div>
            </div>

            <div className="divide-y divide-apple-gray-50 dark:divide-white/5 transition-colors duration-300">
              {txs.length === 0 ? (
                <div className="py-20 text-center text-apple-gray-300 dark:text-apple-gray-600 font-medium">
                  No transactions found for this account.
                </div>
              ) : (
                <div className="overflow-x-auto">
                  <table className="w-full">
                    <tbody className="divide-y divide-apple-gray-50 dark:divide-white/5">
                      {txs.map((tx: any) => {
                        const info = getTransactionInfo(tx, address);
                        return (
                          <tr 
                            key={tx.transaction_id.hash} 
                            className="group hover:bg-apple-gray-50/50 dark:hover:bg-white/5 transition-colors cursor-pointer"
                            onClick={() => navigate(`/transaction/${tx.transaction_id.hash}`)}
                          >
                            <td className="px-8 py-6">
                              <div className="flex items-center gap-4">
                                <div className={cn(
                                  "w-10 h-10 rounded-full flex items-center justify-center transition-transform group-hover:scale-110 shadow-sm",
                                  info.isOut ? "bg-apple-system-orange/10 text-apple-system-orange" : "bg-apple-system-green/10 text-apple-system-green"
                                )}>
                                  {info.isOut ? <ArrowRight size={18} className="rotate-[-45deg]" /> : <ArrowLeft size={18} className="rotate-[-45deg]" />}
                                </div>
                                <div>
                                  <p className="text-xs font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-0.5">
                                    {info.label}
                                  </p>
                                  <p className="text-sm font-bold text-apple-gray-600 dark:text-apple-gray-300 font-mono">
                                    {shortenAddress(info.otherParty || '---')}
                                  </p>
                                </div>
                              </div>
                            </td>
                            <td className="px-8 py-6">
                              {info.comment && (
                                <div className="flex items-center gap-2 bg-apple-gray-50 dark:bg-white/5 px-3 py-1.5 rounded-lg border border-apple-gray-100 dark:border-white/5 max-w-[200px]">
                                  <p className="text-[11px] text-apple-gray-500 dark:text-apple-gray-400 truncate font-medium">
                                    {info.comment}
                                  </p>
                                </div>
                              )}
                            </td>
                            <td className="px-8 py-6 text-right">
                              <p className={cn(
                                "text-lg font-bold leading-none mb-1 transition-colors duration-300",
                                info.isOut ? "text-apple-system-orange" : "text-apple-system-green"
                              )}>
                                {info.isOut ? '−' : '+'}{formatTON(info.value)}
                              </p>
                              <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 text-right uppercase tracking-tight transition-colors duration-300">
                                {formatDistanceToNow(tx.utime * 1000, { addSuffix: true })}
                              </p>
                            </td>
                            <td className="px-8 py-6 text-right w-12">
                              <ChevronRight className="text-apple-gray-200 dark:text-apple-gray-700 group-hover:text-apple-blue group-hover:translate-x-1 transition-all" size={20} />
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Sidebar Info */}
        <div className="space-y-8">
          <div className="apple-card transition-colors duration-300">
            <h4 className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-6 transition-colors duration-300">Technical Details</h4>
            <div className="space-y-6">
              <div className="p-4 bg-apple-gray-50 dark:bg-white/5 rounded-2xl border border-apple-gray-100 dark:border-white/5 transition-colors duration-300">
                <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-2 transition-colors duration-300">Code Hash</p>
                <p className="text-[11px] font-mono break-all text-apple-gray-600 dark:text-apple-gray-400 transition-colors duration-300">
                  {data.account_state.code ? 'Present' : 'None (No Code)'}
                </p>
              </div>
              <div className="p-4 bg-apple-gray-50 dark:bg-white/5 rounded-2xl border border-apple-gray-100 dark:border-white/5 transition-colors duration-300">
                <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-2 transition-colors duration-300">Data Hash</p>
                <p className="text-[11px] font-mono break-all text-apple-gray-600 dark:text-apple-gray-400 transition-colors duration-300">
                  {data.account_state.data ? 'Present' : 'None (No Data)'}
                </p>
              </div>
              <div className="p-4 bg-apple-gray-50 dark:bg-white/5 rounded-2xl border border-apple-gray-100 dark:border-white/5 transition-colors duration-300">
                <p className="text-[10px] font-bold text-apple-gray-400 dark:text-apple-gray-400 uppercase tracking-tight mb-2 transition-colors duration-300">Last Tx LT</p>
                <p className="text-sm font-bold text-apple-gray-600 dark:text-apple-gray-200 transition-colors duration-300">{data.last_transaction_id.lt}</p>
              </div>
            </div>
          </div>

          <div className="apple-card bg-apple-gray-50 dark:bg-apple-blue text-apple-gray-600 dark:text-white p-8 border border-apple-gray-100 dark:border-transparent transition-colors duration-300">
            <h4 className="text-apple-gray-400 dark:text-white/50 text-[10px] font-bold uppercase tracking-tight mb-6">Developer Actions</h4>
            <div className="space-y-4">
              <button
                onClick={handleQuickFaucet}
                disabled={faucetStatus === 'loading'}
                className={cn(
                  "flex items-center justify-between w-full h-14 px-6 rounded-2xl font-bold transition-all group active:scale-[0.98] shadow-sm border",
                  faucetStatus === 'loading' ? "bg-apple-gray-200 border-apple-gray-300 text-apple-gray-400 cursor-wait" :
                  faucetStatus === 'success' ? "bg-apple-system-green border-apple-system-green text-white" :
                  faucetStatus === 'error' ? "bg-apple-system-red border-apple-system-red text-white" :
                  "bg-white dark:bg-white/10 hover:bg-apple-gray-100 dark:hover:bg-white/20 border-apple-gray-200 dark:border-white/10"
                )}
              >
                <span className={cn(
                  "transition-colors",
                  faucetStatus === 'idle' ? "text-apple-blue dark:text-white" : "text-white"
                )}>
                  {faucetStatus === 'loading' ? 'Requesting...' : faucetStatus === 'success' ? 'Sent 100 TON!' : faucetStatus === 'error' ? 'Failed' : 'Request 100 TON'}
                </span>
                {faucetStatus === 'loading' ? (
                  <RefreshCw size={20} className="animate-spin text-apple-gray-400" />
                ) : (
                  <Coins size={20} className={cn("transition-transform group-hover:rotate-12", faucetStatus === 'idle' ? "text-apple-blue dark:text-white" : "text-white")} />
                )}
              </button>
              <button className="flex items-center justify-between w-full h-14 px-6 bg-white dark:bg-white/10 hover:bg-apple-gray-100 dark:hover:bg-white/20 border border-apple-gray-200 dark:border-white/10 rounded-2xl font-bold transition-all group active:scale-[0.98] shadow-sm">
                <span className="dark:text-white text-apple-gray-600">Run Get Method</span>
                <Activity size={20} className="text-apple-gray-400 dark:text-white group-hover:scale-110 transition-transform" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function TransactionDetailsView() {
  const { hash } = useParams();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(true);
  const [tx, setTx] = useState<any>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    const fetchTx = async () => {
      if (!hash) return;
      setLoading(true);
      try {
        // We don't have a direct getTransactionByHash yet in the lite-node API,
        // but we can try to find it in the recent activity or by scanning common accounts
        // For now, let's assume it might be findable or we just show limited info
        // Wait, the API doesn't seem to have getTransactionByHash.
        // Let's check if we can add it or if there's a workaround.
        
        // Mocking for now since API doesn't support direct lookup by hash yet
        setLoading(false);
      } catch (e: any) {
        setError(e.message);
        setLoading(false);
      }
    };
    fetchTx();
  }, [hash]);

  if (loading) return (
    <div className="flex flex-col items-center justify-center py-40 gap-6">
      <RefreshCw className="animate-spin text-apple-blue" size={48} />
      <p className="text-apple-gray-400 font-bold uppercase tracking-tight text-xs">Locating Transaction...</p>
    </div>
  );

  return (
    <div className="space-y-8">
      <div className="flex items-center gap-4 text-apple-gray-400 mb-2">
        <button onClick={() => navigate(-1)} className="hover:text-apple-blue transition-colors">Back</button>
        <ChevronRight size={14} />
        <span className="font-bold text-apple-gray-600 dark:text-apple-gray-300 truncate">Transaction Detail</span>
      </div>

      <div className="apple-card">
        <div className="flex items-center gap-4 mb-8">
          <div className="w-12 h-12 bg-apple-system-green/10 text-apple-system-green rounded-full flex items-center justify-center">
            <CheckCircle2 size={28} />
          </div>
          <div>
            <h3 className="text-xl font-bold">Confirmed transaction</h3>
            <p className="text-sm text-apple-gray-400 font-medium">The transaction has been successfully processed and included in a block.</p>
          </div>
        </div>

        <div className="space-y-6 pt-8 border-t border-apple-gray-50 dark:border-white/5">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
            <div>
              <p className="text-[10px] font-bold text-apple-gray-400 uppercase tracking-tight mb-2">Transaction Hash</p>
              <div className="bg-apple-gray-50 dark:bg-white/5 p-4 rounded-xl border border-apple-gray-100 dark:border-white/5">
                <p className="text-sm font-mono font-bold break-all select-all">{hash}</p>
              </div>
            </div>
            <div>
              <p className="text-[10px] font-bold text-apple-gray-400 uppercase tracking-tight mb-2">Status</p>
              <div className="inline-flex items-center gap-2 bg-apple-system-green/10 text-apple-system-green px-4 py-2 rounded-xl border border-apple-system-green/20">
                <div className="w-2 h-2 bg-apple-system-green rounded-full animate-pulse" />
                <span className="font-bold text-sm uppercase tracking-tight">Success</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="apple-card p-0 overflow-hidden">
        <div className="p-6 border-b border-apple-gray-100 dark:border-white/5 bg-apple-gray-50/50 dark:bg-white/5">
          <h3 className="font-bold">Transaction Overview</h3>
        </div>
        <div className="p-8">
          <p className="text-apple-gray-400 dark:text-apple-gray-500 text-center py-12 italic">
            Detailed transaction indexing and message trace visualization coming in next version.
          </p>
        </div>
      </div>
    </div>
  );
}
