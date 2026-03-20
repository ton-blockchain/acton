import { Address } from '@ton/core';
import { QueryClient } from '@tanstack/react-query';
import { AppKit, Network, TonConnectConnector } from '@ton/appkit-react';

const networkMode =
  import.meta.env.VITE_TON_NETWORK === 'mainnet' ? 'mainnet' : 'testnet';

export const TON_NETWORK =
  networkMode === 'mainnet' ? Network.mainnet() : Network.testnet();
export const IS_TESTNET = TON_NETWORK.chainId === Network.testnet().chainId;
export const TON_NETWORK_LABEL = IS_TESTNET ? 'Testnet' : 'Mainnet';
export const TONCENTER_API_KEY = import.meta.env.VITE_TONCENTER_API_KEY;

const toncenterBaseUrl = IS_TESTNET
  ? 'https://testnet.toncenter.com'
  : 'https://toncenter.com';
const TON_CONNECT_MANIFEST_URL =
  'https://ton-connect.github.io/demo-dapp-with-react-ui/tonconnect-manifest.json';

export const TONCENTER_BASE_URL = toncenterBaseUrl;
export const TONCENTER_RPC_URL = `${toncenterBaseUrl}/api/v2/jsonRPC`;
export const TONVIEWER_URL = IS_TESTNET
  ? 'https://testnet.tonviewer.com'
  : 'https://tonviewer.com';

export function formatAddressForNetwork(
  address: string,
  chainId: string | number = TON_NETWORK.chainId,
): string {
  return Address.parse(address).toString({
    bounceable: false,
    testOnly: chainId === Network.testnet().chainId,
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
