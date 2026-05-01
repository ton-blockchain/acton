import { Address, fromNano } from '@ton/core';
import { QueryClient } from '@tanstack/react-query';
import { TonClient } from '@ton/ton';
import { AppKit, Network, TonConnectConnector } from '@ton/appkit-react';

export type TonNetworkMode = 'mainnet' | 'testnet';

const NETWORK_STORAGE_KEY = 'simple-extension-network';
const MAINNET = Network.mainnet();
const TESTNET = Network.testnet();

function isNetworkMode(value: string | null): value is TonNetworkMode {
  return value === 'mainnet' || value === 'testnet';
}

function readNetworkMode(): TonNetworkMode {
  const envNetworkMode =
    import.meta.env.VITE_TON_NETWORK === 'mainnet' ? 'mainnet' : 'testnet';

  if (typeof window === 'undefined') {
    return envNetworkMode;
  }

  const params = new URLSearchParams(window.location.search);
  const urlNetwork = params.get('network');
  if (isNetworkMode(urlNetwork)) {
    return urlNetwork;
  }

  const isTestnet = params.get('testnet');
  if (isTestnet === 'true') {
    return 'testnet';
  }
  if (isTestnet === 'false') {
    return 'mainnet';
  }

  try {
    const storedNetwork = window.localStorage.getItem(NETWORK_STORAGE_KEY);
    if (isNetworkMode(storedNetwork)) {
      return storedNetwork;
    }
  } catch {
    // Ignore storage access errors and fall back to the configured default.
  }

  return envNetworkMode;
}

function toncenterApiKey(network: TonNetworkMode): string | undefined {
  const networkKey =
    network === 'testnet'
      ? import.meta.env.TONCENTER_TESTNET_API_KEY
      : import.meta.env.TONCENTER_MAINNET_API_KEY;

  return (
    networkKey ??
    import.meta.env.TONCENTER_API_KEY ??
    import.meta.env.VITE_TONCENTER_API_KEY
  );
}

function toncenterBaseUrl(network: TonNetworkMode): string {
  return network === 'testnet'
    ? 'https://testnet.toncenter.com'
    : 'https://toncenter.com';
}

export const TON_NETWORK_MODE = readNetworkMode();
export const TON_NETWORK = TON_NETWORK_MODE === 'mainnet' ? MAINNET : TESTNET;
export const IS_TESTNET = TON_NETWORK.chainId === TESTNET.chainId;
export const TON_NETWORK_LABEL = IS_TESTNET ? 'Testnet' : 'Mainnet';
export const TONCENTER_API_KEY = toncenterApiKey(TON_NETWORK_MODE);
export const TONCENTER_REQUEST_DELAY_MS = TONCENTER_API_KEY ? 350 : 2600;
const TONCENTER_RATE_LIMIT_RETRY_MS = TONCENTER_API_KEY ? 2500 : 8000;
const TONCENTER_MAX_RETRIES = TONCENTER_API_KEY ? 2 : 4;

const selectedToncenterBaseUrl = toncenterBaseUrl(TON_NETWORK_MODE);
const TON_CONNECT_MANIFEST_URL =
  import.meta.env.VITE_TON_CONNECT_MANIFEST_URL ??
  'https://ton-connect.github.io/demo-dapp-with-react-ui/tonconnect-manifest.json';

export const TONCENTER_BASE_URL = selectedToncenterBaseUrl;
export const TONCENTER_RPC_URL = `${selectedToncenterBaseUrl}/api/v2/jsonRPC`;
export const TONVIEWER_URL = IS_TESTNET
  ? 'https://testnet.tonviewer.com'
  : 'https://tonviewer.com';

export const tonClient = new TonClient({
  endpoint: TONCENTER_RPC_URL,
  apiKey: TONCENTER_API_KEY,
});

let toncenterQueue: Promise<unknown> = Promise.resolve();
let nextToncenterRequestAt = Date.now() + (TONCENTER_API_KEY ? 0 : 2500);
let toncenterCooldownUntil = 0;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

function rawErrorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : typeof error === 'string'
      ? error
      : 'Unexpected error.';
}

function isToncenterRateLimit(error: unknown): boolean {
  const status = readHttpStatus(error);
  const rawMessage = rawErrorMessage(error);

  return (
    status === 429 ||
    rawMessage.includes('status code 429') ||
    rawMessage.includes('Too Many Requests') ||
    rawMessage.includes('Ratelimit exceed')
  );
}

async function waitForToncenterSlot() {
  const waitUntil = Math.max(nextToncenterRequestAt, toncenterCooldownUntil);
  const waitMs = Math.max(0, waitUntil - Date.now());

  if (waitMs > 0) {
    await sleep(waitMs);
  }

  nextToncenterRequestAt = Date.now() + TONCENTER_REQUEST_DELAY_MS;
}

