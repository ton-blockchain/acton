import { Address } from '@ton/core';
import { TonClient } from '@ton/ton';
import { QueryClient } from '@tanstack/react-query';
import { CHAIN } from '@tonconnect/ui-react';

import type { Network } from './router';

export function toncenterApiKey(network: Network): string | undefined {
  return network === 'testnet'
    ? import.meta.env.TONCENTER_TESTNET_API_KEY
    : import.meta.env.TONCENTER_MAINNET_API_KEY;
}

export function toncenterBaseUrl(network: Network): string {
  return network === 'testnet'
    ? 'https://testnet.toncenter.com'
    : 'https://toncenter.com';
}

export function toncenterRpcUrl(network: Network): string {
  return `${toncenterBaseUrl(network)}/api/v2/jsonRPC`;
}

export function tonviewerUrl(network: Network): string {
  return network === 'testnet'
    ? 'https://testnet.tonviewer.com'
    : 'https://tonviewer.com';
}

export function networkLabel(network: Network): string {
  return network === 'testnet' ? 'Testnet' : 'Mainnet';
}

export function networkChain(network: Network): CHAIN {
  return network === 'testnet' ? CHAIN.TESTNET : CHAIN.MAINNET;
}

export function formatAddressForNetwork(
  address: string,
  network: Network,
): string {
  return Address.parse(address).toString({
    bounceable: false,
    testOnly: network === 'testnet',
  });
}

const clientCache = new Map<Network, TonClient>();

export function getTonClient(network: Network): TonClient {
  const existing = clientCache.get(network);
  if (existing) return existing;
  const client = new TonClient({
    endpoint: toncenterRpcUrl(network),
    apiKey: toncenterApiKey(network),
  });
  clientCache.set(network, client);
  return client;
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { refetchOnWindowFocus: false, retry: 1 },
  },
});
