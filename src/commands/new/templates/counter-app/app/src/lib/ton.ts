import { Address } from '@ton/core';
import { TonClient } from '@ton/ton';
import { QueryClient } from '@tanstack/react-query';
import { CHAIN } from '@tonconnect/ui-react';

export type TonNetworkMode = 'mainnet' | 'testnet';

const NETWORK_STORAGE_KEY = 'counter-network';

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
  return network === 'testnet'
    ? import.meta.env.TONCENTER_TESTNET_API_KEY
    : import.meta.env.TONCENTER_MAINNET_API_KEY;
}

function toncenterBaseUrl(network: TonNetworkMode): string {
  return network === 'testnet'
    ? 'https://testnet.toncenter.com'
    : 'https://toncenter.com';
}

export const TON_NETWORK_MODE = readNetworkMode();
export const TON_CHAIN: CHAIN =
  TON_NETWORK_MODE === 'mainnet' ? CHAIN.MAINNET : CHAIN.TESTNET;
export const IS_TESTNET = TON_CHAIN === CHAIN.TESTNET;
export const TON_NETWORK_LABEL = IS_TESTNET ? 'Testnet' : 'Mainnet';
export const TONCENTER_API_KEY = toncenterApiKey(TON_NETWORK_MODE);

const selectedToncenterBaseUrl = toncenterBaseUrl(TON_NETWORK_MODE);

export const TONCENTER_BASE_URL = selectedToncenterBaseUrl;
export const TONCENTER_RPC_URL = `${selectedToncenterBaseUrl}/api/v2/jsonRPC`;
export const TONSCAN_ADDRESS_URL = IS_TESTNET
  ? 'https://testnet.tonscan.org/address'
  : 'https://tonscan.org/address';

export const tonClient = new TonClient({
  endpoint: TONCENTER_RPC_URL,
  apiKey: TONCENTER_API_KEY,
});

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
  address: string,
  chain: CHAIN | string = TON_CHAIN,
): string {
  return Address.parse(address).toString({
    bounceable: false,
    testOnly: chain === CHAIN.TESTNET,
  });
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