async function runQueuedToncenterRequest<T>(
  operation: () => Promise<T>,
): Promise<T> {
  let lastError: unknown;

  for (let attempt = 0; attempt <= TONCENTER_MAX_RETRIES; attempt += 1) {
    await waitForToncenterSlot();

    try {
      return await operation();
    } catch (error) {
      lastError = error;

      if (!isToncenterRateLimit(error) || attempt === TONCENTER_MAX_RETRIES) {
        throw error;
      }

      toncenterCooldownUntil = Math.max(
        toncenterCooldownUntil,
        Date.now() + TONCENTER_RATE_LIMIT_RETRY_MS * (attempt + 1),
      );
    }
  }

  throw lastError;
}

export function runToncenterRequest<T>(
  operation: () => Promise<T>,
): Promise<T> {
  const result = toncenterQueue.then(() =>
    runQueuedToncenterRequest(operation),
  );
  toncenterQueue = result.then(
    () => undefined,
    () => undefined,
  );

  return result;
}

export function setTonNetworkMode(network: TonNetworkMode) {
  try {
    window.localStorage.setItem(NETWORK_STORAGE_KEY, network);
  } catch {
    // The URL is still enough to select the network after reload.
  }

  if (network === TON_NETWORK_MODE) {
    return;
  }

  const url = new URL(window.location.href);
  url.searchParams.set('network', network);
  url.searchParams.delete('testnet');
  window.location.assign(url.toString());
}

export function formatAddressForNetwork(
  address: Address | string,
  chainId: string | number = TON_NETWORK.chainId,
): string {
  const parsed = Address.isAddress(address) ? address : Address.parse(address);

  return parsed.toString({
    bounceable: false,
    testOnly: chainId === TESTNET.chainId,
  });
}

export function normalizeAddress(address: Address | string): string {
  return formatAddressForNetwork(address);
}

export function toRawAddress(address: Address | string): string {
  const parsed = Address.isAddress(address) ? address : Address.parse(address);
  return parsed.toRawString();
}

export function sameAddress(
  left: Address | string | null | undefined,
  right: Address | string | null | undefined,
): boolean {
  if (!left || !right) {
    return false;
  }

  try {
    const parsedLeft = Address.isAddress(left) ? left : Address.parse(left);
    const parsedRight = Address.isAddress(right) ? right : Address.parse(right);
    return parsedLeft.equals(parsedRight);
  } catch {
    return false;
  }
}

function trimFraction(value: string, precision: number): string {
  const [whole, fraction = ''] = value.split('.');

  if (!fraction) {
    return whole;
  }

  const trimmedFraction = fraction.slice(0, precision).replace(/0+$/, '');
  return trimmedFraction ? `${whole}.${trimmedFraction}` : whole;
}

export function formatTonAmount(
  value: bigint | string | null | undefined,
  precision = 4,
): string {
  if (value === null || value === undefined) {
    return 'n/a';
  }

  const source = typeof value === 'bigint' ? fromNano(value) : value;
  return `${trimFraction(source, precision)} TON`;
}

export function getExplorerUrl(address: Address | string): string {
  return `${TONVIEWER_URL}/${encodeURIComponent(formatAddressForNetwork(address))}`;
}

function readHttpStatus(error: unknown): number | null {
  if (typeof error !== 'object' || error === null) {
    return null;
  }

  const candidate = error as {
    response?: { status?: number };
    status?: number;
    statusCode?: number;
  };

  return (
    candidate.response?.status ??
    candidate.status ??
    candidate.statusCode ??
    null
  );
}

export function getErrorMessage(error: unknown): string {
  const status = readHttpStatus(error);
  const rawMessage = rawErrorMessage(error);

  if (
    status === 429 ||
    rawMessage.includes('status code 429') ||
    rawMessage.includes('Too Many Requests') ||
    rawMessage.includes('Ratelimit exceed')
  ) {
    return 'Toncenter rate limit reached (HTTP 429). Wait and retry, or set TONCENTER_TESTNET_API_KEY / TONCENTER_MAINNET_API_KEY.';
  }

  if (
    status === 401 ||
    status === 403 ||
    rawMessage.includes('status code 401') ||
    rawMessage.includes('status code 403')
  ) {
    return 'Toncenter rejected the API request. Check the configured TONCENTER API key for the selected network.';
  }

  if (
    status !== null &&
    status >= 500 &&
    status < 600 &&
    !rawMessage.includes(`HTTP ${status}`)
  ) {
    return `Toncenter API error (HTTP ${status}). Retry after the API recovers.`;
  }

  if (
    rawMessage.includes('Failed to fetch') ||
    rawMessage.includes('NetworkError') ||
    rawMessage.includes('fetch failed')
  ) {
    return 'Network request failed. Check connectivity and the selected TON network.';
  }

  return rawMessage;
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

export const appKit = new AppKit({
  networks: {
    [TON_NETWORK.chainId]: {
      apiClient: {
        url: TONCENTER_BASE_URL,
        key: TONCENTER_API_KEY,
      },
    },
  },
  defaultNetwork: TON_NETWORK,
  connectors: [
    new TonConnectConnector({
      tonConnectOptions: {
        manifestUrl: TON_CONNECT_MANIFEST_URL,
      },
    }),
  ],
});
