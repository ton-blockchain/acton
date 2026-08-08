import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import {
  TonConnectButton,
  THEME,
  useTonAddress,
  useTonConnectUI,
  useTonWallet,
} from '@tonconnect/ui-react';
import {
  AlertTriangle,
  Check,
  ChevronDown,
  Circle,
  ExternalLink,
  Moon,
  PlugZap,
  RefreshCcw,
  Search,
  Sun,
  WalletCards,
  X,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { cn } from '@/lib/utils';
import {
  MissingGetMethodError,
  EXTENSION_EXTERNAL_ACTION_LABELS,
  type ExtensionExternalAction,
  type ExtensionInspection,
  type WalletInspection,
  inspectWallet,
  sendExtensionExternalMessage,
} from './lib/w5';
import {
  formatAddressForNetwork,
  formatGramAmount,
  getErrorMessage,
  normalizeAddress,
  sameAddress,
  setTonNetworkMode,
  TON_CHAIN,
  TON_NETWORK_LABEL,
  TON_NETWORK_MODE,
  type TonNetworkMode,
} from './lib/ton';

type PendingAction = 'inspect' | null;
type Theme = 'dark' | 'light';

interface PendingExternalAction {
  rawAddress: string;
  action: ExtensionExternalAction;
}

interface InspectionState {
  status: 'idle' | 'loading' | 'ready' | 'empty' | 'error';
  data: WalletInspection | null;
  message: string;
  fetchedAt: string | null;
}

interface PopupState {
  id: number;
  title: string;
  message: string;
  tone: 'warning' | 'danger' | 'success';
}

const initialInspectionState: InspectionState = {
  status: 'idle',
  data: null,
  message: 'Enter a Wallet V5 address to read installed extensions.',
  fetchedAt: null,
};

function getAddressPreview(value: string): {
  value: string | null;
  error: string | null;
} {
  const trimmed = value.trim();

  if (!trimmed) {
    return {
      value: null,
      error: null,
    };
  }

  try {
    return {
      value: normalizeAddress(trimmed),
      error: null,
    };
  } catch (error) {
    return {
      value: null,
      error: getErrorMessage(error),
    };
  }
}

function formatUnixTimestamp(value: bigint | null): string {
  if (value === null) {
    return 'Not recorded';
  }

  const seconds = Number(value);
  if (!Number.isFinite(seconds)) {
    return `${value.toString()} sec`;
  }

  return new Date(seconds * 1000).toLocaleString();
}

function formatInterval(value: bigint): string {
  const seconds = Number(value);

  if (!Number.isFinite(seconds)) {
    return `${value.toString()} sec`;
  }

  if (seconds < 60) {
    return `${seconds} sec`;
  }

  if (seconds < 3600) {
    return `${(seconds / 60).toFixed(seconds % 60 === 0 ? 0 : 1)} min`;
  }

  if (seconds < 86400) {
    return `${(seconds / 3600).toFixed(seconds % 3600 === 0 ? 0 : 1)} hr`;
  }

  return `${(seconds / 86400).toFixed(seconds % 86400 === 0 ? 0 : 1)} days`;
}

function shortAddress(value: string): string {
  return value.length <= 16
    ? value
    : `${value.slice(0, 8)}...${value.slice(-8)}`;
}

function Field({
  label,
  hint,
  error,
  children,
}: {
  label: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label className="text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground">
        {label}
      </Label>
      {children}
      {error ? (
        <div className="text-[11.5px] text-destructive">{error}</div>
      ) : hint ? (
        <div className="text-[11.5px] text-muted-foreground">{hint}</div>
      ) : null}
    </div>
  );
}

function Metric({
  label,
  value,
  mono = false,
}: {
  label: ReactNode;
  value: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-dashed border-border py-2.5 last:border-b-0">
      <span className="text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground">
        {label}
      </span>
      <span
        className={cn(
          'max-w-[62%] overflow-hidden text-ellipsis whitespace-nowrap text-right text-sm text-foreground',
          mono && 'font-mono text-xs',
        )}
        title={typeof value === 'string' ? value : undefined}
      >
        {value}
      </span>
    </div>
  );
}

function NetworkDropdown({ theme }: { theme: Theme }) {
  const options: { mode: TonNetworkMode; label: string }[] = [
    { mode: 'mainnet', label: 'Mainnet' },
    { mode: 'testnet', label: 'Testnet' },
  ];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          className="h-10 rounded-full px-3 text-[15px] font-bold gap-1.5 max-sm:h-9 max-sm:px-2.5 max-sm:text-sm"
          style={{
            background: theme === 'light' ? '#F0F1F3' : '#19191B',
            color: theme === 'light' ? 'var(--foreground)' : '#fff',
          }}
          type="button"
        >
          <Circle
            className="size-2 fill-current"
            style={{
              color:
                TON_NETWORK_MODE === 'testnet'
                  ? 'var(--warning)'
                  : 'var(--success)',
            }}
          />
          {TON_NETWORK_LABEL}
          <ChevronDown className="size-3 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-[180px] rounded-xl p-2">
        {options.map((option) => (
          <DropdownMenuItem
            key={option.mode}
            className="cursor-pointer gap-2.5 rounded-xl px-3.5 py-3 text-[15px] font-medium"
            onClick={() => setTonNetworkMode(option.mode)}
          >
            <Circle
              className="size-2 fill-current"
              style={{
                color:
                  option.mode === 'testnet'
                    ? 'var(--warning)'
                    : 'var(--success)',
              }}
            />
            {option.label}
            {TON_NETWORK_MODE === option.mode ? (
              <Check className="size-4 ml-auto" />
            ) : null}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function kindBadgeClass(extension: ExtensionInspection): string {
  if (extension.infoStatus === 'ready') {
    return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-500';
  }

  if (extension.infoStatus === 'missing-getter') {
    return 'border-amber-500/30 bg-amber-500/10 text-amber-500';
  }

  if (extension.infoStatus === 'inactive') {
    return 'border-muted-foreground/25 bg-muted text-muted-foreground';
  }

  return 'border-destructive/30 bg-destructive/10 text-destructive';
}

function Popup({ popup, onClose }: { popup: PopupState; onClose: () => void }) {
  return (
    <div className="fixed right-4 top-4 z-50 w-[min(420px,calc(100vw-32px))] animate-in">
      <Alert
        variant={
          popup.tone === 'danger'
            ? 'destructive'
            : popup.tone === 'success'
              ? 'success'
              : 'default'
        }
        className={cn(
          'bg-card shadow-lg',
          popup.tone === 'warning' &&
            'border-amber-500/30 text-amber-500 *:data-[slot=alert-description]:text-amber-500/90',
        )}
      >
        <AlertTriangle className="size-4" />
        <AlertTitle>{popup.title}</AlertTitle>
        <AlertDescription>{popup.message}</AlertDescription>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="absolute right-2 top-2 size-7"
          title="Close"
          onClick={onClose}
        >
          <X className="size-4" />
        </Button>
      </Alert>
    </div>
  );
}

function ExtensionCard({
  extension,
  inspectedWallet,
  pendingExternalAction,
  disabled,
  onSendExternal,
}: {
  extension: ExtensionInspection;
  inspectedWallet: string;
  pendingExternalAction: ExtensionExternalAction | null;
  disabled: boolean;
  onSendExternal: (
    extension: ExtensionInspection,
    action: ExtensionExternalAction,
  ) => void;
}) {
  const extensionInfo =
    extension.infoStatus === 'ready' ? extension.info : null;
  const linkedToWallet =
    extensionInfo !== null &&
    sameAddress(extensionInfo.walletAddress, inspectedWallet);

  return (
    <article className="rounded-xl border bg-card p-4 shadow-sm">
      <div className="flex flex-col gap-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="mb-1 text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground">
              Extension
            </div>
            <h3
              className="truncate font-mono text-sm font-semibold"
              title={extension.address}
            >
              {extension.address}
            </h3>
          </div>
          <Button
            asChild
            variant="outline"
            size="icon"
            title="Open in Tonviewer"
          >
            <a href={extension.explorerUrl} target="_blank" rel="noreferrer">
              <ExternalLink className="size-4" />
            </a>
          </Button>
        </div>

        <div className="flex flex-wrap gap-2">
          <Badge variant="outline">{extension.state}</Badge>
          <Badge variant="outline" className={kindBadgeClass(extension)}>
            {extension.kindLabel}
          </Badge>
          {linkedToWallet ? (
            <Badge
              variant="outline"
              className="border-primary/30 bg-primary/10 text-primary"
            >
              Linked wallet
            </Badge>
          ) : null}
        </div>

        <Separator />

        <div>
          <Metric label="Balance" value={formatGramAmount(extension.balance)} />
          {extensionInfo ? (
            <>
              <Metric label="Admin" value={extensionInfo.admin} mono />
              <Metric
                label="Wallet V5"
                value={extensionInfo.walletAddress}
                mono
              />
              <Metric
                label="Subscription"
                value={formatGramAmount(extensionInfo.subscriptionAmount)}
              />
              <Metric
                label="Last Payment"
                value={formatUnixTimestamp(extensionInfo.lastPaymentTime)}
              />
              <Metric
                label="Interval"
                value={formatInterval(extensionInfo.paymentTimeInterval)}
              />
              <div className="mt-3 grid gap-2 sm:grid-cols-2">
                <Button
                  type="button"
                  size="sm"
                  disabled={disabled}
                  onClick={() =>
                    onSendExternal(extension, 'collectPaymentFromWallet')
                  }
                >
                  {pendingExternalAction === 'collectPaymentFromWallet' ? (
                    <span className="spinner" />
                  ) : (
                    <RefreshCcw className="size-4" />
                  )}
                  {EXTENSION_EXTERNAL_ACTION_LABELS.collectPaymentFromWallet}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={disabled}
                  onClick={() =>
                    onSendExternal(extension, 'withdrawExtensionBalance')
                  }
                >
                  {pendingExternalAction === 'withdrawExtensionBalance' ? (
                    <span className="spinner" />
                  ) : (
                    <WalletCards className="size-4" />
                  )}
                  {EXTENSION_EXTERNAL_ACTION_LABELS.withdrawExtensionBalance}
                </Button>
              </div>
            </>
          ) : (
            <Alert
              variant={
                extension.infoStatus === 'missing-getter'
                  ? 'default'
                  : 'destructive'
              }
              className={cn(
                'mt-3',
                extension.infoStatus === 'missing-getter' &&
                  'border-amber-500/30 text-amber-500 *:data-[slot=alert-description]:text-amber-500/90',
              )}
            >
              <AlertTriangle className="size-4" />
              <AlertTitle>{extension.kindLabel}</AlertTitle>
              <AlertDescription>{extension.errorMessage}</AlertDescription>
            </Alert>
          )}
        </div>
      </div>
    </article>
  );
}

export default function App() {
  const [tonConnectUI] = useTonConnectUI();
  const wallet = useTonWallet();
  const walletAddress = useTonAddress(false) || undefined;
  const walletChain = wallet?.account.chain;

  const [theme, setTheme] = useState<Theme>(() => {
    const savedTheme = localStorage.getItem('simple-extension-theme');
    return savedTheme === 'light' ? 'light' : 'dark';
  });
  const [walletInput, setWalletInput] = useState('');
  const deferredWalletInput = useDeferredValue(walletInput);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [inspection, setInspection] = useState<InspectionState>(
    initialInspectionState,
  );
  const [pendingExternalAction, setPendingExternalAction] =
    useState<PendingExternalAction | null>(null);
  const [statusMessage, setStatusMessage] = useState(
    'Ready to inspect a Wallet V5 address.',
  );
  const [popup, setPopup] = useState<PopupState | null>(null);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('simple-extension-theme', theme);
    tonConnectUI.uiOptions = {
      uiPreferences: {
        theme: theme === 'light' ? THEME.LIGHT : THEME.DARK,
      },
    };
  }, [theme, tonConnectUI]);

  useEffect(() => {
    if (!popup) {
      return;
    }

    const timeout = window.setTimeout(() => setPopup(null), 9000);
    return () => window.clearTimeout(timeout);
  }, [popup]);

  const walletPreview = useMemo(
    () => getAddressPreview(deferredWalletInput),
    [deferredWalletInput],
  );
  const connectedWalletDisplay = useMemo(() => {
    if (!walletAddress) {
      return 'Not connected';
    }

    try {
      return formatAddressForNetwork(walletAddress, walletChain ?? TON_CHAIN);
    } catch {
      return walletAddress;
    }
  }, [walletAddress, walletChain]);

  const walletNetworkMismatch =
    walletChain !== undefined && walletChain !== TON_CHAIN;
  const busy = pendingAction !== null || pendingExternalAction !== null;
  const extensions = inspection.data?.extensions ?? [];
  const simpleExtensions = extensions.filter(
    (extension) => extension.infoStatus === 'ready',
  );
  const missingGetterExtensions = extensions.filter(
    (extension) => extension.infoStatus === 'missing-getter',
  );

  function showPopup(next: Omit<PopupState, 'id'>) {
    setPopup({
      id: Date.now(),
      ...next,
    });
  }

  function handleNormalizeWalletInput() {
    if (walletPreview.value) {
      setWalletInput(walletPreview.value);
    }
  }

  function handleUseConnectedWallet() {
    if (!walletAddress) {
      setStatusMessage('Connect a wallet first.');
      return;
    }

    try {
      setWalletInput(normalizeAddress(walletAddress));
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    }
  }

  async function handleInspect() {
    if (!walletInput.trim()) {
      setStatusMessage('Enter a wallet address first.');
      return;
    }

    if (walletPreview.error) {
      setStatusMessage(walletPreview.error);
      return;
    }

    setPopup(null);
    setPendingAction('inspect');
    setInspection({
      status: 'loading',
      data: null,
      message: 'Reading get_extensions and extensionInfo getters...',
      fetchedAt: null,
    });

    try {
      const snapshot = await inspectWallet(walletInput);
      const missingGetterCount = snapshot.extensions.filter(
        (extension) => extension.infoStatus === 'missing-getter',
      ).length;

      setWalletInput(snapshot.address);
      setInspection({
        status: snapshot.extensions.length === 0 ? 'empty' : 'ready',
        data: snapshot,
        message:
          snapshot.extensions.length === 0
            ? 'Wallet is active, but get_extensions returned no installed extensions.'
            : `Loaded ${snapshot.extensions.length} extension${snapshot.extensions.length === 1 ? '' : 's'}.`,
        fetchedAt: new Date().toLocaleTimeString(),
      });
      setStatusMessage(
        snapshot.extensions.length === 0
          ? `No extensions found for ${snapshot.address}.`
          : `Loaded ${snapshot.extensions.length} extension${snapshot.extensions.length === 1 ? '' : 's'} from ${snapshot.address.slice(0, 4)}...`,
      );

      if (missingGetterCount > 0) {
        showPopup({
          title: 'Missing extensionInfo getter',
          message: `${missingGetterCount} installed extension${missingGetterCount === 1 ? '' : 's'} returned exit code -11 for extensionInfo(). Those contracts do not expose the SimpleExtension getter.`,
          tone: 'warning',
        });
      }
    } catch (error) {
      const message = getErrorMessage(error);
      setInspection({
        status: 'error',
        data: null,
        message,
        fetchedAt: null,
      });
      setStatusMessage(message);

      if (error instanceof MissingGetMethodError) {
        showPopup({
          title: 'Missing get_extensions getter',
          message,
          tone: 'warning',
        });
      } else {
        showPopup({
          title: 'Wallet inspection failed',
          message,
          tone: 'danger',
        });
      }
    } finally {
      setPendingAction(null);
    }
  }

  async function handleSendExternal(
    extension: ExtensionInspection,
    action: ExtensionExternalAction,
  ) {
    if (extension.infoStatus !== 'ready' || !extension.info) {
      setStatusMessage(
        'External messages are available for SimpleExtension contracts only.',
      );
      return;
    }

    setPopup(null);
    setPendingExternalAction({
      rawAddress: extension.rawAddress,
      action,
    });

    try {
      const result = await sendExtensionExternalMessage({
        extensionAddress: extension.address,
        action,
      });
      const label = EXTENSION_EXTERNAL_ACTION_LABELS[action];

      setStatusMessage(
        `${label} external message sent to ${result.address}. Inspect again after confirmation to refresh on-chain state.`,
      );
      showPopup({
        title: 'External message sent',
        message: `${label} was sent to ${result.address}.`,
        tone: 'success',
      });
    } catch (error) {
      const message = getErrorMessage(error);

      setStatusMessage(message);
      showPopup({
        title: 'External message failed',
        message,
        tone: 'danger',
      });
    } finally {
      setPendingExternalAction(null);
    }
  }

  return (
    <div className="flex min-h-full flex-col bg-background text-foreground">
      {popup ? <Popup popup={popup} onClose={() => setPopup(null)} /> : null}

      <header
        className="sticky top-0 z-50 flex h-[60px] items-center justify-between border-b px-7 max-sm:h-auto max-sm:flex-wrap max-sm:gap-2.5 max-sm:px-4 max-sm:py-3"
        style={{
          background: theme === 'light' ? '#fff' : '#08080A',
          borderBottomColor:
            theme === 'light' ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.06)',
        }}
      >
        <div className="flex items-center gap-6 max-sm:w-full max-sm:justify-between max-sm:gap-2.5">
          <div className="flex items-center gap-2.5 text-[17px] font-bold max-sm:text-[15px]">
            <div className="flex h-8 w-8 items-center justify-center rounded-[9px] bg-[#0098EA] text-white max-sm:h-7 max-sm:w-7 max-sm:rounded-[7px]">
              <PlugZap className="size-4 max-sm:size-3.5" />
            </div>
            Wallet W5 Extension
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-10 rounded-full max-sm:size-9"
            style={{
              background: theme === 'light' ? '#F0F1F3' : '#19191B',
              color: theme === 'light' ? 'var(--foreground)' : '#fff',
            }}
            title={
              theme === 'dark'
                ? 'Switch to light theme'
                : 'Switch to dark theme'
            }
            onClick={() =>
              setTheme((current) => (current === 'dark' ? 'light' : 'dark'))
            }
          >
            {theme === 'dark' ? (
              <Sun className="size-[18px]" />
            ) : (
              <Moon className="size-[18px]" />
            )}
          </Button>
          <NetworkDropdown theme={theme} />
          <TonConnectButton />
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-7xl flex-1 flex-col gap-4 px-4 py-6 md:px-6 md:py-8">
        <section className="min-w-0">
          <div className="mb-1 text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground">
            Wallet V5 subscription extension
          </div>
          <h1 className="truncate text-2xl font-semibold tracking-tight md:text-3xl">
            Simple Extension Inspector
          </h1>
        </section>

        <section className="grid flex-1 gap-4 lg:grid-cols-[420px_minmax(0,1fr)]">
          <div className="flex flex-col gap-4">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Search className="size-5 text-primary" />
                  Inspect wallet
                </CardTitle>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <Field
                  label="Wallet address"
                  hint={
                    walletPreview.value
                      ? `Normalized: ${shortAddress(walletPreview.value)}`
                      : 'Accepts raw, bounceable, or non-bounceable addresses.'
                  }
                  error={walletPreview.error}
                >
                  <Input
                    value={walletInput}
                    onChange={(event) => setWalletInput(event.target.value)}
                    onBlur={handleNormalizeWalletInput}
                    placeholder="EQ..."
                    spellCheck={false}
                    className="font-mono"
                  />
                </Field>

                <div className="grid grid-cols-2 gap-2">
                  <Button
                    type="button"
                    disabled={busy || !walletInput.trim()}
                    onClick={handleInspect}
                  >
                    {pendingAction === 'inspect' ? (
                      <span className="spinner" />
                    ) : (
                      <RefreshCcw className="size-4" />
                    )}
                    Inspect
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    disabled={!walletAddress}
                    onClick={handleUseConnectedWallet}
                  >
                    <WalletCards className="size-4" />
                    Use connected
                  </Button>
                </div>

                <Alert>
                  <PlugZap className="size-4" />
                  <AlertTitle>Status</AlertTitle>
                  <AlertDescription>{statusMessage}</AlertDescription>
                </Alert>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <WalletCards className="size-5 text-primary" />
                  Connected wallet
                </CardTitle>
              </CardHeader>
              <CardContent>
                <Metric label="Address" value={connectedWalletDisplay} mono />
                <Metric label="Read network" value={TON_NETWORK_LABEL} />
                {walletNetworkMismatch ? (
                  <Alert
                    variant="default"
                    className="mt-3 border-amber-500/30 text-amber-500 *:data-[slot=alert-description]:text-amber-500/90"
                  >
                    <AlertTriangle className="size-4" />
                    <AlertTitle>Network mismatch</AlertTitle>
                    <AlertDescription>
                      Connected wallet network differs from the selected read
                      network.
                    </AlertDescription>
                  </Alert>
                ) : null}
              </CardContent>
            </Card>

            {inspection.data ? (
              <Card>
                <CardHeader>
                  <CardTitle>Wallet state</CardTitle>
                </CardHeader>
                <CardContent>
                  <Metric
                    label="Balance"
                    value={formatGramAmount(inspection.data.balance)}
                  />
                  <Metric
                    label="Address"
                    value={inspection.data.address}
                    mono
                  />
                </CardContent>
              </Card>
            ) : null}
          </div>

          <div className="flex min-w-0 flex-col gap-4">
            <div className="rounded-xl border bg-card shadow-sm">
              <div className="flex flex-col gap-3 p-6 md:flex-row md:items-start md:justify-between">
                <div>
                  <div className="mb-1 text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground">
                    get_extensions results
                  </div>
                  <h2 className="text-xl font-semibold tracking-tight">
                    Installed extensions
                  </h2>
                </div>

                <div className="flex flex-wrap gap-2">
                  <Badge variant="secondary">{extensions.length} total</Badge>
                  <Badge
                    variant="outline"
                    className="border-emerald-500/30 bg-emerald-500/10 text-emerald-500"
                  >
                    {simpleExtensions.length} SimpleExtension
                  </Badge>
                  {missingGetterExtensions.length > 0 ? (
                    <Badge
                      variant="outline"
                      className="border-amber-500/30 bg-amber-500/10 text-amber-500"
                    >
                      {missingGetterExtensions.length} missing getter
                    </Badge>
                  ) : null}
                </div>
              </div>

              <Separator />

              <div className="p-6">
                {inspection.status === 'idle' ? (
                  <div className="flex min-h-90 flex-col items-center justify-center rounded-xl border border-dashed text-center text-muted-foreground">
                    <PlugZap className="mb-3 size-8" />
                    <p>Inspect a wallet to load installed extensions.</p>
                  </div>
                ) : null}

                {inspection.status === 'loading' ? (
                  <div className="flex min-h-90 flex-col items-center justify-center rounded-xl border border-dashed text-center text-muted-foreground">
                    <span className="spinner mb-3" />
                    <p>Reading wallet and extension getters...</p>
                  </div>
                ) : null}

                {inspection.status === 'error' ? (
                  <Alert variant="destructive">
                    <AlertTriangle className="size-4" />
                    <AlertTitle>Inspection failed</AlertTitle>
                    <AlertDescription>{inspection.message}</AlertDescription>
                  </Alert>
                ) : null}

                {inspection.status === 'empty' && inspection.data ? (
                  <Alert>
                    <PlugZap className="size-4" />
                    <AlertTitle>No extensions</AlertTitle>
                    <AlertDescription>
                      get_extensions returned an empty dictionary for{' '}
                      {inspection.data.address}.
                    </AlertDescription>
                  </Alert>
                ) : null}

                {inspection.data?.skippedSelfReferenceCount ? (
                  <Alert className="mb-4">
                    <AlertTriangle className="size-4" />
                    <AlertTitle>Self reference skipped</AlertTitle>
                    <AlertDescription>
                      Skipped {inspection.data.skippedSelfReferenceCount}{' '}
                      extension{' '}
                      {inspection.data.skippedSelfReferenceCount === 1
                        ? 'entry'
                        : 'entries'}{' '}
                      that resolved to the wallet itself.
                    </AlertDescription>
                  </Alert>
                ) : null}

                {extensions.length > 0 ? (
                  <div className="grid gap-4 xl:grid-cols-2">
                    {extensions.map((extension) => (
                      <ExtensionCard
                        key={extension.rawAddress}
                        extension={extension}
                        inspectedWallet={inspection.data?.address ?? ''}
                        pendingExternalAction={
                          pendingExternalAction?.rawAddress ===
                          extension.rawAddress
                            ? pendingExternalAction.action
                            : null
                        }
                        disabled={busy}
                        onSendExternal={handleSendExternal}
                      />
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          </div>
        </section>
      </main>
    </div>
  );
}
