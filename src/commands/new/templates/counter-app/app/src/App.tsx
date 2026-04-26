import { useDeferredValue, useEffect, useMemo, useState } from 'react';
import {
  TonConnectButton,
  useAddress,
  useAppKitTheme,
  useBalance,
  useNetwork,
  useSendTransaction,
} from '@ton/appkit-react';
import {
  Calculator,
  Check,
  ChevronDown,
  Circle,
  ExternalLink,
  Minus,
  Moon,
  Plus,
  RefreshCcw,
  Sun,
  WalletCards,
} from 'lucide-react';

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
import {
  buildCounterActionTransaction,
  buildDeployTransaction,
  DEFAULT_COUNTER_ID,
  DEFAULT_DEPLOY_VALUE,
  DEFAULT_MESSAGE_VALUE,
  DEFAULT_STEP,
  getCounterPreview,
  getErrorMessage,
  isCounterDeployed,
  normalizeCounterAddress,
  readCounter,
} from './lib/counter';
import {
  formatAddressForNetwork,
  setTonNetworkMode,
  TON_NETWORK,
  TON_NETWORK_MODE,
  TONSCAN_ADDRESS_URL,
  type TonNetworkMode,
} from './lib/ton';

type PendingAction = 'deploy' | 'increase' | 'decrease' | 'fetch' | null;
type Theme = 'dark' | 'light';

interface CounterValueState {
  status: 'idle' | 'loading' | 'ready' | 'missing' | 'error';
  value: bigint | null;
  owner: string | null;
  message: string;
  fetchedAt: string | null;
}

const initialCounterValueState: CounterValueState = {
  status: 'idle',
  value: null,
  owner: null,
  message: 'Enter a counter address and fetch the current value.',
  fetchedAt: null,
};

