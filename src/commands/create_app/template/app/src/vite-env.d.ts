/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_TON_NETWORK?: 'mainnet' | 'testnet';
  readonly TONCENTER_MAINNET_API_KEY?: string;
  readonly TONCENTER_TESTNET_API_KEY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
