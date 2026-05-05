import { useEffect, useState } from 'react';
import {
  TonConnectButton,
  THEME,
  useTonAddress,
  useTonConnectUI,
} from '@tonconnect/ui-react';
import { Sun, Moon } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { NetworkDropdown } from './components/NetworkDropdown';
import { useRouter } from './lib/router';
import { formatAddressForNetwork } from './lib/ton';
import { IconTonDiamond } from './components/TonDiamond';

function useTheme() {
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    const stored = localStorage.getItem('ton-dapp:theme');
    return stored === 'light' ? 'light' : 'dark';
  });
  const [tonConnectUI] = useTonConnectUI();

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('ton-dapp:theme', theme);
    tonConnectUI.uiOptions = {
      uiPreferences: { theme: theme === 'light' ? THEME.LIGHT : THEME.DARK },
    };
  }, [theme, tonConnectUI]);

  return { theme, setTheme };
}

export default function App() {
  const { network, setTestnet } = useRouter();
  const walletAddress = useTonAddress();
  const { theme, setTheme } = useTheme();

  const userWallet = walletAddress
    ? (() => {
        try {
          return formatAddressForNetwork(walletAddress, network);
        } catch {
          return walletAddress;
        }
      })()
    : '';

  return (
    <div className="min-h-screen flex flex-col">
      {/* ─── Topbar ─── */}
      <header className="flex items-center justify-between px-7 h-[60px] border-b sticky top-0 z-50 bg-background max-sm:px-4 max-sm:h-auto max-sm:flex-wrap max-sm:gap-2.5 max-sm:py-3">
        <div className="flex items-center gap-2.5 text-[17px] font-bold max-sm:text-[15px]">
          <div className="w-8 h-8 rounded-[9px] bg-[#0098EA] flex items-center justify-center max-sm:w-7 max-sm:h-7 max-sm:rounded-[7px]">
            <IconTonDiamond size={16} />
          </div>
          TON dApp
        </div>
        <div className="flex items-center gap-2.5">
          <Button
            variant="ghost"
            size="icon"
            className="rounded-full size-10 bg-secondary max-sm:size-9"
            title="Toggle theme"
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
          >
            {theme === 'dark' ? (
              <Sun className="size-[18px]" />
            ) : (
              <Moon className="size-[18px]" />
            )}
          </Button>
          <NetworkDropdown network={network} setTestnet={setTestnet} />
          <TonConnectButton />
        </div>
      </header>

      {/* ─── Main content ─── */}
      <main className="flex-1 py-8 px-6 max-w-[1200px] mx-auto w-full">
        <div className="flex flex-col items-center justify-center gap-4 py-20 text-center">
          <div className="w-16 h-16 rounded-2xl bg-[#0098EA] flex items-center justify-center">
            <IconTonDiamond size={32} />
          </div>
          <h1 className="text-[22px] font-semibold tracking-tight">
            TON dApp Template
          </h1>
          <p className="text-muted-foreground text-[15px] max-w-md">
            Connect your wallet and start building.
            {userWallet && (
              <span className="block mt-2 font-mono text-[13px] text-foreground/70 break-all">
                {userWallet}
              </span>
            )}
          </p>
          <p className="text-muted-foreground text-[13px]">
            Network:{' '}
            <span
              className={cn(
                'font-semibold',
                network === 'mainnet' ? 'text-success' : 'text-warning',
              )}
            >
              {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
            </span>
          </p>
        </div>
      </main>
    </div>
  );
}