export default function App() {
  const walletAddress = useAddress();
  const walletBalance = useBalance();
  const walletNetwork = useNetwork();
  const [, setAppKitTheme] = useAppKitTheme();

  const [theme, setTheme] = useState<Theme>(() => {
    const savedTheme = localStorage.getItem('counter-theme');
    return savedTheme === 'light' ? 'light' : 'dark';
  });
  const [counterId, setCounterId] = useState(DEFAULT_COUNTER_ID);
  const deferredCounterId = useDeferredValue(counterId);
  const [counterAddress, setCounterAddress] = useState('');
  const [step, setStep] = useState(DEFAULT_STEP);
  const [deployValue, setDeployValue] = useState(DEFAULT_DEPLOY_VALUE);
  const [messageValue, setMessageValue] = useState(DEFAULT_MESSAGE_VALUE);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [statusMessage, setStatusMessage] = useState(
    'Connect a wallet, deploy a counter, then interact with it.',
  );
  const [counterValue, setCounterValue] = useState<CounterValueState>(
    initialCounterValueState,
  );

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('counter-theme', theme);
    setAppKitTheme(theme);
  }, [setAppKitTheme, theme]);

  const preview = useMemo(() => {
    if (!walletAddress) {
      return {
        value: null,
        error: 'Connect a wallet to derive the owner and deploy address.',
      };
    }

    try {
      return {
        value: getCounterPreview(deferredCounterId, walletAddress),
        error: null,
      };
    } catch (error) {
      return {
        value: null,
        error: getErrorMessage(error),
      };
    }
  }, [deferredCounterId, walletAddress]);

  const displayWalletAddress = useMemo(() => {
    if (!walletAddress) {
      return 'Not connected';
    }

    try {
      return formatAddressForNetwork(
        walletAddress,
        walletNetwork?.chainId ?? TON_NETWORK.chainId,
      );
    } catch {
      return walletAddress;
    }
  }, [walletAddress, walletNetwork]);

  const { mutateAsync: sendTransaction, isPending: isWalletPromptOpen } =
    useSendTransaction();
  const walletReady = Boolean(walletAddress);
  const walletNetworkMismatch =
    walletNetwork !== undefined &&
    walletNetwork.chainId !== TON_NETWORK.chainId;
  const busy = pendingAction !== null || isWalletPromptOpen;

  async function fetchCounter(addressValue: string) {
    const snapshot = await readCounter(addressValue);

    setCounterAddress(snapshot.address);
    setCounterValue({
      status: snapshot.isDeployed ? 'ready' : 'missing',
      value: snapshot.value,
      owner: snapshot.owner,
      message: snapshot.isDeployed
        ? 'Latest value loaded from chain.'
        : 'This address is not deployed on the selected network yet.',
      fetchedAt: new Date().toLocaleTimeString(),
    });
  }

  async function handleDeploy() {
    if (!walletAddress) {
      setStatusMessage('Connect a wallet before deploying a counter.');
      return;
    }

    if (preview.error || !preview.value) {
      setStatusMessage(preview.error ?? 'Counter ID is invalid.');
      return;
    }

    setPendingAction('deploy');

    try {
      const deployment = buildDeployTransaction(
        counterId,
        deployValue,
        walletAddress,
      );
      const alreadyDeployed = await isCounterDeployed(
        deployment.preview.contract.address,
      );

      if (alreadyDeployed) {
        setCounterAddress(deployment.address);
        setStatusMessage(
          `Counter ${counterId} is already deployed at ${deployment.address}.`,
        );
        return;
      }

      await sendTransaction(deployment.request);
      setCounterAddress(deployment.address);
      setCounterValue(initialCounterValueState);
      setStatusMessage(
        `Deployment request sent. After the transaction lands, fetch the value for ${deployment.address}.`,
      );
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    } finally {
      setPendingAction(null);
    }
  }

  async function handleAction(action: 'increase' | 'decrease') {
    if (!walletReady) {
      setStatusMessage('Connect a wallet before sending contract messages.');
      return;
    }

    setPendingAction(action);

    try {
      const transaction = buildCounterActionTransaction({
        action,
        addressValue: counterAddress,
        messageValue,
        stepValue: step,
      });

      await sendTransaction(transaction.request);
      setCounterAddress(transaction.address);
      setStatusMessage(
        `${action === 'increase' ? 'Increase' : 'Decrease'} request sent. Fetch the latest value after confirmation.`,
      );
    } catch (error) {
      setStatusMessage(getErrorMessage(error));
    } finally {
      setPendingAction(null);
    }
  }

  async function handleFetch() {
    if (!counterAddress.trim()) {
      setStatusMessage('Provide a counter address first.');
      return;
    }

    setPendingAction('fetch');
    setCounterValue((current) => ({
      ...current,
      status: 'loading',
      message: 'Reading contract state...',
    }));

    try {
      await fetchCounter(counterAddress);
      setStatusMessage('Counter state refreshed from chain.');
    } catch (error) {
      const message = getErrorMessage(error);
      setCounterValue({
        status: 'error',
        value: null,
        owner: null,
        message,
        fetchedAt: null,
      });
      setStatusMessage(message);
    } finally {
      setPendingAction(null);
    }
  }

  function handleUsePreviewAddress() {
    if (!preview.value) {
      return;
    }

    setCounterAddress(preview.value.address);
    setStatusMessage(`Active counter address set to ${preview.value.address}.`);
  }

  function handleNormalizeAddress() {
    if (!counterAddress.trim()) {
      return;
    }

    try {
      setCounterAddress(normalizeCounterAddress(counterAddress));
    } catch {
      // Leave the original value in place so the user can fix it.
    }
  }

  function toggleTheme() {
    setTheme((current) => (current === 'dark' ? 'light' : 'dark'));
  }

  const contractExplorerUrl = counterAddress
    ? `${TONSCAN_ADDRESS_URL}/${encodeURIComponent(counterAddress)}`
    : null;

  return (
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
              <Calculator className="size-4 max-sm:size-3.5" />
            </div>
            Counter dApp
          </div>
          <nav
            className="flex gap-0.5 p-[3px] h-10 rounded-full items-center max-sm:h-9"
            style={{ background: theme === 'light' ? '#F0F1F3' : '#19191B' }}
          >
            <Button
              variant="ghost"
              size="sm"
              className="rounded-full px-4 h-[34px] text-[15px] font-bold max-sm:h-[30px] max-sm:px-3.5 max-sm:text-[13px] bg-[#0098EA] text-white hover:bg-[#0098EA] hover:text-white"
              type="button"
            >
              Counter
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
            onClick={toggleTheme}
            title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} theme`}
            type="button"
          >
            {theme === 'dark' ? (
              <Sun className="size-[18px]" />
            ) : (
              <Moon className="size-[18px]" />
            )}
          </Button>
          <NetworkDropdown
            network={TON_NETWORK_MODE}
            setNetwork={setTonNetworkMode}
            theme={theme}
          />
          <TonConnectButton />
        </div>
      </header>

      <main className="flex-1 max-w-[960px] w-full mx-auto px-6 pt-9 pb-15 max-sm:px-4 max-sm:pt-6 max-sm:pb-12">
        <section className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 items-center mb-4 rounded-xl border bg-card px-4 py-2.5 max-md:grid-cols-1 max-sm:px-3">
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="size-7 rounded-full bg-secondary flex items-center justify-center text-muted-foreground shrink-0">
              <WalletCards className="size-3.5" />
            </div>
            <div className="min-w-0">
              <p className="text-xs font-bold uppercase tracking-[0.08em] text-muted-foreground">
                Status
              </p>
              <p
                className="truncate text-[14px] leading-5 text-foreground/80"
                title={statusMessage}
              >
                {statusMessage}
              </p>
            </div>
          </div>
          <dl className="flex items-center gap-3 text-right max-md:text-left max-sm:grid max-sm:grid-cols-2 max-sm:gap-2">
            <div className="min-w-0">
              <dt className="text-[11px] font-bold uppercase tracking-[0.08em] text-muted-foreground">
                Wallet
              </dt>
              <dd className="max-w-[220px] truncate text-[13px] font-medium max-sm:max-w-full">
                {displayWalletAddress}
              </dd>
            </div>
            <div>
              <dt className="text-[11px] font-bold uppercase tracking-[0.08em] text-muted-foreground">
                Balance
              </dt>
              <dd className="text-[13px] font-medium">
                {walletBalance.data
                  ? `${walletBalance.data} TON`
                  : 'Connect to load'}
              </dd>
            </div>
          </dl>
        </section>

        <section className="grid grid-cols-2 gap-4 max-md:grid-cols-1">
          <Card className="rounded-xl gap-0 py-0 overflow-hidden">
            <CardHeader className="px-5 py-5 border-b gap-1.5">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs font-bold uppercase tracking-[0.08em] text-muted-foreground">
                    Deploy
                  </p>
                  <CardTitle className="text-[22px] tracking-normal">
                    Create New Counter
                  </CardTitle>
                </div>
                {walletNetworkMismatch ? (
                  <span className="rounded-full border border-[color:var(--warning)]/35 px-3 py-1 text-xs font-bold text-[color:var(--warning)]">
                    Wrong wallet network
                  </span>
                ) : null}
              </div>
            </CardHeader>
            <CardContent className="px-5 py-5">
              <div className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="counter-id">Counter ID</Label>
                  <Input
                    id="counter-id"
                    min="0"
                    onChange={(event) => setCounterId(event.target.value)}
                    step="1"
                    type="number"
                    value={counterId}
                  />
                </div>

                <div className="grid gap-2">
                  <Label htmlFor="deploy-value">Deploy value (TON)</Label>
                  <Input
                    id="deploy-value"
                    inputMode="decimal"
                    onChange={(event) => setDeployValue(event.target.value)}
                    type="text"
                    value={deployValue}
                  />
                </div>

                <div className="rounded-xl border bg-secondary/45 p-4">
                  <p className="text-xs font-bold uppercase tracking-[0.08em] text-muted-foreground mb-2">
                    Deploy address
                  </p>
                  <p className="break-all text-sm font-mono">
                    {preview.value?.address ?? preview.error}
                  </p>
                  {preview.value ? (
                    <p className="mt-3 break-all text-xs text-muted-foreground">
                      Owner: {preview.value.owner}
                    </p>
                  ) : null}
                </div>

                <div className="flex flex-wrap gap-2">
                  <Button
                    className="rounded-full font-bold"
                    disabled={!walletReady || Boolean(preview.error) || busy}
                    onClick={handleDeploy}
                    type="button"
                  >
                    <Plus className="size-4" />
                    {pendingAction === 'deploy'
                      ? 'Creating...'
                      : 'Create New Counter'}
                  </Button>
                  <Button
                    className="rounded-full font-bold"
                    disabled={!preview.value || busy}
                    onClick={handleUsePreviewAddress}
                    type="button"
                    variant="secondary"
                  >
                    Use This Address
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="rounded-xl gap-0 py-0 overflow-hidden">
            <CardHeader className="px-5 py-5 border-b gap-1.5">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-xs font-bold uppercase tracking-[0.08em] text-muted-foreground">
                    Manage
                  </p>
                  <CardTitle className="text-[22px] tracking-normal">
                    Work With Counter
                  </CardTitle>
                </div>
                {contractExplorerUrl ? (
                  <Button
                    asChild
                    className="rounded-full font-bold"
                    size="sm"
                    variant="ghost"
                  >
                    <a
                      href={contractExplorerUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink className="size-4" />
                      Tonscan
                    </a>
                  </Button>
                ) : null}
              </div>
            </CardHeader>
            <CardContent className="px-5 py-5">
              <div className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="counter-address">Counter address</Label>
                  <Input
                    id="counter-address"
                    onBlur={handleNormalizeAddress}
                    onChange={(event) => setCounterAddress(event.target.value)}
                    placeholder="EQ..."
                    type="text"
                    value={counterAddress}
                  />
                </div>

                <div className="grid grid-cols-2 gap-3 max-sm:grid-cols-1">
                  <div className="grid gap-2">
                    <Label htmlFor="step">Step</Label>
                    <Input
                      id="step"
                      min="1"
                      onChange={(event) => setStep(event.target.value)}
                      step="1"
                      type="number"
                      value={step}
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="message-value">Message value (TON)</Label>
                    <Input
                      id="message-value"
                      inputMode="decimal"
                      onChange={(event) => setMessageValue(event.target.value)}
                      type="text"
                      value={messageValue}
                    />
                  </div>
                </div>

                <div className="rounded-xl border bg-secondary/45 p-4">
                  <p className="text-xs font-bold uppercase tracking-[0.08em] text-muted-foreground mb-2">
                    Current value
                  </p>
                  <strong className="block text-[44px] leading-none font-bold">
                    {counterValue.status === 'ready' &&
                    counterValue.value !== null
                      ? counterValue.value.toString()
                      : '-'}
                  </strong>
                  <p className="mt-2 text-sm text-muted-foreground">
                    {counterValue.message}
                  </p>
                  {counterValue.owner ? (
                    <p className="mt-2 break-all text-xs text-muted-foreground">
                      Owner: {counterValue.owner}
                    </p>
                  ) : null}
                  {counterValue.fetchedAt ? (
                    <p className="mt-2 text-xs font-medium text-muted-foreground">
                      Updated at {counterValue.fetchedAt}
                    </p>
                  ) : null}
                </div>

                <div className="flex flex-wrap gap-2">
                  <Button
                    className="rounded-full font-bold"
                    disabled={!walletReady || busy}
                    onClick={() => handleAction('increase')}
                    type="button"
                  >
                    <Plus className="size-4" />
                    {pendingAction === 'increase'
                      ? 'Increasing...'
                      : 'Increase'}
                  </Button>
                  <Button
                    className="rounded-full font-bold"
                    disabled={!walletReady || busy}
                    onClick={() => handleAction('decrease')}
                    type="button"
                    variant="secondary"
                  >
                    <Minus className="size-4" />
                    {pendingAction === 'decrease'
                      ? 'Decreasing...'
                      : 'Decrease'}
                  </Button>
                  <Button
                    className="rounded-full font-bold"
                    disabled={busy}
                    onClick={handleFetch}
                    type="button"
                    variant="ghost"
                  >
                    <RefreshCcw className="size-4" />
                    {pendingAction === 'fetch'
                      ? 'Fetching...'
                      : 'Fetch Latest Value'}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        </section>
      </main>
    </div>
  );
}

function NetworkDropdown({
  network,
  setNetwork,
  theme,
}: {
  network: TonNetworkMode;
  setNetwork: (network: TonNetworkMode) => void;
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
          type="button"
        >
          <Circle
            className="size-2 fill-current"
            style={{
              color:
                network === 'testnet' ? 'var(--warning)' : 'var(--success)',
            }}
          />
          {network === 'mainnet' ? 'Mainnet' : 'Testnet'}
          <ChevronDown className="size-3 opacity-50" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-[180px] rounded-xl p-2">
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setNetwork('mainnet')}
        >
          <Circle
            className="size-2 fill-current"
            style={{ color: 'var(--success)' }}
          />
          Mainnet
          {network === 'mainnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
        <DropdownMenuItem
          className="rounded-xl px-3.5 py-3 text-[15px] font-medium gap-2.5 cursor-pointer"
          onClick={() => setNetwork('testnet')}
        >
          <Circle
            className="size-2 fill-current"
            style={{ color: 'var(--warning)' }}
          />
          Testnet
          {network === 'testnet' && <Check className="size-4 ml-auto" />}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
