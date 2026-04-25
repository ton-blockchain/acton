import { useState, useEffect, createContext, useContext } from 'react';
import { TonConnectButton, useTonConnectUI } from '@tonconnect/ui-react';
import { THEME } from '@tonconnect/ui-react';
import { Gem, Sun, Moon, ChevronDown, Check, Circle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from '@/components/ui/dropdown-menu';
import { DeployPage } from './pages/DeployPage';
import { ManagePage } from './pages/ManagePage';
import { useRouter } from './lib/router';
import './styles.css';

type Theme = 'dark' | 'light';

const ThemeContext = createContext<{ theme: Theme; toggle: () => void }>({
  theme: 'dark',
  toggle: () => {},
});

export const useTheme = () => useContext(ThemeContext);

export function App() {
  const { page, network, address, go, setTestnet, setAddress } = useRouter();
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = localStorage.getItem('jm-theme');
    return saved === 'light' ? 'light' : 'dark';
  });
  const [tonConnectUI] = useTonConnectUI();

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('jm-theme', theme);
    tonConnectUI.uiOptions = {
      uiPreferences: {
        theme: theme === 'light' ? THEME.LIGHT : THEME.DARK,
      },
    };
  }, [theme, tonConnectUI]);

  const toggle = () => setTheme((t) => (t === 'dark' ? 'light' : 'dark'));

  return (
    <ThemeContext.Provider value={{ theme, toggle }}>
      <div className="min-h-full flex flex-col">
        <header
          className="flex items-center justify-between px-7 h-[60px] border-b sticky top-0 z-50 max-sm:px-4 max-sm:h-auto max-sm:flex-wrap max-sm:gap-2.5 max-sm:py-3"
          style={{
            background: theme === 'light' ? '#fff' : '#08080A',
            borderBottomColor:
              theme === 'light' ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.06)',
          }}
        >
          <div className="flex items-center gap-6 max-sm:gap-2.5 max-sm:w-full max-sm:justify-between">
            <div className="flex items-center gap-2.5 text-[17px] font-bold max-sm:text-[15px]">
              <div className="w-8 h-8 bg-[#0098EA] rounded-[9px] flex items-center justify-center text-white max-sm:w-7 max-sm:h-7 max-sm:rounded-[7px]">
                <Gem className="size-4 max-sm:size-3.5" />
              </div>
              Jetton Minter
            </div>
            <nav
              className="flex gap-0.5 p-[3px] h-10 rounded-full items-center max-sm:h-9"
              style={{ background: theme === 'light' ? '#F0F1F3' : '#19191B' }}
            >
              <Button
                variant="ghost"
                size="sm"
                className={`rounded-full px-4 h-[34px] text-[15px] font-bold max-sm:h-[30px] max-sm:px-3.5 max-sm:text-[13px] hover:bg-transparent ${
                  page === 'create'
                    ? 'bg-[#0098EA] text-white hover:bg-[#0098EA] hover:text-white'
                    : theme === 'light'
                      ? 'text-muted-foreground hover:text-foreground'
                      : 'text-white/60 hover:text-white'
                }`}
                onClick={() => go('create')}
              >
                Create
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className={`rounded-full px-4 h-[34px] text-[15px] font-bold max-sm:h-[30px] max-sm:px-3.5 max-sm:text-[13px] hover:bg-transparent ${
                  page === 'manage'
                    ? 'bg-[#0098EA] text-white hover:bg-[#0098EA] hover:text-white'
                    : theme === 'light'
                      ? 'text-muted-foreground hover:text-foreground'
                      : 'text-white/60 hover:text-white'
                }`}
                onClick={() => go('manage')}
              >
                Manage
              </Button>
            </nav>
          </div>
          <div className="flex items-center gap-2.5">
            <Button
              variant="ghost"
              size="icon"
              className="rounded-full size-10 max-sm:size-9"
              style={{
                background: theme === 'light' ? '#F0F1F3' : '#19191B',
                color: theme === 'light' ? 'var(--foreground)' : '#fff',
              }}
              onClick={toggle}
              title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} theme`}
            >
              {theme === 'dark' ? (
                <Sun className="size-[18px]" />
              ) : (
                <Moon className="size-[18px]" />
              )}
            </Button>
            <NetworkDropdown network={network} setTestnet={setTestnet} theme={theme} />
            <TonConnectButton />
          </div>
        </header>

        <main className="flex-1 max-w-[960px] w-full mx-auto px-6 pt-9 pb-15 max-sm:px-4 max-sm:pt-6 max-sm:pb-12">
          {page === 'create' ? (
            <DeployPage network={network} />
          ) : (
            <ManagePage
              network={network}
              initialAddress={address}
              onAddressChange={setAddress}
            />
          )}
        </main>
      </div>
    </ThemeContext.Provider>
  );
}

function NetworkDropdown({
  network,
  setTestnet,
  theme,
}: {
  network: 'mainnet' | 'testnet';
  setTestnet: (testnet: boolean) => void;
  theme: Theme;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          className="rounded-full h-10 px-3 gap-1.5 text-[15px] font-bold max-sm:h-9 max-sm:text-sm max-sm:px-2.5"
          style={{
            background: theme === 'light' ? '#F0F1F3' : '#19191B',
            color: theme === 'light' ? 'var(--foreground)' : '#fff',
          }}
        >
          <Circle
            className="size-2 fill-current"
            style={{ color: network === 'testnet' ? 'var(--warning)' : 'var(--success)' }}
          />
          {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
          <ChevronDown className="size-3 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-[180px] rounded-xl p-2">
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setTestnet(false)}
        >
          <Circle className="size-2 fill-current" style={{ color: 'var(--success)' }} />
          Mainnet
          {network === 'mainnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setTestnet(true)}
        >
          <Circle className="size-2 fill-current" style={{ color: 'var(--warning)' }} />
          Testnet
          {network === 'testnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

