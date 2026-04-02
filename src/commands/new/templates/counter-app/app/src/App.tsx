import { useDeferredValue, useMemo, useState } from 'react';
import {
  TonConnectButton,
  useAddress,
  useBalance,
  useNetwork,
  useSendTransaction,
} from '@ton/appkit-react';

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
  TON_NETWORK,
  TON_NETWORK_LABEL,
  TONVIEWER_URL,
} from './lib/ton';

type PendingAction = 'deploy' | 'increase' | 'decrease' | 'fetch' | null;

interface CounterValueState {
  status: 'idle' | 'loading' | 'ready' | 'missing' | 'error';
  value: bigint | null;
  message: string;
  fetchedAt: string | null;
}

const initialCounterValueState: CounterValueState = {
  status: 'idle',
  value: null,
  message: 'Enter a counter address and fetch the current value.',
  fetchedAt: null,
};

export default function App() {
  const walletAddress = useAddress();
  const walletBalance = useBalance();
  const walletNetwork = useNetwork();

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

  const preview = useMemo(() => {
    try {
      return {
        value: getCounterPreview(deferredCounterId),
        error: null,
      };
    } catch (error) {
      return {
        value: null,
        error: getErrorMessage(error),
      };
    }
  }, [deferredCounterId]);

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
      message: snapshot.isDeployed
        ? 'Latest value loaded from chain.'
        : 'This address is not deployed on the selected network yet.',
      fetchedAt: new Date().toLocaleTimeString(),
    });
  }

  async function handleDeploy() {
    if (!walletReady) {
      setStatusMessage('Connect a wallet before deploying a counter.');
      return;
    }

    if (preview.error || !preview.value) {
      setStatusMessage(preview.error ?? 'Counter ID is invalid.');
      return;
    }

    setPendingAction('deploy');

    try {
      const deployment = buildDeployTransaction(counterId, deployValue);
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

  const contractExplorerUrl = counterAddress
    ? `${TONVIEWER_URL}/${encodeURIComponent(counterAddress)}`
    : null;

  return (
    <main className="app-root">
      <header className="site-header">
        <div className="site-header-inner">
          <div className="page-head-main">
            <div className="page-title-row">
              <h1>Counter dApp</h1>
              <span className="meta-badge">{TON_NETWORK_LABEL}</span>
            </div>
            <div className="meta-row">
              {walletNetworkMismatch ? (
                <span className="meta-badge meta-badge-warning">
                  Connected wallet uses another network
                </span>
              ) : null}
            </div>
          </div>
          <div className="wallet-summary">
            <dl className="details-list">
              <div>
                <dt>Wallet</dt>
                <dd>{displayWalletAddress}</dd>
              </div>
              <div>
                <dt>Balance</dt>
                <dd>
                  {walletBalance.data
                    ? `${walletBalance.data} TON`
                    : 'Connect to load'}
                </dd>
              </div>
            </dl>
            <div className="wallet-action">
              <TonConnectButton />
            </div>
          </div>
        </div>
      </header>

      <div className="app-shell">
        <section className="panel status-card">
          <div className="status-row">
            <span aria-hidden="true" className="status-icon">
              <svg
                fill="none"
                height="16"
                viewBox="0 0 16 16"
                width="16"
                xmlns="http://www.w3.org/2000/svg"
              >
                <circle cx="8" cy="8" r="6.25" stroke="currentColor" />
                <path
                  d="M6.9 6.1a1.27 1.27 0 0 1 2.43.53c0 .73-.42 1.08-.84 1.43-.4.33-.8.66-.8 1.29"
                  stroke="currentColor"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <circle cx="8" cy="11.25" fill="currentColor" r=".75" />
              </svg>
            </span>
            <div className="status-copy">
              <p className="status-label">Status</p>
              <p className="status-text">{statusMessage}</p>
            </div>
          </div>
        </section>

        <section className="content-grid">
          <article className="panel section-card">
            <div className="section-head">
              <div>
                <h2>Create New Counter</h2>
              </div>
            </div>

            <label className="field">
              <span>Counter ID</span>
              <input
                type="number"
                min="0"
                step="1"
                value={counterId}
                onChange={(event) => setCounterId(event.target.value)}
              />
            </label>

            <label className="field">
              <span>Deploy value (TON)</span>
              <input
                type="text"
                inputMode="decimal"
                value={deployValue}
                onChange={(event) => setDeployValue(event.target.value)}
              />
            </label>

            <div className="preview-card">
              <p className="preview-label">Deploy address</p>
              <span className="preview-address">
                {preview.value?.address ?? preview.error}
              </span>
            </div>

            <div className="button-row">
              <button
                className="primary-button"
                disabled={!walletReady || Boolean(preview.error) || busy}
                onClick={handleDeploy}
                type="button"
              >
                {pendingAction === 'deploy'
                  ? 'Creating...'
                  : 'Create New Counter'}
              </button>
              <button
                className="ghost-button"
                disabled={!preview.value || busy}
                onClick={handleUsePreviewAddress}
                type="button"
              >
                Use This Address
              </button>
            </div>
          </article>

          <article className="panel section-card">
            <div className="section-head">
              <div>
                <h2>Work With Counter</h2>
              </div>
              {contractExplorerUrl ? (
                <a
                  className="text-link"
                  href={contractExplorerUrl}
                  rel="noreferrer"
                  target="_blank"
                >
                  Open in Tonviewer
                </a>
              ) : null}
            </div>

            <label className="field">
              <span>Counter address</span>
              <input
                type="text"
                placeholder="EQ..."
                value={counterAddress}
                onBlur={handleNormalizeAddress}
                onChange={(event) => setCounterAddress(event.target.value)}
              />
            </label>

            <div className="field-grid">
              <label className="field">
                <span>Step</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  value={step}
                  onChange={(event) => setStep(event.target.value)}
                />
              </label>
              <label className="field">
                <span>Message value (TON)</span>
                <input
                  type="text"
                  inputMode="decimal"
                  value={messageValue}
                  onChange={(event) => setMessageValue(event.target.value)}
                />
              </label>
            </div>

            <div className="counter-readout">
              <p className="preview-label">Current value</p>
              <strong>
                {counterValue.status === 'ready' && counterValue.value !== null
                  ? counterValue.value.toString()
                  : '—'}
              </strong>
              <p>{counterValue.message}</p>
              {counterValue.fetchedAt ? (
                <span>Updated at {counterValue.fetchedAt}</span>
              ) : null}
            </div>

            <div className="button-row">
              <button
                className="primary-button"
                disabled={!walletReady || busy}
                onClick={() => handleAction('increase')}
                type="button"
              >
                {pendingAction === 'increase' ? 'Increasing...' : 'Increase'}
              </button>
              <button
                className="secondary-button"
                disabled={!walletReady || busy}
                onClick={() => handleAction('decrease')}
                type="button"
              >
                {pendingAction === 'decrease' ? 'Decreasing...' : 'Decrease'}
              </button>
              <button
                className="ghost-button"
                disabled={busy}
                onClick={handleFetch}
                type="button"
              >
                {pendingAction === 'fetch'
                  ? 'Fetching...'
                  : 'Fetch Latest Value'}
              </button>
            </div>
          </article>
        </section>
      </div>
    </main>
  );
}
