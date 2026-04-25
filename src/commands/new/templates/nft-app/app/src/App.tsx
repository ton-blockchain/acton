import { useEffect, useState } from 'react';
import {
  TonConnectButton,
  THEME,
  useTonAddress,
  useTonConnectUI,
} from '@tonconnect/ui-react';
import { Layers, Folder, Sparkles, Sun, Moon } from 'lucide-react';

import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { NetworkDropdown } from './components/NetworkDropdown';
import { DeployCollection } from './components/DeployCollection';
import { DeployItem } from './components/DeployItem';
import { DeployBatch } from './components/DeployBatch';
import { Manage } from './components/Manage';
import { useCollectionsStore } from './lib/collections';
import { useRouter } from './lib/router';
import { formatAddressForNetwork } from './lib/ton';
import { IconTonDiamond } from './components/TonDiamond';

type Subtab = 'collection' | 'item' | 'batch';

function useTheme() {
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    const stored = localStorage.getItem('nft-minter:theme');
    return stored === 'light' ? 'light' : 'dark';
  });
  const [tonConnectUI] = useTonConnectUI();

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('nft-minter:theme', theme);
    tonConnectUI.uiOptions = {
      uiPreferences: { theme: theme === 'light' ? THEME.LIGHT : THEME.DARK },
    };
  }, [theme, tonConnectUI]);

  return { theme, setTheme };
}

function readPersistedSubtab(): Subtab {
  try {
    const raw = localStorage.getItem('nft-minter:view');
    if (!raw) return 'item';
    const parsed = JSON.parse(raw) as { subtab?: Subtab };
    return parsed.subtab ?? 'item';
  } catch {
    return 'item';
  }
}

export default function App() {
  const { page, network, go, setTestnet } = useRouter();
  const walletAddress = useTonAddress();
  const { theme, setTheme } = useTheme();
  const { list, update } = useCollectionsStore(network);

  const [subtab, setSubtab] = useState<Subtab>(readPersistedSubtab);
  const [selectedCollectionId, setSelectedCollectionId] = useState<
    string | null
  >(null);

  useEffect(() => {
    if (selectedCollectionId == null && list.length > 0) {
      setSelectedCollectionId(list[0].id);
    }
    if (
      selectedCollectionId != null &&
      !list.find((c) => c.id === selectedCollectionId)
    ) {
      setSelectedCollectionId(list[0]?.id ?? null);
    }
  }, [list, selectedCollectionId]);

  useEffect(() => {
    localStorage.setItem(
      'nft-minter:view',
      JSON.stringify({ subtab, selectedCollectionId }),
    );
  }, [subtab, selectedCollectionId]);

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
        <div className="flex items-center gap-6 max-sm:gap-2.5 max-sm:w-full max-sm:justify-between">
          <div className="flex items-center gap-2.5 text-[17px] font-bold max-sm:text-[15px]">
            <div className="w-8 h-8 rounded-[9px] bg-[#0098EA] flex items-center justify-center max-sm:w-7 max-sm:h-7 max-sm:rounded-[7px]">
              <IconTonDiamond size={16} />
            </div>
            NFT Minter
          </div>
          <div className="flex gap-0.5 p-[3px] h-10 rounded-full items-center bg-secondary max-sm:h-9">
            <Button
              variant="ghost"
              className={cn(
                'rounded-full px-4 h-[34px] text-[15px] font-bold max-sm:h-[30px] max-sm:px-3.5 max-sm:text-[13px] hover:bg-transparent',
                page === 'create'
                  ? 'bg-[#0098EA] text-white hover:bg-[#0098EA] hover:text-white'
                  : 'text-muted-foreground hover:text-foreground dark:text-white/60 dark:hover:text-white',
              )}
              onClick={() => go('create')}
            >
              Create
            </Button>
            <Button
              variant="ghost"
              className={cn(
                'rounded-full px-4 h-[34px] text-[15px] font-bold max-sm:h-[30px] max-sm:px-3.5 max-sm:text-[13px] hover:bg-transparent',
                page === 'manage'
                  ? 'bg-[#0098EA] text-white hover:bg-[#0098EA] hover:text-white'
                  : 'text-muted-foreground hover:text-foreground dark:text-white/60 dark:hover:text-white',
              )}
              onClick={() => go('manage')}
            >
              Manage
            </Button>
          </div>
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
      <div className="py-8 px-6 max-w-[1200px] mx-auto w-full pb-20">
        {page === 'create' ? (
          <>
            <div className="flex items-end justify-between mb-4.5 gap-4">
              <div>
                <h1 className="text-[22px] font-semibold tracking-tight m-0 mb-0.5">
                  Create
                </h1>
                <p className="text-muted-foreground m-0 text-[13px]">
                  Deploy a new collection, a single item, or a batch of items.
                </p>
              </div>
              <Tabs
                value={subtab}
                onValueChange={(v) => setSubtab(v as Subtab)}
              >
                <TabsList className="h-auto">
                  <TabsTrigger value="collection">
                    <Folder className="size-3.5" /> Collection
                  </TabsTrigger>
                  <TabsTrigger value="item">
                    <Sparkles className="size-3.5" /> Item
                  </TabsTrigger>
                  <TabsTrigger value="batch">
                    <Layers className="size-3.5" /> Batch
                  </TabsTrigger>
                </TabsList>
              </Tabs>
            </div>

            {subtab === 'collection' ? (
              <DeployCollection
                network={network}
                userWallet={userWallet}
                onDeployed={(id) => {
                  setSelectedCollectionId(id);
                  go('manage');
                }}
              />
            ) : null}

            {subtab === 'item' ? (
              <DeployItem
                network={network}
                collections={list}
                selectedCollectionId={selectedCollectionId}
                onSelectCollection={setSelectedCollectionId}
                userWallet={userWallet}
                onMinted={(collectionId) => {
                  const c = list.find((x) => x.id === collectionId);
                  if (c) update(c.id, { nextItemIndex: c.nextItemIndex + 1 });
                }}
              />
            ) : null}

            {subtab === 'batch' ? (
              <DeployBatch
                network={network}
                collections={list}
                selectedCollectionId={selectedCollectionId}
                onSelectCollection={setSelectedCollectionId}
                userWallet={userWallet}
                onMinted={(collectionId, count) => {
                  const c = list.find((x) => x.id === collectionId);
                  if (c)
                    update(c.id, { nextItemIndex: c.nextItemIndex + count });
                }}
              />
            ) : null}
          </>
        ) : (
          <>
            <div className="flex items-end justify-between mb-4.5 gap-4">
              <div>
                <h1 className="text-[22px] font-semibold tracking-tight m-0 mb-0.5">
                  Manage
                </h1>
                <p className="text-muted-foreground m-0 text-[13px]">
                  Administer collections and items you own.
                </p>
              </div>
            </div>
            <Manage
              network={network}
              collections={list}
              selectedCollectionId={selectedCollectionId}
              onSelectCollection={setSelectedCollectionId}
              onRefreshCollection={update}
            />
          </>
        )}
      </div>
    </div>
  );
}
